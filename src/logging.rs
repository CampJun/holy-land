// Lightweight logging facility for both Miyoo deployment and desktop QA.
//
// Three categories, each with different routing:
//   - log_info!   — important; always to stderr AND to the log file
//   - log_debug!  — context noise; always to the log file; to stderr only
//                   when stderr is NOT a TTY (i.e. Miyoo `2>` redirect),
//                   or when LOG_VERBOSE=1 forces it on
//   - log_verbose!— per-frame noise (fps, timing); log file only unless
//                   LOG_VERBOSE=1 forces it to stderr
//
// The categorization lets the desktop QA terminal stay clean while running
// the debug console (operator can read their typing without `fps: 60`
// scrolling past), while Miyoo deployment keeps capturing everything in
// holyland.log via stderr redirect AND via this module's file handle.
//
// init() is idempotent (OnceLock); macros work uninitialized too (they
// just fall back to plain stderr writes), which keeps tests + early-init
// code paths simple.

use std::fmt;
use std::fs::OpenOptions;
use std::io::{IsTerminal, Write};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static LOGGER: OnceLock<Logger> = OnceLock::new();

pub struct Logger {
    file: Option<Mutex<std::fs::File>>,
    stderr_is_tty: bool,
    verbose_to_stderr: bool,
}

/// Open `<save_dir>/holyland.log` for append. Errors are swallowed so a
/// read-only filesystem (some Miyoo SD card states) doesn't crash the
/// game.
pub fn init(save_dir: &Path) {
    let log_path = save_dir.join("holyland.log");
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok()
        .map(Mutex::new);
    let stderr_is_tty = std::io::stderr().is_terminal();
    let verbose_to_stderr = std::env::var("LOG_VERBOSE")
        .ok()
        .map(|v| v != "0")
        .unwrap_or(false);
    let logger = Logger {
        file,
        stderr_is_tty,
        verbose_to_stderr,
    };
    let _ = LOGGER.set(logger);
    info(format_args!(
        "=== survival session start (log {}) ===",
        log_path.display()
    ));
    if let Some(l) = LOGGER.get() {
        if l.stderr_is_tty && !l.verbose_to_stderr {
            info(format_args!(
                "(interactive terminal detected; fps/timing suppressed from stderr — set LOG_VERBOSE=1 to enable)"
            ));
        }
    }
}

pub fn info(args: fmt::Arguments) {
    eprintln!("{}", args);
    file_write(args);
}

pub fn debug(args: fmt::Arguments) {
    file_write(args);
    if let Some(l) = LOGGER.get() {
        if !l.stderr_is_tty || l.verbose_to_stderr {
            eprintln!("{}", args);
        }
    } else {
        eprintln!("{}", args);
    }
}

pub fn verbose(args: fmt::Arguments) {
    file_write(args);
    if let Some(l) = LOGGER.get() {
        if l.verbose_to_stderr {
            eprintln!("{}", args);
        }
    }
}

fn file_write(args: fmt::Arguments) {
    if let Some(l) = LOGGER.get() {
        if let Some(f) = &l.file {
            if let Ok(mut f) = f.lock() {
                let _ = writeln!(f, "{}", args);
            }
        }
    }
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => { $crate::logging::info(format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => { $crate::logging::debug(format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! log_verbose {
    ($($arg:tt)*) => { $crate::logging::verbose(format_args!($($arg)*)) };
}
