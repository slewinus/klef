//! Helpers for stdout writes that exit cleanly on `BrokenPipe`.
//!
//! Rust's default `println!`/`print!` panics with
//! `"failed printing to stdout: Broken pipe (os error 32)"`
//! when the downstream consumer of stdout closes the pipe early
//! (typical `klef list | head` scenario). These helpers swallow that
//! specific error path and exit with status 0 (a clean SIGPIPE-style
//! exit), while still propagating other I/O failures.
//!
//! See issue #73 for context.

use std::io;

/// Print a line to stdout, exiting cleanly on `BrokenPipe`.
///
/// Same calling convention as `println!`. All other I/O errors abort
/// the process with status 74 (`sysexits.h` `EX_IOERR`).
#[macro_export]
macro_rules! outln {
    () => {{
        use std::io::Write as _;
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();
        if let Err(e) = writeln!(lock) {
            $crate::output::handle_io_error(&e);
        }
    }};
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();
        if let Err(e) = writeln!(lock, $($arg)*) {
            $crate::output::handle_io_error(&e);
        }
    }};
}

/// Print without newline, exiting cleanly on `BrokenPipe`.
#[macro_export]
macro_rules! out {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();
        if let Err(e) = write!(lock, $($arg)*) {
            $crate::output::handle_io_error(&e);
        }
    }};
}

/// Handle an I/O error from a stdout write. `BrokenPipe` -> exit 0.
/// Anything else -> log on stderr and exit 74.
pub fn handle_io_error(e: &io::Error) -> ! {
    if e.kind() == io::ErrorKind::BrokenPipe {
        std::process::exit(0);
    }
    eprintln!("error: stdout write failed: {e}");
    std::process::exit(74);
}
