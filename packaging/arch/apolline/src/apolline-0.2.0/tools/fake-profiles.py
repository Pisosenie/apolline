#!/usr/bin/env python3
"""A fake power-profiles-daemon with a performance profile, run as root with the real one stopped:
tools/fake-profiles.py"""

from gi.repository import Gio, GLib

INTROSPECTION = """
<node>
  <interface name="org.freedesktop.UPower.PowerProfiles">
    <property name="ActiveProfile" type="s" access="readwrite"/>
    <property name="Profiles" type="aa{sv}" access="read"/>
  </interface>
</node>
"""
PATH = "/org/freedesktop/UPower/PowerProfiles"
NAMES = ["power-saver", "balanced", "performance"]
active = ["balanced"]


def get_property(connection, sender, path, interface, name):
    if name == "ActiveProfile":
        return GLib.Variant("s", active[0])
    return GLib.Variant("aa{sv}", [{"Profile": GLib.Variant("s", n), "Driver": GLib.Variant("s", "fake")} for n in NAMES])


def set_property(connection, sender, path, interface, name, value):
    active[0] = value.unpack()
    print(f"ActiveProfile -> {active[0]}", flush=True)
    return True


def on_bus(connection, name):
    for interface in Gio.DBusNodeInfo.new_for_xml(INTROSPECTION).interfaces:
        connection.register_object(PATH, interface, None, get_property, set_property)


Gio.bus_own_name(Gio.BusType.SYSTEM, "org.freedesktop.UPower.PowerProfiles", Gio.BusNameOwnerFlags.REPLACE, on_bus, None, None)
GLib.MainLoop().run()
