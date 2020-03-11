use std::env;
use std::fs;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use dbus::blocking::Connection;
use nix::sys::sysinfo::sysinfo;
use slog::{debug, error, info, Logger};

use night_kitchen::{resume_timestamp_file, root_logger};

mod systemd;

/// This is the shortest uptime for which night-kitchen will not hold itself responsible for booting. If the
/// uptime at program start is any less than this, night-kitchen-runner will shut the system down afterwards.
const MIN_INNOCENT_UPTIME: Duration = Duration::from_secs(300);

/// This is the shortest time since the resume timestamp was written for which night-kitchen will not hold itself
/// responsible for waking the system up.
const MIN_INNOCENT_WAKETIME: Duration = Duration::from_secs(60);

fn main() -> Result<()> {
    let logger = root_logger();

    let start_time = Utc::now();
    debug!(&logger, "night-kitchen-runner started at {}", start_time; "start_time" => start_time.timestamp());
    let should_shutdown = caused_boot(&logger);

    let unit = match env::args().nth(1) {
        Some(unit) => unit,
        None => bail!(
            "Usage: {} <systemd unit name>",
            env::args()
                .next()
                .unwrap_or_else(|| "night-kitchen-runner".to_string())
        ),
    };
    info!(&logger, "Running systemd unit {unit}", unit = &unit);

    let mut dbus_conn = Connection::new_system().context("Could not connect to system D-Bus")?;
    systemd::start_unit(&logger, &mut dbus_conn, &unit)?;

    if should_shutdown {
        info!(&logger, "Shutting system down...");
        systemd::shutdown(&dbus_conn)?;
    } else if caused_wake(&logger, start_time) {
        info!(&logger, "Suspending system...");
        systemd::suspend(&dbus_conn)?;
    } else {
        info!(&logger, "Not responsible for booting/waking");
    }

    Ok(())
}

/// Returns `true` if night kitchen was most likely responsible for the system booting. This uses the current uptime
/// as a heuristic, so it must be called early on
fn caused_boot(logger: &Logger) -> bool {
    match sysinfo() {
        Ok(info) => {
            let uptime = info.uptime();
            debug!(&logger, "Uptime is {:?}", uptime);
            uptime < MIN_INNOCENT_UPTIME
        }
        Err(err) => {
            error!(&logger, "Could not determine uptime"; "error" => ?err);
            false
        }
    }
}

fn caused_wake(logger: &Logger, start_time: DateTime<Utc>) -> bool {
    let timestamp_str = match fs::read_to_string(resume_timestamp_file()) {
        Ok(s) => s,
        // Assume this failed because the system has not suspended and the file does not exist
        Err(_) => return false,
    };

    let timestamp_ms: i64 = match timestamp_str.parse() {
        Ok(ts) => ts,
        Err(_) => {
            error!(&logger, "Timestamp file was corrupted"; "contents" => timestamp_str);
            return false;
        }
    };

    let resume_time = Utc.timestamp_millis(timestamp_ms);
    debug!(&logger, "Resumed from suspend at {}", resume_time);
    match (start_time - resume_time).to_std() {
        Ok(delta) => delta < MIN_INNOCENT_WAKETIME,
        // If night-kitchen-scheduler didn't write the resume timestamp until after night-kitchen-runner started, it almost certainly is
        // the reason the system resumed
        Err(_) => true,
    }
}
