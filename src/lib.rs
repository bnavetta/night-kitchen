use std::env;
use std::path::PathBuf;

pub mod dbus;

use slog::{o, Drain, Duplicate, Logger};
use slog_async::Async;
use slog_journald::JournaldDrain;
use slog_term::{FullFormat, TermDecorator};

/// Creates a root logger
pub fn root_logger() -> Logger {
    let decorator = TermDecorator::new().build();
    let term_drain = FullFormat::new(decorator).build();
    let drain = Duplicate::new(term_drain, JournaldDrain).fuse();
    let drain = Async::new(drain).build().fuse();
    Logger::root(
        drain,
        o!("name" => env!("CARGO_PKG_NAME"), "version" => env!("CARGO_PKG_VERSION")),
    )
}

/// Determines where the system resume timestamp file is. The scheduler updates this whenever the system
/// wakes from suspend, and the runner uses it to decide whether or not to re-suspend.
pub fn resume_timestamp_file() -> PathBuf {
    let runtime_dir = env::var("RUNTIME_DIRECTORY")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    runtime_dir.join("resume-timestamp")
}
