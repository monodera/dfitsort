//! `-p`: output through `$PAGER` (default `less`) when stdout is a terminal (Unix only).

use std::ffi::{OsStr, OsString};
use std::io::{self, BufWriter, IsTerminal, Write};
use std::process::{Child, Command, Stdio};

use crate::run;

/// The running pager, if any, with the SIGINT disposition to restore when it has exited,
/// and the error messages held back while it owns the terminal.
pub struct Pager {
    child: Option<(Child, sigint::Saved)>,
    held: Vec<String>,
}

/// Starts the pager when `page` is set and stdout is a terminal; returns the stream to
/// write the output to, which goes to stdout otherwise.
pub fn start(page: bool) -> (Box<dyn Write>, Pager) {
    let child = page.then(command).flatten().and_then(spawn);
    let mut pager = Pager { child, held: Vec::new() };
    match pager.child.as_mut().and_then(|(c, _)| c.stdin.take()) {
        Some(stdin) => (Box::new(BufWriter::with_capacity(1 << 16, stdin)), pager),
        None => (Box::new(BufWriter::with_capacity(1 << 16, io::stdout().lock())), pager),
    }
}

/// The pager command, or `None` when there is to be no pager.
fn command() -> Option<Command> {
    if cfg!(not(unix)) || !io::stdout().is_terminal() {
        return None;
    }
    let cmd = std::env::var_os("PAGER").unwrap_or_else(|| "less".into());
    if matches!(cmd.to_str().map(str::trim), Some("" | "cat")) {
        return None;
    }
    let mut command = Command::new("sh");
    command.arg("-c").arg(cmd).stdin(Stdio::piped());
    if std::env::var_os("LESS").is_none() {
        // Quit if one screen, pass colours through, keep the text on screen after quitting.
        command.env("LESS", "FRX");
    }
    Some(command)
}

/// Spawns the pager; returns it and the SIGINT disposition to restore when it has exited.
fn spawn(mut command: Command) -> Option<(Child, sigint::Saved)> {
    let old = sigint::ignore(&mut command);
    match command.spawn() {
        Ok(child) => Some((child, old)),
        Err(e) => {
            sigint::restore(old);
            let cmd = command.get_args().nth(1).unwrap_or_default().to_string_lossy();
            eprintln!("dfitsort: cannot run pager {cmd}: {e}");
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
        if self.child.is_some() {
            self.held.push(text);
        } else {
            eprint!("{text}");
        }
    }

    /// Closes `out`, waits for the pager, prints the held messages and returns the exit
    /// status as [`run::finish`] does.
    pub fn finish(self, out: Box<dyn Write>, result: io::Result<()>, code: i32) -> i32 {
        drop(out);
        if let Some((mut child, old_sigint)) = self.child {
            let _ = child.wait();
            sigint::restore(old_sigint);
        }
        self.held.iter().for_each(|text| eprint!("{text}"));
        run::finish(result, code)
    }
}

/// Legacy tools: a first argument `-p` asks for the pager; returns that and the
/// arguments without it (argv[0] kept).
pub fn leading_p(args: &[OsString]) -> (bool, Vec<OsString>) {
    match args.get(1) {
        Some(a) if a == OsStr::new("-p") => (true, args[..1].iter().chain(&args[2..]).cloned().collect()),
        _ => (false, args.to_vec()),
    }
}
