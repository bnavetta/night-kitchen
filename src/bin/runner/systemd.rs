use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Result, Context};
use dbus::Message;
use dbus::blocking::Connection;
use slog::{Logger, debug, error};

use night_kitchen::dbus::systemd_manager;
use night_kitchen::dbus::systemd::{OrgFreedesktopSystemd1Manager, OrgFreedesktopSystemd1ManagerJobRemoved};

/// Starts the given systemd unit and blocks until it has started.
pub fn start_unit(logger: &Logger, conn: &mut Connection, unit: &str) -> Result<()> {
    let manager = systemd_manager(conn);

    manager.subscribe().context("Could not subscribe to systemd signals")?;

    let started = Arc::new(AtomicBool::new(false));

    {
        let logger = logger.clone();
        let started = started.clone();
        let unit = unit.to_string();

        manager.match_signal(move |j: OrgFreedesktopSystemd1ManagerJobRemoved, _: &Connection, _: &Message| {
            if j.arg2 == unit {
                debug!(&logger, "Job for {} completed with result: {}", unit, j.arg3; "unit" => &unit, "result" => &j.arg3, "job" => %j.arg1, "id" => j.arg0);
                started.store(true, Ordering::Relaxed);
                false
            } else {
                true
            }
        }).context("Could not listen for job signals")?;
    }

    match manager.start_unit(&unit, "fail") {
        Ok(job) => {
            debug!(logger, "Started job {} for {}", job, unit; "job" => %job, "unit" => unit);
        },
        Err(err) => {
            error!(logger, "Failed to start {}", unit; "unit" => unit, "error" => ?err);
            return Err(err.into());
        }
    };

    while !started.load(Ordering::Relaxed) {
        conn.process(Duration::from_millis(500)).context("Failed waiting for D-Bus signals from systemd")?;
    }

    // TODO: capture job status

    Ok(())
}