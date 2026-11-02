"""NetworkManager with one Wi-Fi card and a few networks around; tools/fake-bus.sh starts it.

It starts on "Polina", which is saved. Every locked network takes the password "polina";
a wrong one fails a second later the way NetworkManager does, for want of secrets. "Office" is
saved as a home network but now asks for a work login, the way a look-alike would.
"""

import uuid

import dbus
import dbusmock
from gi.repository import GLib

BUS_NAME = "org.freedesktop.NetworkManager"
MAIN_OBJ = "/org/freedesktop/NetworkManager"
MAIN_IFACE = "org.freedesktop.NetworkManager"
SYSTEM_BUS = True

DEVICE = MAIN_OBJ + "/Devices/1"
SETTINGS = MAIN_OBJ + "/Settings"
WIRELESS = "org.freedesktop.NetworkManager.Device.Wireless"
POINT = "org.freedesktop.NetworkManager.AccessPoint"
PROFILE = "org.freedesktop.NetworkManager.Settings.Connection"
ACTIVE = "org.freedesktop.NetworkManager.Connection.Active"

PSK, SAE, EAP = 0x188, 0x400, 0x288
NETWORKS = [
    ("Polina", 82, PSK),
    ("Neighbours", 64, PSK),
    ("Home 6E", 58, SAE),
    ("Office", 52, EAP),
    ("Cafe: Free", 41, 0),
]
PASSWORD = "polina"
ACTIVATING, ACTIVATED, DEACTIVATED = 1, 2, 4
# The device says why it failed; the active connection only that the device went away.
FAILED, NO_SECRETS, DEVICE_DISCONNECTED = 120, 7, 3

count = [0]


def load(mock, _parameters):
    mock.AddProperties(
        MAIN_IFACE,
        {
            "WirelessEnabled": dbus.Boolean(True),
            "WirelessHardwareEnabled": dbus.Boolean(True),
        },
    )
    mock.AddMethods(
        MAIN_IFACE,
        [
            ("GetDevices", "", "ao", lambda _self: [dbus.ObjectPath(DEVICE)]),
            ("ActivateConnection", "ooo", "o", lambda _self, profile, _dev, point: activate(profile, point)),
            ("AddAndActivateConnection", "a{sa{sv}}oo", "oo", add_and_activate),
        ],
    )
    mock.AddObject(DEVICE, "org.freedesktop.NetworkManager.Device", {"DeviceType": dbus.UInt32(2)}, [])
    points = []
    for index, (ssid, strength, keys) in enumerate(NETWORKS):
        path = f"{MAIN_OBJ}/AccessPoint/{index + 1}"
        mock.AddObject(
            path,
            POINT,
            {
                "Ssid": dbus.ByteArray(ssid.encode()),
                "Strength": dbus.Byte(strength),
                "Flags": dbus.UInt32(1 if keys else 0),
                "WpaFlags": dbus.UInt32(0),
                "RsnFlags": dbus.UInt32(keys),
            },
            [],
        )
        points.append(dbus.ObjectPath(path))
    dbusmock.get_object(DEVICE).AddProperties(
        WIRELESS,
        {
            "AccessPoints": dbus.Array(points, signature="o"),
            "ActiveAccessPoint": dbus.ObjectPath("/"),
        },
    )
    dbusmock.get_object(DEVICE).AddMethods(WIRELESS, [("RequestScan", "a{sv}", "", lambda _self, _options: None)])
    mock.AddObject(
        SETTINGS,
        "org.freedesktop.NetworkManager.Settings",
        {"Connections": dbus.Array([], signature="o")},
        [("ListConnections", "", "ao", profiles)],
    )
    saved = save("Polina", PASSWORD, PSK)
    save("Office", PASSWORD, PSK)
    finish(activate(saved, points[0]), ACTIVATED, 0)


def profiles(_self):
    return dbus.Array([dbus.ObjectPath(p) for p in dbusmock.mockobject.objects if p.startswith(SETTINGS + "/")], signature="o")


def save(ssid, password, keys):
    count[0] += 1
    path = f"{SETTINGS}/{count[0]}"
    settings = {
        "connection": {"id": ssid, "type": "802-11-wireless", "uuid": str(uuid.uuid4())},
        "802-11-wireless": {"ssid": dbus.ByteArray(ssid.encode())},
    }
    if keys:
        settings["802-11-wireless-security"] = {"key-mgmt": {PSK: "wpa-psk", SAE: "sae", EAP: "wpa-eap"}[keys]}
    dbusmock.get_object(DEVICE).AddObject(
        path,
        PROFILE,
        {},
        [
            ("GetSettings", "", "a{sa{sv}}", lambda _self: settings),
            ("Delete", "", "", lambda self: forget(self.path)),
        ],
    )
    dbusmock.get_object(path).password = password
    dbusmock.get_object(SETTINGS).Set("org.freedesktop.NetworkManager.Settings", "Connections", profiles(None))
    return path


def forget(path):
    device = dbusmock.get_object(DEVICE)
    for active in dbusmock.mockobject.objects.values():
        if active.props.get(ACTIVE, {}).get("Connection") == path:
            device.Set(WIRELESS, "ActiveAccessPoint", dbus.ObjectPath("/"))
    device.RemoveObject(path)
    dbusmock.get_object(SETTINGS).Set("org.freedesktop.NetworkManager.Settings", "Connections", profiles(None))


def add_and_activate(_self, settings, _device, point):
    props = dbusmock.get_object(point).props[POINT]
    security = settings.get("802-11-wireless-security", {})
    path = save(bytes(props["Ssid"]).decode(), str(security.get("psk", "")), int(props["RsnFlags"]))
    return (dbus.ObjectPath(path), activate(path, point))


def activate(profile, point):
    count[0] += 1
    path = f"{MAIN_OBJ}/ActiveConnection/{count[0]}"
    dbusmock.get_object(DEVICE).AddObject(
        path,
        ACTIVE,
        {"State": dbus.UInt32(ACTIVATING), "Connection": dbus.ObjectPath(profile)},
        [],
    )
    rsn = dbusmock.get_object(point).props[POINT]["RsnFlags"]
    known = dbusmock.get_object(profile).password
    fits = rsn == 0 or known == PASSWORD

    def later():
        if fits:
            dbusmock.get_object(DEVICE).Set(WIRELESS, "ActiveAccessPoint", dbus.ObjectPath(point))
        if not fits:
            dbusmock.get_object(DEVICE).EmitSignal(
                "org.freedesktop.NetworkManager.Device", "StateChanged", "uuu", [FAILED, 50, NO_SECRETS]
            )
        finish(path, ACTIVATED if fits else DEACTIVATED, 0 if fits else DEVICE_DISCONNECTED)
        return False

    GLib.timeout_add(1000, later)
    return dbus.ObjectPath(path)


def finish(path, state, reason):
    active = dbusmock.get_object(path)
    active.UpdateProperties(ACTIVE, {"State": dbus.UInt32(state)})
    active.EmitSignal(ACTIVE, "StateChanged", "uu", [dbus.UInt32(state), dbus.UInt32(reason)])
