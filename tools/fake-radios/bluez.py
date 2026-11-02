"""BlueZ with one adapter and a few devices; tools/fake-radios.sh starts it.

The headphones are paired and connected, the mouse is paired but switched off. Looking for
devices finds three more after a second, and pairing asks the shell's agent the way real ones
do: the phone wants its code confirmed, the keyboard shows a code to type (and "types" it four
seconds later), the speaker wants PIN 0000.
"""

import dbus
import dbusmock
from gi.repository import GLib

BUS_NAME = "org.bluez"
MAIN_OBJ = "/"
MAIN_IFACE = "org.freedesktop.DBus.ObjectManager"
SYSTEM_BUS = True
IS_OBJECT_MANAGER = True

ADAPTER = "/org/bluez/hci0"
ADAPTER_IFACE = "org.bluez.Adapter1"
DEVICE_IFACE = "org.bluez.Device1"
AGENT_IFACE = "org.bluez.Agent1"

YOURS = [
    ("88:C9:E8:00:00:01", "WH-1000XM5", "audio-headset", True, 80),
    ("D4:3B:04:00:00:02", "MX Master 3S", "input-mouse", False, None),
]
NEARBY = [
    ("A8:9C:ED:00:00:03", "Pixel 8", "phone"),
    ("DC:2C:26:00:00:04", "Keychron K2", "input-keyboard"),
    ("00:1D:43:00:00:05", "Old speaker", "audio-card"),
    ("4C:11:AE:00:00:06", None, None),
]


def failed(name, text):
    return dbus.exceptions.DBusException(text, name="org.bluez.Error." + name)


def load(mock, _parameters):
    mock.AddObject(
        "/org/bluez",
        "org.bluez.AgentManager1",
        {},
        [
            ("RegisterAgent", "os", "", lambda self, path, _caps: setattr(self, "agent", path)),
            ("RequestDefaultAgent", "o", "", lambda _self, _path: None),
            ("UnregisterAgent", "o", "", lambda _self, _path: None),
        ],
    )
    mock.AddObject(
        ADAPTER,
        ADAPTER_IFACE,
        {
            "Address": "00:1A:7D:DA:71:13",
            "Name": "apolline-dev",
            "Alias": "apolline-dev",
            "Powered": dbus.Boolean(True),
            "Discovering": dbus.Boolean(False),
        },
        [
            ("StartDiscovery", "", "", start_discovery),
            ("StopDiscovery", "", "", lambda self: self.UpdateProperties(ADAPTER_IFACE, {"Discovering": dbus.Boolean(False)})),
            ("RemoveDevice", "o", "", remove_device),
        ],
    )
    mock.object_manager_emit_added(ADAPTER)
    for address, name, icon, connected, battery in YOURS:
        add(mock, address, name, icon, paired=True, connected=connected, battery=battery)


def add(mock, address, name, icon, paired=False, connected=False, battery=None):
    path = f"{ADAPTER}/dev_{address.replace(':', '_')}"
    if path in dbusmock.mockobject.objects:
        return
    props = {
        "Address": address,
        "Alias": name or address.replace(":", "-"),
        "Adapter": dbus.ObjectPath(ADAPTER),
        "Paired": dbus.Boolean(paired),
        "Bonded": dbus.Boolean(paired),
        "Trusted": dbus.Boolean(paired),
        "Connected": dbus.Boolean(connected),
        "RSSI": dbus.Int16(-60),
    }
    if name:
        props["Name"] = name
    if icon:
        props["Icon"] = icon
    mock.AddObject(
        path,
        DEVICE_IFACE,
        props,
        [
            ("Connect", "", "", connect),
            ("Disconnect", "", "", lambda self: self.UpdateProperties(DEVICE_IFACE, {"Connected": dbus.Boolean(False)})),
            ("Pair", "", "", pair),
            ("CancelPairing", "", "", lambda _self: None),
        ],
    )
    if battery is not None:
        dbusmock.get_object(path).AddProperties("org.bluez.Battery1", {"Percentage": dbus.Byte(battery)})
    mock.object_manager_emit_added(path)


def start_discovery(adapter):
    adapter.UpdateProperties(ADAPTER_IFACE, {"Discovering": dbus.Boolean(True)})
    manager = dbusmock.get_object(MAIN_OBJ)

    def found():
        for address, name, icon in NEARBY:
            add(manager, address, name, icon)
        return False

    GLib.timeout_add(1000, found)


def remove_device(adapter, path):
    if path not in dbusmock.mockobject.objects:
        raise failed("DoesNotExist", "Does Not Exist")
    dbusmock.get_object(MAIN_OBJ).object_manager_emit_removed(path)
    adapter.RemoveObject(path)


def connect(device):
    if device.props[DEVICE_IFACE]["Icon"] == "input-mouse":
        raise failed("Failed", "br-connection-page-timeout")
    device.UpdateProperties(DEVICE_IFACE, {"Connected": dbus.Boolean(True)})


# Asks the agent and waits for the answer, answering nothing else meanwhile, the way bluetoothd
# is busy with a pairing. The shell's agent calls nothing back, so this can't deadlock.
def pair(device):
    props = device.props[DEVICE_IFACE]
    if props["Paired"]:
        raise failed("AlreadyExists", "Already Exists")
    agent = find_agent(device.connection, getattr(dbusmock.get_object("/org/bluez"), "agent", None))
    if agent is None:
        raise failed("AuthenticationFailed", "No agent to ask")
    path = dbus.ObjectPath(device.path)
    try:
        if props.get("Icon") == "phone":
            agent.RequestConfirmation(path, dbus.UInt32(123456), dbus_interface=AGENT_IFACE, timeout=120)
        elif props.get("Icon") == "input-keyboard":
            agent.DisplayPasskey(path, dbus.UInt32(48213), dbus.UInt16(0), dbus_interface=AGENT_IFACE)
            GLib.usleep(4_000_000)
        elif (pin := agent.RequestPinCode(path, dbus_interface=AGENT_IFACE, timeout=120)) != "0000":
            raise failed("AuthenticationFailed", f"Authentication Failed: the PIN was {pin!r}")
    except dbus.exceptions.DBusException as err:
        if err.get_dbus_name().startswith("org.bluez.Error.Authentication"):
            raise
        raise failed("AuthenticationRejected", "Authentication Rejected") from err
    device.UpdateProperties(DEVICE_IFACE, {"Paired": dbus.Boolean(True), "Bonded": dbus.Boolean(True)})


# The mock isn't told who registered, so it asks everyone on the bus who serves that path.
def find_agent(bus, path):
    for name in bus.list_names() if path else []:
        if not name.startswith(":") or name == bus.get_unique_name():
            continue
        try:
            obj = bus.get_object(name, path, introspect=False)
            if AGENT_IFACE in obj.Introspect(dbus_interface="org.freedesktop.DBus.Introspectable", timeout=1):
                return obj
        except dbus.exceptions.DBusException:
            continue
    return None
