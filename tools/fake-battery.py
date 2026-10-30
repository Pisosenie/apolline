#!/usr/bin/env python3
"""A fake UPower battery for checking the bar, run as root with upower stopped:
tools/fake-battery.py PERCENT [UNPLUG_AFTER_SECONDS]"""

import sys

from gi.repository import Gio, GLib

INTROSPECTION = """
<node>
  <interface name="org.freedesktop.UPower.Device">
    <property name="IsPresent" type="b" access="read"/>
    <property name="Percentage" type="d" access="read"/>
    <property name="State" type="u" access="read"/>
  </interface>
</node>
"""
PATH = "/org/freedesktop/UPower/devices/DisplayDevice"
CHARGING, DISCHARGING = 1, 2

percent = float(sys.argv[1])
unplug_after = int(sys.argv[2]) if len(sys.argv) > 2 else 0
state = [CHARGING]


def get_property(connection, sender, path, interface, name):
    if name == "IsPresent":
        return GLib.Variant("b", True)
    if name == "Percentage":
        return GLib.Variant("d", percent)
    return GLib.Variant("u", state[0])


def unplug(connection):
    state[0] = DISCHARGING
    changed = GLib.Variant("(sa{sv}as)", (
        "org.freedesktop.UPower.Device", {"State": GLib.Variant("u", DISCHARGING)}, [],
    ))
    connection.emit_signal(None, PATH, "org.freedesktop.DBus.Properties", "PropertiesChanged", changed)
    return False


def on_bus(connection, name):
    for interface in Gio.DBusNodeInfo.new_for_xml(INTROSPECTION).interfaces:
        connection.register_object(PATH, interface, None, get_property, None)
    if unplug_after:
        GLib.timeout_add_seconds(unplug_after, unplug, connection)


Gio.bus_own_name(Gio.BusType.SYSTEM, "org.freedesktop.UPower", Gio.BusNameOwnerFlags.REPLACE, on_bus, None, None)
GLib.MainLoop().run()
