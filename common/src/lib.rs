use std::time::Duration;

use dbus::blocking::{Connection, Proxy};
use slog::{Logger, Duplicate, Drain, o};
use slog_async::Async;
use slog_journald::JournaldDrain;
use slog_term::{FullFormat, TermDecorator};

mod login1;
mod power_monitor;
mod session;

pub use power_monitor::{PowerEvent, PowerMonitor};
pub use session::SessionClient;

pub fn root_logger(name: &'static str, version: &'static str) -> Logger {
    let decorator = TermDecorator::new().build();
    let term_drain = FullFormat::new(decorator).build();
    let drain = Duplicate::new(term_drain, JournaldDrain).fuse();
    let drain = Async::new(drain).build().fuse();
    Logger::root(drain, o!("name" => name, "version" => version))
}

pub(crate) fn login_manager<'a>(connection: &'a Connection) -> Proxy<'a, &'a Connection> {
    connection.with_proxy(
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        Duration::from_millis(500),
    )
}
