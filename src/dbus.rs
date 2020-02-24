//! D-Bus bindings generated by [dbus-codegen-rust](https://github.com/diwic/dbus-rs/tree/master/dbus-codegen)
use std::time::Duration;

use dbus::blocking::{Connection, Proxy};

pub mod logind;
pub mod systemd;
pub mod systemd_timer;

const PROXY_TIMEOUT: Duration = Duration::from_millis(500);

/// Creates a D-Bus connection proxy referring to the systemd-logind manager API object
pub fn login_manager<'a>(connection: &'a Connection) -> Proxy<'a, &'a Connection> {
    connection.with_proxy(
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        PROXY_TIMEOUT,
    )
}

/// Creates a D-Bus connection proxy referring to the systemd manager API object
pub fn systemd_manager<'a>(connection: &'a Connection) -> Proxy<'a, &'a Connection> {
    connection.with_proxy(
        "org.freedesktop.systemd1",
        "/org/freedesktop.systemd1",
        PROXY_TIMEOUT
    )
}