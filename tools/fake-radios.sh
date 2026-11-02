#!/usr/bin/env bash
# A fake NetworkManager and BlueZ on a bus of their own, for trying Wi-Fi and Bluetooth in quick
# settings on a machine without either. Needs python-dbusmock. Prints how to start apolline
# against them and where the calls are logged (with what was typed, so no real passwords);
# Ctrl+C stops everything.
set -euo pipefail

here=$(dirname "$(realpath "$0")")
socket=${XDG_RUNTIME_DIR:?needs a runtime directory of your own}/apolline-fake-radios
rm -f "$socket"
trap 'kill $(jobs -p) 2>/dev/null; rm -f "$socket" "$socket"-*.log' EXIT
trap 'exit 130' INT TERM
dbus-daemon --session --nofork --address="unix:path=$socket" &
for _ in $(seq 50); do
    [ -S "$socket" ] && break
    kill -0 $! 2>/dev/null || { echo "dbus-daemon didn't start" >&2; exit 1; }
    sleep 0.1
done

export DBUS_SYSTEM_BUS_ADDRESS=unix:path=$socket
for fake in networkmanager bluez; do
    python3 -m dbusmock --system -t "$here/fake-radios/$fake.py" > "$socket-$fake.log" 2>&1 &
done
echo "DBUS_SYSTEM_BUS_ADDRESS=$DBUS_SYSTEM_BUS_ADDRESS apolline"
echo "calls: $socket-*.log"
wait
