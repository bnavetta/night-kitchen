pub mod dbus;

use slog::{Drain, Duplicate, Logger, o};
use slog_async::Async;
use slog_journald::JournaldDrain;
use slog_term::{TermDecorator, FullFormat};

/// Creates a root logger
pub fn root_logger() -> Logger {
    let decorator = TermDecorator::new().build();
    let term_drain = FullFormat::new(decorator).build();
    let drain = Duplicate::new(term_drain, JournaldDrain).fuse();
    let drain = Async::new(drain).build().fuse();
    Logger::root(drain, o!("name" => env!("CARGO_PKG_NAME"), "version" => env!("CARGO_PKG_VERSION")))
}
