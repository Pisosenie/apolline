#!/usr/bin/env python3
"""A tray icon with a menu, for trying the bar's app icons: registers a StatusNotifierItem with
whatever watcher is running and logs what the shell asks of it. Usage: fake-tray-app.py [ICON]"""
import sys

import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

ITEM = "org.kde.StatusNotifierItem"
MENU = "com.canonical.dbusmenu"
ICON = sys.argv[1] if len(sys.argv) > 1 else "mpv"


class Item(dbus.service.Object):
    def __init__(self, bus):
        super().__init__(bus, "/StatusNotifierItem")
        self.status = "Active"

    @dbus.service.method(dbus.PROPERTIES_IFACE, in_signature="ss", out_signature="v")
    def Get(self, interface, name):
        return self.GetAll(interface)[name]

    @dbus.service.method(dbus.PROPERTIES_IFACE, in_signature="s", out_signature="a{sv}")
    def GetAll(self, interface):
        return {
            "Id": "fake-tray-app",
            "Title": "Fake tray app",
            "Status": self.status,
            "IconName": ICON,
            "ItemIsMenu": dbus.Boolean(False),
            "Menu": dbus.ObjectPath("/Menu"),
            "ToolTip": dbus.Struct(("", dbus.Array([], signature="(iiay)"), "Fake tray app", ""), signature=None),
        }

    @dbus.service.method(ITEM, in_signature="ii")
    def Activate(self, x, y):
        print("activate", flush=True)
        self.status = "NeedsAttention" if self.status == "Active" else "Active"
        self.NewStatus(self.status)

    @dbus.service.method(ITEM, in_signature="ii")
    def SecondaryActivate(self, x, y):
        print("secondary", flush=True)

    @dbus.service.signal(ITEM, signature="s")
    def NewStatus(self, status):
        pass


class Menu(dbus.service.Object):
    ENTRIES = [(1, "_Open the window", "", False), (2, "", "separator", False), (3, "Keep running", "", True), (4, "_Quit", "", False)]

    def __init__(self, bus):
        super().__init__(bus, "/Menu")

    @dbus.service.method(MENU, in_signature="i", out_signature="b")
    def AboutToShow(self, parent):
        return False

    @dbus.service.method(MENU, in_signature="iias", out_signature="u(ia{sv}av)")
    def GetLayout(self, parent, depth, names):
        children = []
        for ident, label, kind, toggle in self.ENTRIES:
            props = {"label": dbus.String(label, variant_level=1)}
            if kind:
                props["type"] = dbus.String(kind, variant_level=1)
            if toggle:
                props["toggle-type"] = dbus.String("checkmark", variant_level=1)
                props["toggle-state"] = dbus.Int32(1, variant_level=1)
            child = dbus.Struct(
                (dbus.Int32(ident), dbus.Dictionary(props, signature="sv"), dbus.Array([], signature="v")),
                signature="ia{sv}av",
                variant_level=1,
            )
            children.append(child)
        root = dbus.Struct(
            (dbus.Int32(0), dbus.Dictionary({}, signature="sv"), dbus.Array(children, signature="v")),
            signature="ia{sv}av",
        )
        return dbus.UInt32(1), root

    @dbus.service.method(MENU, in_signature="isvu")
    def Event(self, ident, kind, data, stamp):
        print(f"menu {ident} {kind}", flush=True)


def main():
    DBusGMainLoop(set_as_default=True)
    bus = dbus.SessionBus()
    item, menu = Item(bus), Menu(bus)
    watcher = bus.get_object("org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher")
    watcher.RegisterStatusNotifierItem(bus.get_unique_name(), dbus_interface="org.kde.StatusNotifierWatcher")
    print("registered", bus.get_unique_name(), flush=True)
    GLib.MainLoop().run()


main()
