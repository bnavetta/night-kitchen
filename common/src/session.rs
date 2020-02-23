use std::env;
use std::time::Duration;

use anyhow::{Result, Context};
use dbus::blocking::Connection;
use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use libc;
use slog::{Logger, o, warn, debug};

use crate::bindings::OrgFreedesktopLogin1Manager;
use super::login_manager;

/// A client to the login daemon for getting information about user sessions.
/// It uses the systemd-logind D-Bus API documented [here](https://www.freedesktop.org/wiki/Software/systemd/logind/).
pub struct SessionClient {
    connection: Connection,
    logger: Logger,
}

impl SessionClient {
    pub fn new(logger: &Logger) -> Result<SessionClient> {
        let connection = Connection::new_system().context("Could not connect to system D-Bus")?;
        let name = connection.unique_name().to_string();
        Ok(SessionClient {
            connection,
            logger: logger.new(o!("dbus-connection-name" => name))
        })
    }

    /// Returns the session ID for the current process. This will attempt to look it up from systemd-logind, and if that fails,
    /// fall back to the XDG_SESSION_ID environment variable.
    pub fn session_id(&self) -> Result<String> {
        self.session_id_of_pid(get_pid()).or_else(|err| {
            warn!(&self.logger, "Could not get session ID from D-Bus, falling back to XDG_SESSION_ID"; "error" => ?err);
            env::var("XDG_SESSION_ID").context("Could not get session ID from environment")
        })
    }

    /// Returns the ID of the session the given process belongs to
    pub fn session_id_of_pid(&self, pid: u32) -> Result<String> {
        let manager = login_manager(&self.connection);

        let session_path = manager.get_session_by_pid(pid).with_context(|| format!("Could not get session for process {}", pid))?;
        debug!(&self.logger, "Found session path for process ID"; "path" => session_path.to_string(), "pid" => pid);

        let session_proxy = self.connection.with_proxy(
            "org.freedesktop.login1",
            session_path,
            Duration::from_millis(500)
        );

        let session_id: String = session_proxy.get("org.freedesktop.login1.Session", "Id").context("Could not get ID from session")?;
        debug!(&self.logger, "Looked up ID from session path"; "session_id" => &session_id);
        Ok(session_id)
    }

    /// Returns `true` if there are other sessions running on the system besides the current one
    pub fn has_other_sessions(&self) -> Result<bool> {
        let manager = login_manager(&self.connection);
        let sessions = manager.list_sessions().context("Could not list sessions")?;

        for (session_id, user_id, user_name, seat_id, _) in sessions.iter() {
            debug!(&self.logger, "Found session"; "session_id" => session_id, "user_id" => user_id, "user_name" => user_name, "seat_id" => seat_id);
        }

        let other_session = match self.session_id() {
            Ok(our_id) => sessions.iter().any(|sess| {
                sess.0 == our_id
            }),
            Err(_) =>
                // If we aren't part of a session, then return true if there are _any_ sessions
                !sessions.is_empty()
        };

        Ok(other_session)
    }
}

/// Helper to get the current process ID. This is just a wrapper around `libc::getpid` with
/// a cast to the type the D-Bus API expects.
fn get_pid() -> u32 {
    unsafe {
        libc::getpid() as u32
    }
}