# dfitsort-core

The FITS header reading library behind the
[dfitsort](https://github.com/monodera/dfitsort) command-line tool.

It is published only so that `cargo install dfitsort` can build. It is an
internal crate, not a general-purpose FITS library: its API follows what the CLI
needs and may change in any release without notice. If you want the tool itself,
install `dfitsort`.

Licensed under either of the [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE)
licenses, at your option. See [NOTICE](NOTICE) for the ESO notice covering the
`dfits`/`fitsort` behaviour.
