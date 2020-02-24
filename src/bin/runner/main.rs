use anyhow::Result;
use slog::info;

use night_kitchen::root_logger;
use crate::session::SessionClient;
use crate::systemd::start_unit;

mod session;
mod systemd;

fn main() -> Result<()> {
    let logger = root_logger();

    // TODO: checking for sessions probably won't actually work
    // - if system were shut down and booted from RTC alarm, won't be any sessions
    // - if system were suspended and woken by systemd, probably will be sessions
    // Possible option: when scheduler gets PostSleep notification, records timestamp
    // runner can then check this timestamp (either D-Bus or file)

    // TODO: start configured unit (via dbus)

    start_unit(&logger, &mut dbus::blocking::Connection::new_system()?, "atop.service")?;

    let sc = SessionClient::new(&logger)?;
    info!(&logger, "Session ID is {}", sc.session_id()?);

    if sc.has_other_sessions()? {
        info!(&logger, "No other sessions are running");
    } else {
        info!(&logger, "Other sessions are running");
    }

    Ok(())
}

