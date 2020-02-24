mod session;

use slog::info;

use night_kitchen::root_logger;
use crate::session::SessionClient;

fn main() -> anyhow::Result<()> {
    let logger = root_logger();

    let sc = SessionClient::new(&logger)?;
    info!(&logger, "Session ID is {}", sc.session_id()?);

    if sc.has_other_sessions()? {
        info!(&logger, "No other sessions are running");
    } else {
        info!(&logger, "Other sessions are running");
    }

    Ok(())
}
