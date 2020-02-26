use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Result, Context};
use dbus::Message;
use dbus::blocking::Connection;
use slog::{Logger, debug, error};

use night_kitchen::dbus::{login_manager, systemd_manager};
use night_kitchen::dbus::systemd::{OrgFreedesktopSystemd1Manager, OrgFreedesktopSystemd1ManagerJobRemoved};
use night_kitchen::dbus::logind::OrgFreedesktopLogin1Manager;

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

/// Powers off the system
pub fn shutdown(conn: &Connection) -> Result<()> {
    // Important: Both the systemd and logind D-Bus APIs have PowerOff methods. The logind method goes through a graceful shutdown, respecting inhibitor locks
    // and stopping services, while the systemd one immediately shuts the system down. Calling the systemd one directly by mistake would be unfortunate.
    let manager = login_manager(conn);
    // The boolean argument is whether PolicyKit should prompt the user for authentication if needed. Since night-kitchen-runner is activated by a timer,
    // we want to fail-fast if we don't have sufficient privileges instead.
    OrgFreedesktopLogin1Manager::power_off(&manager, false).context("Could not power off the system")?;
    Ok(())
}

/// Puts the system to sleep
pub fn suspend(conn: &Connection) -> Result<()> {
    let manager = login_manager(conn);
    // Boolean is the same PolicyKit flag as in shutdown()
    manager.suspend(false).context("Could not suspend the system")?;
    Ok(())
}