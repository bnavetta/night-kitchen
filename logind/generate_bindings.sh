#!/bin/bash

# Generate bindings for the login manager D-Bus API
# See https://www.freedesktop.org/wiki/Software/systemd/logind/ for the Manager documentation

dbus-codegen-rust -s \
    -d org.freedesktop.login1 \
    -p /org/freedesktop/login1 \
    -f org.freedesktop.login1.Manager \
    -c blocking -m None \
    -o src/bindings.rs

# TODO: automate this
echo "The create_session method must be removed, since dbus-rs only supports up to 10 arguments"
