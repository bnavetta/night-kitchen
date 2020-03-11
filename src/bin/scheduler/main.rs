#[macro_use]
extern crate nix;

use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use dbus::blocking::Connection;
use signal_hook;
use slog::{debug, error, info, warn, Logger};

mod power_monitor;
mod rtcwake;
mod time;

use night_kitchen::dbus::systemd_timer::OrgFreedesktopSystemd1Timer;
use night_kitchen::dbus::systemd_unit;
use night_kitchen::{resume_timestamp_file, root_logger};

use crate::power_monitor::{PowerEvent, PowerMonitor};
use crate::rtcwake::Rtc;
use crate::time::{from_timestamp_usecs, monotonic_to_realtime};

const TIMER_UNITS: &[&str] = &["night-kitchen-daily.timer", "night-kitchen-weekly.timer"];

fn main() -> Result<()> {
    let logger = root_logger();

    let mut conn = Connection::new_system().context("Could not connect to system D-Bus")?;

    let monitor = PowerMonitor::new(
        logger.clone(),
        "Night Kitchen Scheduler",
        "Scheduling next system wakeup",
        move |conn, ev| {
            match ev {
                PowerEvent::PostSleep => {
                    if let Err(err) = update_resume_timestamp(&logger) {
                        error!(&logger, "Could not update resume timestamp: {:?}", err);
                    }
                }
                PowerEvent::PreShutdown => {
                    // Find the soonest activation time across all night kitchen timers
                    let alarm_time = TIMER_UNITS
                        .iter()
                        .map(|unit| next_activation(&logger, conn, unit))
                        .fold(None, |acc, time| match (acc, time) {
                            (_, Err(e)) => {
                                warn!(&logger, "Could not get timer activation time: {:?}", e);
                                acc
                            }
                            (None, Ok(time)) => Some(time),
                            (Some(prev_time), Ok(time)) => Some(prev_time.min(time)),
                        });

                    if let Some(alarm_time) = alarm_time {
                        info!(&logger, "Next timer activation is at {}", alarm_time);
                        match set_wake_alarm(&logger, &alarm_time) {
                            Ok(_) => info!(&logger, "Scheduled wake alarm"),
                            Err(e) => error!(&logger, "Could not set wake alarm: {:?}", e),
                        }
                    }
                }
                _ => (),
            };
        },
    );

    PowerMonitor::register(&conn, monitor)?;

    let shutdown = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGTERM, shutdown.clone())
        .context("Could not add SIGTERM hook")?;

    while !shutdown.load(Ordering::SeqCst) {
        conn.process(Duration::from_secs(1))?;
    }

    Ok(())
}

fn set_wake_alarm(logger: &Logger, alarm_time: &DateTime<Utc>) -> Result<()> {
    info!(&logger, "Setting RTC alarm for {}", alarm_time);
    let rtc = Rtc::new()?;
    let clock_mode = Rtc::read_clock_mode().context("Could not get hardware clock mode")?;

    let mut alarm_config = rtc.alarm_configuration()?;
    if alarm_config.enabled() {
        let current_alarm = clock_mode.to_datetime(&alarm_config.time());
        if &current_alarm < alarm_time {
            debug!(
                &logger,
                "Will not override earlier alarm at {}", current_alarm
            );
        } else {
            debug!(&logger, "Overriding later alarm at {}", current_alarm);
            alarm_config.set_time(&clock_mode.to_hardware(alarm_time));
        }
    } else {
        debug!(&logger, "No previous alarm set");
        alarm_config.set_enabled(true);
        alarm_config.set_time(&clock_mode.to_hardware(alarm_time));
    }

    rtc.set_alarm_configuration(&alarm_config)?;

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
