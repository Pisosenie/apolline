# Apolline roadmap

The goal: the desktop people pick niri for, without the evening of config reading.
Install it, run `apolline setup`, and the first login already looks finished.
Everything you can see you can change by clicking, and the file shows what changed.

## Who it is for

- **The developer on a laptop.** Terminal, editor and browser side by side, lots of copy and paste,
  a few hours of focus a day. Wants the keyboard for everything and a clipboard that remembers.
- **The player.** Wants games to run as fast as the machine allows, nothing popping up mid-match,
  and the screen to stay on while the gamepad is in hand.
- **The one coming from GNOME or macOS.** Expects a settings window, a launcher, quick settings,
  a lock screen and a clear way out of every screen. Never wants to open a KDL file.

## Why people leave HyDE for it

HyDE shows how much people want a riced Hyprland that looks good out of the box. Apolline keeps that
promise and drops the cost: everything HyDE lets you change, in a window, layered, and nothing breaks
on update.

- **One command, nothing to babysit.** A package and `apolline setup`; updates never overwrite your files.
- **A window instead of scripts.** Every option has a control and a live preview; the file still works
  and shows what changed, and Ctrl+Z takes any change back.
- **Colors from your wallpaper** without pywal and a theme folder per look.
- **Eye candy you can dial:** opacity, dimming, corners, shadows, gradient borders, blur, how windows
  move and appear, per app if you want, each one a control and a line in the config.
- **Fast with the keyboard:** Alt+Tab, launcher, clipboard, every shortcut listed and searchable.
- **Games first-class:** a mode that turns on by itself when a game starts, VRR, no popups, the screen
  stays awake.

## Next to Noctalia and Dux

Noctalia (now a native C++ shell with plugins and its own greeter) and Dux, formerly DankMaterialShell
(Quickshell and Go), are what people run on niri today, both with more widgets than Apolline.
Apolline doesn't race them on widgets:

- **Modes change the whole desktop and switch themselves.** Notifications, apps, performance, look and
  keys change together, and a mode can turn on while a game runs, on battery, at set hours or when a
  monitor is plugged in. Neither of them has rule-driven profiles.
- **Updates don't break it.** No Qt private API to break on a minor update; your niri config keeps one
  include line, and CI will run Apolline against every new niri release.
- **No surprise writes.** Coloring apps is opt-in, every file it writes is listed next to the switch,
  and turning it off removes exactly that. A gtk.css that is a dotfiles link is never touched.
- **Quiet at rest.** The shell wakes once a minute for the clock and repaints nothing else until something
  changes; if the GPU renderer can't start, it falls back to the CPU, so a driver problem can't keep you out.

## Principles

1. **Finished on first login.** Nothing to set up before it looks good. No modes until you make one.
2. **No guessing.** Every option has a control, a sentence that says what it does, and the exact
   lines it writes. Typos in the file are reported with the line and a suggestion; the rest applies.
3. **Your config stays yours.** Apolline adds one include line to niri and keeps a copy; it never edits
   your files again without a switch you turned on, and every such write can be undone.
4. **Keyboard first, mouse friendly.** Every overlay works with arrows, Enter and Esc, and closes
   on a click outside. Every shortcut is shown where the action is.
5. **Fast and quiet.** No polling where an event exists, nothing kept on disk that you didn't ask for.

## Done

- Bar: workspaces, pinned apps and open windows, clock, tray, notifications bell; top or bottom,
  floating or edge to edge, glass or frosted; one per monitor.
- Quick settings: Wi-Fi, Bluetooth, do not disturb, keep awake, volume and output device, microphone,
  brightness, player, notification history, screenshot, power.
