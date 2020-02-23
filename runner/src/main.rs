use slog::info;

use night_kitchen_common::{SessionClient, root_logger};

fn main() -> anyhow::Result<()> {
    let logger = root_logger(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    let sc = SessionClient::new(&logger)?;
    info!(&logger, "Session ID is {}", sc.session_id()?);

    if sc.has_other_sessions()? {
        info!(&logger, "No other sessions are running");
    } else {
        info!(&logger, "Other sessions are running");
    }

    Ok(())
}
