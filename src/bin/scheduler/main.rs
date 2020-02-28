use std::time::Duration;

use anyhow::{Result, Context, anyhow};
use chrono::{DateTime, Utc, Local};
use dbus::blocking::LocalConnection;
use slog::{Logger, debug, info};

mod power_monitor;
mod server;
mod time;

use night_kitchen::root_logger;
use night_kitchen::dbus::systemd_unit;
use night_kitchen::dbus::systemd_timer::OrgFreedesktopSystemd1Timer;

use crate::power_monitor::{PowerMonitor, PowerEvent};
use crate::time::{from_timestamp_usecs, monotonic_to_realtime};

fn main() -> Result<()> {
    let logger = root_logger();

    let mut conn = LocalConnection::new_system().context("Could not connect to system D-Bus")?;

    for unit in &["systemd-tmpfiles-clean.timer", "shadow.timer"] {
        let next_activation = next_activation(&logger, &conn, unit)?.with_timezone(&Local);
        info!(&logger, "{} will next run at {}", unit, next_activation);
    }

    let server = server::add_server(&conn).context("Could not add D-Bus server")?;

    let monitor = PowerMonitor::new(logger.clone(), "Night Kitchen Scheduler", "Scheduling next system wakeup", move |ev| {
        info!(&logger, "Got a power event"; "event" => ?ev);
        match ev {
            PowerEvent::PreSleep => server.set_resume_timestamp(Utc::now()),
            _ => ()
        };
    });

    // Need to add a D-bus policy file allowing us to request the service name in /usr/share/dbus-1/system.d
    // Or just write to a file instead of using a D-Bus server

    // TODO: on PreShutdown, figure out when next RTC alarm should be (don't clobber if there's a sooner one)

    PowerMonitor::register(&mut conn, monitor)?;

    loop {
        conn.process(Duration::from_secs(60))?;
    }

    // TODO: handle ctrl-c gracefully
}

fn next_activation(logger: &Logger, conn: &LocalConnection, timer_unit: &str) -> Result<DateTime<Utc>> {
    let timer = systemd_unit(conn, timer_unit)?;

    // If either is 0, that means the timer doesn't include any events using the corresponding clock
    let next_realtime = match timer.next_elapse_usec_realtime().context("Could not get next CLOCK_REALTIME elapsation point")? {
        0 => None,
        realtime_usecs => {
            let next_realtime = from_timestamp_usecs(realtime_usecs);
            debug!(&logger, "Next CLOCK_REALTIME elapsation point is {}", next_realtime; "unit" => timer_unit);
            Some(next_realtime)
        }
    };

    let next_monotonic = match timer.next_elapse_usec_monotonic().context("Could not get next monotonic elapsation point")? {
        0 => None,
        monotonic_usecs => {
            let next_monotonic = monotonic_to_realtime(from_timestamp_usecs(monotonic_usecs));
            debug!(&logger, "Next CLOCK_MONOTONIC elapsation point is {}", next_monotonic; "unit" => timer_unit);
            Some(next_monotonic)
        }
    };

    let next_elapse = match (next_realtime, next_monotonic) {
        (_, None) => next_realtime,
        (None, _) => next_monotonic,
        (Some(next_realtime), Some(next_monotonic)) => Some(next_realtime.min(next_monotonic))
    };

    next_elapse.ok_or_else(|| anyhow!("Neither monotonic nor realtime next elapsation point"))
}

