use slog::info;

mod power_monitor;

use night_kitchen::root_logger;
use crate::power_monitor::PowerMonitor;

fn main() -> anyhow::Result<()> {
    let logger = root_logger();

    let monitor = PowerMonitor::new(logger.clone(), "Night Kitchen Scheduler", "Scheduling next system wakeup", move |ev| {
        info!(&logger, "Got a power event"; "event" => ?ev);
    });
    
    PowerMonitor::run_blocking(monitor)?;

    Ok(())
}
