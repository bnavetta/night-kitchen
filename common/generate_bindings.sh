#!/bin/bash

# Generate bindings for systemd D-Bus APIs
# See https://www.freedesktop.org/wiki/Software/systemd/dbus/ for systemd documentation
# See https://www.freedesktop.org/wiki/Software/systemd/logind/ for the logind Manager documentation

dbus-codegen-rust -s \
    -d org.freedesktop.systemd1 \
    -p /org/freedesktop/systemd1 \
    -f org.freedesktop.systemd1.Manager \
    -c blocking -m None \
    -o src/systemd1.rs

dbus-codegen-rust -s \
    -d org.freedesktop.systemd1 \
    -p /org/freedesktop/systemd1/unit/shadow_2etimer \
    -f org.freedesktop.systemd1.Timer \
    -c blocking -m None \
    -o src/systemd_timer.rs

dbus-codegen-rust -s \
    -d org.freedesktop.login1 \
    -p /org/freedesktop/login1 \
    -f org.freedesktop.login1.Manager \
    -c blocking -m None \
    -o src/login1.rs

# TODO: automate this
echo "The create_session method must be removed, since dbus-rs only supports up to 10 arguments"
