"""fprintd with one reader that knows a finger of yours; tools/fake-bus.sh starts it as root,
since the lock screen believes no other fprintd.

A finger is "put on" three seconds after the reader starts waiting: the first doesn't match,
the next one does, so a lock screen on this bus unlocks by itself after about six seconds.
"""

import dbus
import dbusmock
from gi.repository import GLib

BUS_NAME = "net.reactivated.Fprint"
MAIN_OBJ = "/net/reactivated/Fprint/Manager"
MAIN_IFACE = "net.reactivated.Fprint.Manager"
SYSTEM_BUS = True

DEVICE = "/net/reactivated/Fprint/Device/0"
DEVICE_IFACE = "net.reactivated.Fprint.Device"


def load(mock, _parameters):
    mock.AddMethods(
        MAIN_IFACE,
        [
            ("GetDefaultDevice", "", "o", lambda _self: dbus.ObjectPath(DEVICE)),
            ("GetDevices", "", "ao", lambda _self: [dbus.ObjectPath(DEVICE)]),
        ],
    )
    mock.AddObject(
        DEVICE,
        DEVICE_IFACE,
        {"name": "Fake reader", "scan-type": "press", "num-enroll-stages": dbus.Int32(5)},
        [
            ("ListEnrolledFingers", "s", "as", lambda _self, _user: ["right-index-finger"]),
            ("Claim", "s", "", claim),
            ("Release", "", "", release),
            ("VerifyStart", "s", "", verify_start),
            ("VerifyStop", "", "", lambda _self: None),
        ],
    )


def claim(device, _user):
    if getattr(device, "claimed", False):
        raise dbus.exceptions.DBusException(
            "Device was already claimed", name="net.reactivated.Fprint.Error.AlreadyInUse"
        )
    device.claimed = True
    device.tries = 0


def release(device):
    device.claimed = False


def verify_start(device, _finger):
    device.tries += 1
    result = "verify-match" if device.tries > 1 else "verify-no-match"

    def touched():
        device.EmitSignal(DEVICE_IFACE, "VerifyStatus", "sb", [result, True])
        return False

    GLib.timeout_add(3000, touched)
