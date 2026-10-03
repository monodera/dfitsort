//! `-p`: output through `$PAGER` (default `less`) when stdout is a terminal (Unix only).

use std::ffi::{OsStr, OsString};
use std::io::{self, BufWriter, IsTerminal, Write};
use std::process::{Child, Command, Stdio};

use crate::run;

/// The running pager, if any, and the error messages held back while it owns the terminal.
pub struct Pager {
    running: Option<Running>,
    held: Vec<String>,
}

struct Running {
    child: Child,
    /// `$PAGER` as given, for messages.
    name: String,
    via_shell: bool,
    /// SIGINT disposition to restore when the pager has exited.
    old_sigint: sigint::Saved,
}

/// Starts the pager when `page` is set and stdout is a terminal; returns the stream to
/// write the output to, which goes to stdout otherwise.
pub fn start(page: bool) -> (Box<dyn Write>, Pager) {
    let running = page.then(command).flatten().and_then(|(command, name)| spawn(command, name));
    let mut pager = Pager { running, held: Vec::new() };
    match pager.running.as_mut().and_then(|r| r.child.stdin.take()) {
        Some(stdin) => (Box::new(BufWriter::with_capacity(1 << 16, stdin)), pager),
        None => (Box::new(BufWriter::with_capacity(1 << 16, io::stdout().lock())), pager),
    }
}

/// The pager command and `$PAGER`, or `None` when there is to be no pager.
fn command() -> Option<(Command, OsString)> {
    if cfg!(not(unix)) || !io::stdout().is_terminal() {
        return None;
    }
    let cmd = std::env::var_os("PAGER").unwrap_or_else(|| "less".into());
    if matches!(cmd.to_str().map(str::trim), Some("" | "cat")) {
        return None;
    }
    let mut command = if is_plain(&cmd) {
        Command::new(&cmd)
    } else {
        let mut sh = Command::new("sh");
        sh.arg("-c").arg(&cmd);
        sh
    };
    command.stdin(Stdio::piped());
    if std::env::var_os("LESS").is_none() {
        // Quit if one screen, pass colours through, keep the text on screen after quitting.
        command.env("LESS", "FRX");
    }
    Some((command, cmd))
}

/// As in git: a plain command name is run directly, not through sh, so that a missing
/// one fails to spawn (and the output goes to stdout) instead of failing inside sh.
fn is_plain(cmd: &OsStr) -> bool {
    cmd.to_str().is_some_and(|c| !c.contains(|ch| "|&;<>()$`\\\"' \t\n*?[#~=%".contains(ch)))
}

fn spawn(mut command: Command, name: OsString) -> Option<Running> {
    let via_shell = !is_plain(&name);
    let name = name.to_string_lossy().into_owned();
    let old_sigint = sigint::ignore(&mut command);
    match command.spawn() {
        Ok(child) => Some(Running { child, name, via_shell, old_sigint }),
        Err(e) => {
            sigint::restore(old_sigint);
            eprintln!("dfitsort: cannot run pager {name}: {e}");
            None
        }
    }
}

/// Ctrl-C in the pager reaches us too; dying would hand the terminal back to the
/// shell while the pager still uses it. So SIGINT is ignored here while it runs.
#[cfg(unix)]
mod sigint {
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    pub type Saved = libc::sighandler_t;

    /// Ignores SIGINT here, but not in `command`; returns the disposition replaced.
    pub fn ignore(command: &mut Command) -> Saved {
        // SAFETY: SIG_IGN and a disposition returned by `signal` install no handler code.
        let old = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
        // SAFETY: `signal` is async-signal-safe. The pager must not inherit SIG_IGN.
        unsafe {
            command.pre_exec(move || {
                libc::signal(libc::SIGINT, old);
                Ok(())
            })
        };
        old
    }

    pub fn restore(old: Saved) {
        // SAFETY: as in `ignore`.
        unsafe { libc::signal(libc::SIGINT, old) };
    }
}

/// No pager is started off Unix.
#[cfg(not(unix))]
mod sigint {
    use std::process::Command;

    pub struct Saved;

    pub fn ignore(_: &mut Command) -> Saved {
        Saved
    }

    pub fn restore(_: Saved) {}
}

impl Pager {
    /// Prints `text` to stderr, or holds it until the pager has exited.
    pub fn eprint(&mut self, text: String) {
        if self.running.is_some() {
            self.held.push(text);
        } else {
            eprint!("{text}");
        }
    }

    /// Closes `out`, waits for the pager, prints the held messages and returns the exit
    /// status as [`run::finish`] does, but at least 1 when sh could not run the pager
    /// (the output is then lost).
    pub fn finish(self, out: Box<dyn Write>, result: io::Result<()>, code: i32) -> i32 {
        drop(out);
        let mut lost = false;
        if let Some(mut running) = self.running {
            let status = running.child.wait();
            sigint::restore(running.old_sigint);
            // sh exits 127 when the command is not found, 126 when it cannot be executed.
            if running.via_shell && status.is_ok_and(|s| matches!(s.code(), Some(126 | 127))) {
                eprintln!("dfitsort: cannot run pager {}", running.name);
                lost = true;
            }
        }
        self.held.iter().for_each(|text| eprint!("{text}"));
        let status = run::finish(result, code);
        if lost { status.max(1) } else { status }
    }
}

/// Legacy tools: `-p` asks for the pager as the first argument, right after the tool's
/// leading options (`lead` counts them in the arguments it is given) or as the last
/// argument; returns that and the arguments without it (argv[0] kept).
pub fn legacy_p(args: &[OsString], lead: impl Fn(&[OsString]) -> usize) -> (bool, Vec<OsString>) {
    let mut args = args.to_vec();
    let mut page = false;
    let mut take = |args: &mut Vec<OsString>, i: usize| {
        if i >= 1 && args.get(i).is_some_and(|a| a == "-p") {
            args.remove(i);
            page = true;
        }
    };
    take(&mut args, 1);
    let after_options = 1 + lead(&args);
    take(&mut args, after_options);
    let last = args.len().saturating_sub(1);
    take(&mut args, last);
    (page, args)
}
