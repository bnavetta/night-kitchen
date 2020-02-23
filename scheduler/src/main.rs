use slog::info;

use night_kitchen_common::{PowerMonitor, root_logger};

fn main() -> anyhow::Result<()> {
    let logger = root_logger(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    let monitor = PowerMonitor::new(logger.clone(), "Night Kitchen Scheduler", "Scheduling next system wakeup", move |ev| {
        info!(&logger, "Got a power event"; "event" => ?ev);
    });
    
    PowerMonitor::run_blocking(monitor)?;

    Ok(())
}
