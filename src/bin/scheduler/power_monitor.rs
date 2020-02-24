use std::cell::Cell;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use dbus::arg::OwnedFd;
use dbus::blocking::Connection;
use dbus::Message;
use slog::{debug, error, info, Logger};

use night_kitchen::dbus::login_manager;
use night_kitchen::dbus::logind::{
    OrgFreedesktopLogin1Manager, OrgFreedesktopLogin1ManagerPrepareForShutdown,
    OrgFreedesktopLogin1ManagerPrepareForSleep,
};

/// A power event reported by logind
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PowerEvent {
    /// Indicates that the system is about to suspend/sleep
    PreSleep,

    /// Indicates that the system has resumed from suspend/sleep
    PostSleep,

    /// Indicates that the system is about to shut down or reboot
    PreShutdown,
}

/// State manager for detecting events around system suspend and shutdown.
/// 
/// Internally, `PowerMonitor` uses D-Bus signals to notice when the system is preparing to sleep or shutdown. It also
/// uses systemd inhibitor locks to prevent the system from doing so until its callback has completed.
/// 
/// See [the systemd documentation](https://www.freedesktop.org/wiki/Software/systemd/inhibit/) for more details.
pub struct PowerMonitor<F: Fn(PowerEvent) + Send + Sync + 'static> {
    // The "who" and "why" we're taking inhibitor locks
    inhibitor_source: String,
    inhibitor_reason: String,

    callback: F,
    inhibitor: Mutex<Cell<Option<OwnedFd>>>,
    logger: Logger,
}

impl<F: Fn(PowerEvent) + Send + Sync + 'static> PowerMonitor<F> {
    /// Create a new `PowerMonitor` that calls `callback` on any system power events it detects.
    /// 
    /// The `inhibitor_source` and `inhibitor_reason` values are passed to systemd and indicate who is delaying shutdown/suspend and why, respectively.
    pub fn new<S1: Into<String>, S2: Into<String>>(
        logger: Logger,
        inhibitor_source: S1,
        inhibitor_reason: S2,
        callback: F,
    ) -> Arc<PowerMonitor<F>> {
        Arc::new(PowerMonitor {
            inhibitor_source: inhibitor_source.into(),
            inhibitor_reason: inhibitor_reason.into(),
            callback,
            inhibitor: Mutex::new(Cell::new(None)),
            logger,
        })
    }

    /// Run the monitor on the current thread, blocking forever if no errors occur.
    pub fn run_blocking(conn: &mut Connection, monitor: Arc<PowerMonitor<F>>) -> Result<()> {
        PowerMonitor::register_signal_matchers(monitor.clone(), &conn);
        monitor
            .take_inhibitor(&conn)
            .context("Could not take inhibitor lock")?;

        // TODO: way to break out of loop?
        loop {
            conn.process(Duration::from_secs(60))?;
        }
    }

    /// Using the given system D-Bus connection, request a `delay` inhibitor lock with the `sleep` and
    /// `shutdown` lock types. If this monitor already holds an inhibitor lock, it will not take a new one.
    fn take_inhibitor(&self, conn: &Connection) -> Result<()> {
        let manager = login_manager(conn);

        let inhibitor = self
            .inhibitor
            .lock()
            .map_err(|_| anyhow!("Mutex containing inhibitor lock was poisoned"))?;
        let new_inhibitor = match inhibitor.take() {
            Some(fd) => Some(fd), // If we already have the lock, don't re-take it
            None => {
                let fd = manager
                    .inhibit(
                        "sleep:shutdown",
                        &self.inhibitor_source,
                        &self.inhibitor_reason,
                        "delay",
                    )
                    .context("Failed to take inhibitor lock")?;
                debug!(&self.logger, "Took inhibitor lock"; "fd" => ?fd);
                Some(fd)
            }
        };
        inhibitor.set(new_inhibitor);

        Ok(())
    }

    /// If this monitor holds an inhibitor lock, release it.
    fn release_inhibitor(&self) -> Result<()> {
        // If we had an inhibitor lock, .take() will replace the Some(OwnedFd) with None
        // Then, dropping the OwnedFd will close the file descriptor and release the lock
        debug!(&self.logger, "Releasing inhibitor lock");
        self.inhibitor
            .lock()
            .map_err(|_| anyhow!("Mutex containing inhibitor lock was poisoned"))?
            .take();
        Ok(())
    }

    /// Add signal matchers to the given system D-Bus connection that will monitor the
    /// `PrepareForSleep` and `PrepareForShutdown` signals. When those signals are received,
    /// the monitor's inhibitor lock will be updated appropriately following the standard
    /// [delay lock pattern](https://www.freedesktop.org/wiki/Software/systemd/inhibit/).
    /// In addition, the monitor's callback will be called with the corresponding `PowerEvent`.
    fn register_signal_matchers(monitor: Arc<PowerMonitor<F>>, conn: &Connection) {
        let manager = login_manager(conn);

        {
            let monitor = monitor.clone();
            let _ = manager.match_signal(
                move |p: OrgFreedesktopLogin1ManagerPrepareForSleep, c: &Connection, _: &Message| {
                    let cb = &monitor.callback;
                    if p.arg0 {
                        info!(&monitor.logger, "About to sleep");
                        cb(PowerEvent::PreSleep);
                        match monitor.release_inhibitor() {
                            Ok(_) => (),
                            Err(e) => error!(&monitor.logger, "Failed to release inhibitor"; "error" => ?e)
                        };
                    } else {
                        info!(&monitor.logger, "Resumed from sleep");
                        cb(PowerEvent::PostSleep);
                        match monitor.take_inhibitor(c) {
                            Ok(_) => (),
                            Err(e) => error!(&monitor.logger, "Failed to take inhibitor"; "error" => ?e)
                        };
                    }
                    true
                },
            );
        }

        let _ = manager.match_signal(
            move |p: OrgFreedesktopLogin1ManagerPrepareForShutdown, _: &Connection, message: &Message| {
                let cb = &monitor.callback;
                if p.arg0 {
                    info!(&monitor.logger, "About to shut down");
                    cb(PowerEvent::PreShutdown);
                    match monitor.release_inhibitor() {
                        Ok(_) => (),
                        Err(e) => error!(&monitor.logger, "Failed to release inhibitor"; "error" => ?e)
                    };
            } else {
                    error!(&monitor.logger, "Unexpected PrepareForShutdown(false) signal"; "message" => ?message);
                }
                true
            }
        );
    }
}
