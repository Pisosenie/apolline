# Changelog

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
