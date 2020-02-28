use std::env;
use std::mem::MaybeUninit;
use std::time::Duration;

use anyhow::{bail, Result, Context};
use chrono::Utc;
use dbus::blocking::LocalConnection;
use libc;
use slog::{Logger, debug, info, error};

use night_kitchen::root_logger;

mod systemd;

/// This is the shortest uptime for which night-kitchen will not hold itself responsible for booting. If the
/// uptime at program start is any less than this, night-kitchen-runner will shut the system down afterwards.
const MIN_INNOCENT_UPTIME: Duration = Duration::from_secs(300);

fn main() -> Result<()> {
    let logger = root_logger();

    let start_time = Utc::now();
    debug!(&logger, "night-kitchen-runner started at {}", start_time; "start_time" => start_time.timestamp());
    let should_shutdown = caused_boot(&logger);

    let unit = match env::args().nth(1) {
        Some(unit) => unit,
        None => bail!("Usage: {} <systemd unit name>", env::args().next().unwrap_or("night-kitchen-runner".to_string()))
    };
    info!(&logger, "Running systemd unit {unit}", unit = &unit);

    let mut dbus_conn = LocalConnection::new_system().context("Could not connect to system D-Bus")?;
    systemd::start_unit(&logger, &mut dbus_conn, &unit)?;

    if should_shutdown {
        info!(&logger, "Shutting system down...");
        systemd::shutdown(&dbus_conn)?;
    } else {
        info!(&logger, "Not responsible for booting, will not shut down");
    }

    Ok(())
}

/// Returns `true` if night kitchen was most likely responsible for the system booting. This uses the current uptime
/// as a heuristic, so it must be called early on 
fn caused_boot(logger: &Logger) -> bool {
    // Use MaybeUninit to get a zeroed sysinfo_t struct for sysinfo() to fill in
    let mut info: libc::sysinfo = unsafe { MaybeUninit::zeroed().assume_init() };

    let status = unsafe {
        libc::sysinfo(&mut info)
    };

    if status == 0 {
        let uptime = Duration::from_secs(info.uptime as u64);
        debug!(&logger, "Uptime is {:?}", uptime);
        uptime < MIN_INNOCENT_UPTIME
    } else {
        error!(&logger, "sysinfo() failed, could not determine uptime");
        false
    }
}