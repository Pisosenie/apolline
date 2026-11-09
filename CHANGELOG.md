# Changelog

## Unreleased

- A Wallpaper Engine scene's own preview keeps standing for it on the lock screen, in the switcher and in the accent once it plays: it's saved as a real PNG now, not a JPEG or GIF under a .png name.
- `Mod+Shift+G` only gives niri's keys back and starts the shell when the shell is gone, not when it's slow, and it keeps your start mode and theme; staying still lets you save a replay and switch the keyboard layout, your own `switch-layout` keys too, and says so when niri didn't take the change.
- The Set up button in Settings does all that `apolline setup` does: layouts from Hyprland, the fastest refresh, autostart; and both check the fragment with niri before writing it.
- A lock screen picture that won't load, a video without its frame yet say, gives way to the desktop's, then Apolline's own.
- A live wallpaper on one screen no longer stops the others from changing their pictures or puts their desktop a layer up.
- The installer builds the new package before it removes the old one, leaves a file manager you chose and a network set up with iwd or systemd-networkd alone, and never writes over an earlier copy of your greetd config.

## 0.3.0 — 2026-11-07

- **Live wallpapers**: a video, a Shadertoy shader or a Wallpaper Engine scene as the wallpaper, drawn by [apolline-wall](https://github.com/Pisosenie/apolline-wall) as the shell's child. Settings → Appearance lists the scenes and videos from every Steam library, marked ▶; they pause while the screen is locked and in modes with performance on, and their last frame stands in for the accent and the lock screen. The one-line installer brings apolline-wall too.
- **The lock screen's own wallpaper**: Settings → Lock screen → Wallpaper, or `[lock] wallpaper`.
- **Stay in a game**: `Mod+Shift+G` keeps niri's keys and the hot corner from switching workspaces, columns or windows until it's pressed again or the window closes; sound keys and screenshots still work.
- **Glass**: quick settings, the calendar, the launcher and the other cards blur what's under them.
- **The GPU on the bar**: `[bar] stats` takes `gpu`, from amdgpu or NVIDIA's NVML, without waking a sleeping one.
- **Setup**: runs each screen at its fastest refresh unless you chose one, takes the keyboard layouts from Hyprland, keeps nm-applet and blueman out of niri only, and starts from niri's own config on a fresh system.
- Steam's toasts open in the corner without taking the focus; the pointer over the shell is the usual arrow; layers redraw on NVIDIA after a present comes back outdated; the lock tells logind the session is locked.
- The installer survives a rerun and keeps an existing greetd config; the README starts with how to install and the first things to do.

## 0.2.0 — 2026-11-06

- **Dock** (`[dock] show = true`, Settings → Bar → Dock): pinned and open apps along any edge, a dot for each window, a count of unread notifications on each app, magnify on hover, a menu with the app's windows, drag to reorder or unpin, hidden until the pointer reaches the edge.
- **Plugins in any language**: a folder with a `plugin.toml` adds bar pieces, quick-settings tiles, launcher sources, desktop widgets and its own settings (secrets go to the keyring). `apolline plugin install|list|enable|disable|remove`, and Settings → Plugins. An example is in `examples/plugins/ip`.
- **Motion**: the launcher, quick settings, calendar, clipboard, menus and the OSD fade and slide in and out, cards slide in and close the gap they leave, the workspace pill glides, buttons light up and dim on hover; all of it follows `[windows] animations`, and nothing runs at rest.
- **Lock screen**: what PAM asks beyond the password, a one-time code or a touch of a key, is asked right in the field. An expired password no longer keeps you out of your own session, and unlocking refreshes Kerberos tickets and the like instead of deleting them.

## 0.1.2 — 2026-11-05

- A command too long for the control socket gets its answer instead of a reset, which also makes the test suite pass on Linux.

## 0.1.1 — 2026-11-05

- `[desktop] wallpaper = "none"` (Settings → Appearance → Drawn by another program) leaves the wallpaper to apolline-wall, swww or hyprpaper: the shell keeps the clock and widgets a layer above it and lets clicks through to it.

## 0.1.0 — 2026-11-05

The first release: a whole desktop around niri, set up by `apolline setup`.

- **Bar**: workspaces, windows and pinned apps, the clock with the month, the weather and your calendars, what's playing, notifications, the tray, system figures, pending updates, a sign while the microphone listens or the screen is shared, timers and your own modules (Waybar scripts work as they are). Any edge, islands or one pill, its own pieces per screen, hidden until the pointer reaches the edge.
- **Launcher**: apps and games, open windows, modes, sums and units, currencies, files, emoji, commands, web search and timers (`timer 25m tea`).
- **Quick settings**: Wi-Fi, Bluetooth, VPN, volume per app, microphone, brightness, night light, power profiles, do not disturb, the player with the line being sung.
- **Notifications**: a daemon with cards, replies and buttons, a history that outlives a reboot, quiet hours and per-app rules.
- **Lock and login screens**, a polkit agent, a clipboard history with pictures, a system monitor, idle stages that shorten on battery.
- **Looks**: ten themes, palettes from the wallpaper, a big clock and widgets on the desktop, colors carried to GTK, Qt, vif and apps with templates.
- **Modes**: sets of wallpaper, accent, notifications, apps, performance and shortcuts, switched by a key, a game, the battery, the time or a monitor.
- **Settings**: a window for all of it that writes the config and keeps your comments; `apolline check` and `apolline doctor` for the rest.
