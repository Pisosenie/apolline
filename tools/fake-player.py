#!/usr/bin/env python3
"""A fake MPRIS player for checking the bar: tools/fake-player.py TITLE ARTIST [PAUSE_AFTER_SECONDS]"""

import sys

from gi.repository import Gio, GLib

INTROSPECTION = """
<node>
  <interface name="org.mpris.MediaPlayer2">
    <property name="Identity" type="s" access="read"/>
  </interface>
  <interface name="org.mpris.MediaPlayer2.Player">
    <property name="PlaybackStatus" type="s" access="read"/>
    <property name="Metadata" type="a{sv}" access="read"/>
  </interface>
</node>
"""
PATH = "/org/mpris/MediaPlayer2"

title, artist = sys.argv[1], sys.argv[2]
pause_after = int(sys.argv[3]) if len(sys.argv) > 3 else 0
status = ["Playing"]


def get_property(connection, sender, path, interface, name):
    if name == "Identity":
        return GLib.Variant("s", "Fake")
    if name == "PlaybackStatus":
        return GLib.Variant("s", status[0])
    return GLib.Variant("a{sv}", {
        "xesam:title": GLib.Variant("s", title),
        "xesam:artist": GLib.Variant("as", [artist]),
    })


def pause(connection):
    status[0] = "Paused"
    changed = GLib.Variant("(sa{sv}as)", (
        "org.mpris.MediaPlayer2.Player", {"PlaybackStatus": GLib.Variant("s", "Paused")}, [],
    ))
    connection.emit_signal(None, PATH, "org.freedesktop.DBus.Properties", "PropertiesChanged", changed)
    return False


def on_bus(connection, name):
    for interface in Gio.DBusNodeInfo.new_for_xml(INTROSPECTION).interfaces:
        connection.register_object(PATH, interface, None, get_property, None)
    if pause_after:
        GLib.timeout_add_seconds(pause_after, pause, connection)


Gio.bus_own_name(Gio.BusType.SESSION, "org.mpris.MediaPlayer2.fake", Gio.BusNameOwnerFlags.NONE, on_bus, None, None)
GLib.MainLoop().run()
