//! Simple server to expose suspend/resume information to the runner
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use dbus::blocking::Connection;
use dbus::tree::Factory;

/// Representation of server state, for updating
pub struct Server {
    resume_timestamp: AtomicI64,
}

impl Server {
    fn new() -> Server {
        Server {
            resume_timestamp: AtomicI64::new(0)
        }
    }

    /// Update the timestamp for the last resume from suspend
    pub fn set_resume_timestamp(&self, timestamp: DateTime<Utc>) {
        self.set_resume_timestamp_raw(timestamp.timestamp_millis());
    }

    /// Update the timestamp for the last resume from suspend, as milliseconds since the UNIX epoch
    pub fn set_resume_timestamp_raw(&self, timestamp: i64) {
        self.resume_timestamp.store(timestamp, Ordering::Relaxed);
    }

    /// Get the currently-stored resume timestamp as milliseconds since the UNIX epoch
    pub fn resume_timestamp_raw(&self) -> i64 {
        self.resume_timestamp.load(Ordering::Relaxed)
    }
}

pub fn add_server(conn: &Connection) -> Result<Arc<Server>> {
    conn.request_name("com.bennavetta.nightkitchen.scheduler", false, true, false).context("Could not request name from D-Bus")?;

    let server = Arc::new(Server::new());
    let server2 = server.clone();

    let f = Factory::new_fn::<()>();
    let tree = f.tree(()).add(f.object_path("/", ()).introspectable().add(
        f.interface("com.bennavetta.nightkitchen.Scheduler", ()).add_m(
            f.method("ResumeTimestamp", (), move |m| {
                let ret = m.msg.method_return().append1(server.resume_timestamp_raw());
                Ok(vec![ret])
            }).outarg::<i64, _>("timestamp")
        )
    ));

    tree.start_receive(conn);

    Ok(server2)
}