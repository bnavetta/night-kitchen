use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local, Utc};
use dbus::blocking::Connection;
use signal_hook;
use slog::{debug, error, info, Logger};

mod power_monitor;
mod time;

use night_kitchen::dbus::systemd_timer::OrgFreedesktopSystemd1Timer;
use night_kitchen::dbus::systemd_unit;
use night_kitchen::{resume_timestamp_file, root_logger};

use crate::power_monitor::{PowerEvent, PowerMonitor};
use crate::time::{from_timestamp_usecs, monotonic_to_realtime};

fn main() -> Result<()> {
    let logger = root_logger();

    let mut conn = Connection::new_system().context("Could not connect to system D-Bus")?;

    for unit in &["systemd-tmpfiles-clean.timer", "shadow.timer"] {
        let next_activation = next_activation(&logger, &conn, unit)?.with_timezone(&Local);
        info!(&logger, "{} will next run at {}", unit, next_activation);
    }

    let monitor = PowerMonitor::new(
        logger.clone(),
        "Night Kitchen Scheduler",
        "Scheduling next system wakeup",
        move |ev| {
            match ev {
                PowerEvent::PostSleep => {
                    if let Err(err) = update_resume_timestamp(&logger) {
                        error!(&logger, "Could not update resume timestamp: {}", err; "error" => ?err);
                    }
                }
                _ => (),
            };
        },
    );

    // TODO: on PreShutdown, figure out when next RTC alarm should be (don't clobber if there's a sooner one)

    PowerMonitor::register(&mut conn, monitor)?;

    let shutdown = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGTERM, shutdown.clone())
        .context("Could not add SIGTERM hook")?;

    while !shutdown.load(Ordering::SeqCst) {
        conn.process(Duration::from_secs(60))?;
    }

    Ok(())
}

fn update_resume_timestamp(logger: &Logger) -> Result<()> {
    let timestamp = Utc::now();
    let timestamp_file = resume_timestamp_file();
    debug!(&logger, "Writing resume timestamp"; "timestamp" => %timestamp, "file" => %timestamp_file.display());

    let mut f = File::create(&timestamp_file)
        .with_context(|| format!("Could not create {}", timestamp_file.display()))?;
    // Write as a string for debuggability
    write!(&mut f, "{}", timestamp.timestamp_millis())
        .context("Could not write to timestamp file")?;

    Ok(())
}

fn next_activation(logger: &Logger, conn: &Connection, timer_unit: &str) -> Result<DateTime<Utc>> {
    let timer = systemd_unit(conn, timer_unit)?;

    // If either is 0, that means the timer doesn't include any events using the corresponding clock
    let next_realtime = match timer
        .next_elapse_usec_realtime()
        .context("Could not get next CLOCK_REALTIME elapsation point")?
    {
        0 => None,
        realtime_usecs => {
            let next_realtime = from_timestamp_usecs(realtime_usecs);
            debug!(&logger, "Next CLOCK_REALTIME elapsation point is {}", next_realtime; "unit" => timer_unit);
            Some(next_realtime)
        }
    };

    let next_monotonic = match timer
        .next_elapse_usec_monotonic()
        .context("Could not get next monotonic elapsation point")?
    {
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
        (Some(next_realtime), Some(next_monotonic)) => Some(next_realtime.min(next_monotonic)),
    };

    next_elapse.ok_or_else(|| anyhow!("Neither monotonic nor realtime next elapsation point"))
}
