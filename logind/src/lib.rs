use std::fmt;
use std::time::Duration;

use anyhow::{Context, Result};
use dbus::blocking::{Connection, Proxy};
use dbus::arg::OwnedFd;
use itertools::Itertools;
use slog::{o, debug, Logger};

use bindings::OrgFreedesktopLogin1Manager;

mod bindings;

pub struct LoginManager {
    logger: Logger,
    connection: Connection,
}

impl LoginManager {
    pub fn new(parent_logger: &Logger) -> Result<LoginManager> {
        let connection = Connection::new_system().context("Could not connect to system D-Bus")?;

        Ok(LoginManager {
            logger: parent_logger.new(o!("component" => "logind")),
            connection,
        })
    }

    pub fn has_sessions(&self) -> Result<bool> {
        debug!(self.logger, "Getting session list");
        let sessions = self
            .proxy()
            .list_sessions()
            .context("Could not enumerate login sessions")?;

        for session in sessions.iter() {
            let (session_id, user_id, user_name, seat_id, _) = session;
            debug!(
                self.logger,
                "Found session {session_id} for user {user_name} (uid {user_id}) on seat {seat_id}",
                session_id = session_id,
                user_id = user_id,
                user_name = user_name,
                seat_id = seat_id
            );
        }

        Ok(!sessions.is_empty())
    }

    /// Take a lock inhibiting the operations specified by `types`. 
    #[must_use]
    pub fn inhibit<I: IntoIterator<Item=InhibitorLockType>>(&self, types: I, mode: InhibitorLockMode, who: &str, why: &str) -> Result<InhibitorLock> {
        let what = types.into_iter().map(|t| t.type_str()).join(":");

        debug!(self.logger, "Taking inhibitor lock"; "what" => &what, "mode" => mode, "who" => who, "why" => why);

        let fd = self.proxy().inhibit(
            &what,
            who,
            why,
            mode.mode_str()
        ).context("Could not take inhibitor lock")?;

        debug!(self.logger, "Got inhibitor lock with fd {:?}", fd);

        Ok(InhibitorLock {
            fd,
            what,
            mode
        })
    }

    fn proxy<'a>(&'a self) -> Proxy<'a, &'a Connection> {
        self.connection.with_proxy(
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            Duration::from_millis(500),
        )
    }
}

#[derive(Debug)]
pub struct InhibitorLock {
    /// File descriptor representing the lock. Closing it releases the inhibitor lock
    fd: OwnedFd, // OwnedFd closes the file descriptor on drop

    // Included for context

    what: String,
    mode: InhibitorLockMode
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum InhibitorLockMode {
    /// Blocking inhibitor locks prevent the inhibited operations entirely. While the lock is held, those operations will
    /// fail unless the lock is overridden.
    Block,

    /// Delay inhibitor locks temporarily prevent the operation.
    Delay
}

impl InhibitorLockMode {
    fn mode_str(&self) -> &'static str {
        use InhibitorLockMode::*;
        match *self {
            Block => "block",
            Delay => "delay"
        }
    }
}

impl fmt::Display for InhibitorLockMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.mode_str())
    }
}

impl slog::Value for InhibitorLockMode {
  fn serialize(&self, _record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result {
      serializer.emit_str(key, self.mode_str())
  }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum InhibitorLockType {
    Sleep,
    Shutdown,
    Idle,
    HandlePowerKey,
    HandleSuspendKey,
    HandleHibernateKey,
    HandleLidSwitch
}

impl InhibitorLockType {
    fn type_str(&self) -> &'static str {
        use InhibitorLockType::*;
        match *self {
            Sleep => "sleep",
            Shutdown => "shutdown",
            Idle => "idle",
            HandlePowerKey => "handle-power-key",
            HandleSuspendKey => "handle-suspend-key",
            HandleHibernateKey => "handle-hibernate-key",
            HandleLidSwitch => "handle-lid-switch"
        }
    }
}

impl fmt::Display for InhibitorLockType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.type_str())
    }
}

impl slog::Value for InhibitorLockType {
    fn serialize(&self, _record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result {
        serializer.emit_str(key, self.type_str())
    }
  }
  