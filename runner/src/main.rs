use slog::{info, o, Drain, Logger};
use slog_async::Async;
use slog_term::{FullFormat, TermDecorator};

use logind::*;

fn root_logger() -> Logger {
    let decorator = TermDecorator::new().build();
    let drain = FullFormat::new(decorator).build().fuse();
    let drain = Async::new(drain).build().fuse();
    Logger::root(
        drain,
        o!("component" => "night-kitchen-runner", "version" => env!("CARGO_PKG_VERSION")),
    )
}

fn main() {
    let logger = root_logger();
    let manager = LoginManager::new(&logger).unwrap();

    if manager.has_sessions().unwrap() {
        info!(&logger, "Users are logged in!");
    }

    let lock = manager.inhibit(vec![InhibitorLockType::Shutdown, InhibitorLockType::Sleep], InhibitorLockMode::Delay, "Night Kitchen Runner", "Testing").unwrap();
    info!(&logger, "Got lock: {:?}", lock);
}