- Launcher: apps and Steam games, open windows, sums, actions, keyboard navigation.
- Lock screen (ext-session-lock + PAM, its own process), idle lock, lock before sleep, screen off.
- Polkit agent with its own dialog, on polkit's socket-activated helper.
- Notification daemon, OSD for volume, brightness, microphone and layout, clipboard history.
- Settings for everything below, with the file it writes and undo; a welcome window on first start.
- Modes you make yourself, with their own keys, that can switch themselves on.
- Windows: niri's Alt+Tab in Apolline colors, window settings, per-app rules.
- Rice: themes (gallery, save, share, `apolline theme next`), accent from the wallpaper, a wallpaper
  gallery from your folders, gradient borders, motion and open effects, blur, GTK and Qt apps in the accent.
- Arch package.

## Next

### 1. Modes — done
Out of the box there are no modes: one desktop, its wallpaper and accent. A mode is a named set of
changes you switch to with one key: wallpaper, accent, notifications, apps to open, performance,
keep awake and its own shortcuts. It can switch itself on while a game runs, on battery, at set hours
or when a monitor is plugged in; when that ends, the mode you had comes back, and switching by hand
always wins.

**Next:** Settings → Modes shows what turned the current mode on; a mode can switch the theme.

### 2. Many windows, comfortably — done
niri's Alt+Tab with live previews in Apolline colors, open windows and pinned apps on the bar, window
settings, opacity and animation speed, per-app rules, windows in the launcher.

### 3. Rice — mostly done
**Next:**
- **Layers you can see**: theme, then your tweaks, then the mode; Settings shows which layer a value
  comes from.
- **Themes as folders**: `theme.toml`, its wallpapers and an optional niri snippet;
  `apolline theme install FILE|URL`.
- **Per-surface style**: `[style.bar]`, `[style.launcher]`, `[style.panel]` for radius, opacity,
  padding and text size; the launcher as a list or a grid.
- **Wallpapers**: a different one per monitor, rotation through a folder, a thumbnail cache on disk.
- **Fonts, icons, cursor and GTK theme** pickers that write the files apps actually read.
- **More apps in the accent**: KDE apps through kdeglobals, live reload of running GTK apps, Flatpak.
  Apolline ships no terminal templates; the Apolline terminal (its own project) reads the palette itself.

### 4. Finding things and getting in
- The launcher answers more: emoji by name, unit and currency conversion, files, and a dmenu mode
  (`apolline pick` reads lines from stdin) for your own scripts.
- Fingerprint next to the password on the lock screen and in the polkit dialog, both at once.
- A login screen for greetd that looks like the lock screen: users, session, keyboard layout, power,
  and no flash on the way into the session.

### 5. Displays and comfort
- Settings → Displays: resolution, refresh rate, scale, arrangement, VRR, with a revert timer,
  validated with `niri validate` before it applies.
- Night light on a schedule.
- Wi-Fi and Bluetooth beyond on and off, over D-Bus instead of command-line tools: nearby networks with
  a password field, pairing with a passkey, forgetting old ones.
- A click on the clock: the month, today's weather from Open-Meteo, events from an ICS or CalDAV
  calendar, no account sign-in.
- Capture: region and window screenshots with quick marks, a screen recorder, a color picker.
- A bar module of your own: a command that prints a new line whenever its text changes.

### 6. Getting it to people
- Screenshots and a short video in the README, `apolline-git` on the AUR, CI that builds and tests
  on every push.
- CI against every niri release, replaying recorded IPC sessions, so a niri update never breaks the bar.
- Idle CPU and memory measured on real hardware next to Noctalia and Dux, the same way each time.
  In a VM on Mesa's software renderer the shell idles at 0.2–0.4% CPU; its memory there is mostly llvmpipe's.
- A Fedora COPR after the AUR.

## Not now

- **Plugins.** They freeze the API while it still changes every week. A bar module of your own covers
  most of what people write plugins for; when plugins come, each gets a CPU and memory budget.
- **A dock.** niri's strip plus windows and pinned apps on the bar already do its job.
- **An AI panel, a process monitor, a notepad.** Other apps do these well.
- **Later:** screen recording with "save the last 30 seconds" for games, and live wallpapers (video,
  shaders) in an `apolline wall` child process the shell drives over JSON lines on stdin and stdout.
