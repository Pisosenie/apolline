#!/usr/bin/env bash
# A fake NetworkManager, BlueZ, fprintd and power-profiles-daemon on a bus of their own, for trying
# Wi-Fi, Bluetooth, the fingerprint reader and power profiles without them. Needs python-dbusmock.
# Prints how to start apolline against them and where the calls are logged (with what was typed,
# so no real passwords); Ctrl+C stops everything.
set -euo pipefail

here=$(dirname "$(realpath "$0")")
socket=${XDG_RUNTIME_DIR:?needs a runtime directory of your own}/apolline-fake-bus
rm -f "$socket"
trap 'kill $(jobs -p) 2>/dev/null; rm -f "$socket" "$socket"-*.log "$socket.conf"' EXIT
trap 'exit 130' INT TERM
# A session bus that also lets root in, for the fake fingerprint reader.
cat > "$socket.conf" <<CONF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <include>/usr/share/dbus-1/session.conf</include>
  <policy context="default"><allow user="$(id -un)"/><allow user="root"/></policy>
</busconfig>
CONF
dbus-daemon --config-file="$socket.conf" --nofork --address="unix:path=$socket" &
for _ in $(seq 50); do
    [ -S "$socket" ] && break
    kill -0 $! 2>/dev/null || { echo "dbus-daemon didn't start" >&2; exit 1; }
    sleep 0.1
done

export DBUS_SYSTEM_BUS_ADDRESS=unix:path=$socket
for fake in networkmanager bluez; do
    python3 -m dbusmock --system -t "$here/fake-bus/$fake.py" > "$socket-$fake.log" 2>&1 &
done
# The lock believes only an fprintd running as root.
if sudo -n true 2>/dev/null; then
    sudo -n env DBUS_SYSTEM_BUS_ADDRESS="$DBUS_SYSTEM_BUS_ADDRESS" \
        python3 -m dbusmock --system -t "$here/fake-bus/fprintd.py" > "$socket-fprintd.log" 2>&1 &
else
    echo "no fake fingerprint reader: it has to run as root, and sudo wants a password" >&2
fi
python3 -m dbusmock --system -t upower_power_profiles_daemon > "$socket-profiles.log" 2>&1 &
echo "DBUS_SYSTEM_BUS_ADDRESS=$DBUS_SYSTEM_BUS_ADDRESS apolline"
echo "calls: $socket-*.log"
wait
