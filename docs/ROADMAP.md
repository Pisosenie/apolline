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
  and shows what changed.
- **Colors from your wallpaper** without pywal and a theme folder per look.
- **Eye candy you can dial:** window opacity, inactive dimming, corner radius, shadows, animation speed,
  per app if you want, each one a slider and a line in the config.
- **Fast with the keyboard:** Alt+Tab, launcher, clipboard, every shortcut listed and searchable.
- **Games first-class:** a performance mode, VRR, no popups, the screen stays awake on a gamepad.

## Next to Noctalia and DankMaterialShell

They are the shells people already run on niri, both QML on Quickshell, both with more widgets than
Apolline. Apolline doesn't race them on widgets:

- **Modes change the whole desktop, not just colors.** Notifications, apps, performance and keys change
  together, and a mode can switch itself on.
- **Updates don't break it.** Your niri config keeps one include line, and CI runs Apolline against
  every new niri release.
- **One native binary.** The README shows idle CPU and memory next to theirs, measured the same way.

## Principles

1. **Finished on first login.** Nothing to set up before it looks good. No modes until you make one.
2. **No guessing.** Every option has a control, a sentence that says what it does, and the exact
   lines it writes. Typos in the file are reported with the line and a suggestion; the rest applies.
3. **Your niri config stays yours.** Apolline adds one include line and keeps a copy; it never edits
   your file again. Its own fragment is regenerated, so removing Apolline is removing one line.
4. **Keyboard first, mouse friendly.** Every overlay works with arrows, Enter and Esc, and closes
   on a click outside. Every shortcut is shown where the action is.
5. **Fast and quiet.** No polling where an event exists, nothing kept on disk that you didn't ask for.

## Done

- Bar with workspaces, clock, tray, notifications bell; one per monitor.
- Quick settings: Wi-Fi, Bluetooth, do not disturb, keep awake, volume, brightness, player,
  notification history, screenshot, power.
- Launcher with apps and Steam games, actions, keyboard navigation.
- Lock screen (ext-session-lock + PAM), idle lock, lock before sleep, screen off.
- Settings: appearance, bar, lock and idle, notifications, keyboard, mouse and touchpad, shortcuts
  with conflicts against niri, health check.
- Notification daemon, OSD for volume, brightness, microphone and layout, clipboard history.
- Arch package.
- Modes you make yourself, with their own keys.
- Windows: niri's Alt+Tab with previews in Apolline colors, windows on the bar, window settings,
  opacity and animation speed, per-app rules.
- Themes: a gallery, save my look, add someone's file; the accent picked from your wallpaper;
  the bar at the top or bottom, floating or edge to edge, as see-through as you like.

## Next

### 1. Modes you make yourself — done
Out of the box there are no modes: one desktop, its wallpaper and accent. A mode is a named set
of changes you switch to with one key.

- Start from a template (Work, Play, Evening, Focus, Presentation) or from nothing.
- Each mode: name, icon, accent, wallpaper, which notifications get through, apps to open,
  performance (power profile, games drive refresh, effects off), keep the screen awake,
  and **its own shortcuts** that exist only while the mode is on.
- Switch from the bar chip, quick settings, the launcher, `Mod+Shift+M` or `apolline mode NAME`;
  `none` leaves the mode. The bar shows no chip until the first mode exists.
- Settings → Modes: list, add from a template, reorder, delete, edit everything above with a live
  preview of the bar and the lock screen.

**Next: modes that switch themselves.** A mode can have triggers, and when the trigger ends the mode
you had before comes back:
- a game starts (Steam, Proton, gamescope, Wine) and ends;
- a time of day;
- running on battery;
- a monitor is plugged in or out.

Switching by hand always wins: triggers act only when something changes. Settings → Modes shows each
mode's triggers and what turned the current one on.

### 2. Many windows, comfortably — done
niri lays windows out in a scrolling strip; Apolline makes moving around it obvious.

- **Alt+Tab**: niri's own switcher with live previews, in recent order, colored and rounded by
  Apolline. Hold Alt, tap Tab, let go; Alt+` stays within one app.
- **Windows on the bar**: icons of the windows on this workspace, the focused one marked.
  Click focuses, middle click closes.
- Settings → Windows: width of new windows, the presets `Mod+R` cycles through, centering
  a lone window, focus follows mouse, warp the pointer to the focused window.
- Window look: opacity of focused and unfocused windows, animation speed (or none), all in
  `[layout]`/`[windows]` with sliders in Appearance.
- Per-app rules: floating, width, opacity.

**Next:** type a window title in the launcher and Enter focuses it; pin apps to the bar next to the
windows, so a pinned app is one click away whether it runs or not.

### 3. Rice: themes you can share — first part done
What HyDE is loved for, without the scripts.

- **Themes**: a look in one file — wallpaper, accent, corners, gaps, borders, shadows, opacity,
  animation speed and the bar. Settings → Themes: five bundled looks applied with a click,
  "save my look", add someone's file, open the folder to share one.
- **Colors from the wallpaper**: the accent is the picture's most present vivid hue, kept soft enough
  to sit next to the bundled accents and lightened until it reads on glass. Bundled wallpapers keep
  their hand-picked accents.
- **The bar your way**: top or bottom, floating or edge to edge, how see-through.

**Next:**
- **Three ways to color**: restrained (one accent on neutral glass), from the wallpaper, or picked
  by hand — in the desktop and in every mode.
- **Layers**: theme, then your tweaks, then the mode. Applying a theme never loses a tweak; Settings
  shows which layer a value comes from and keeps an undo history.
- **Themes as folders**: `theme.toml`, its wallpapers and an optional niri snippet;
  `apolline theme install FILE|URL`. A mode can switch the theme. (`apolline theme NAME|next|list`
  already switches from a script or a key.)
- **Per-surface style**: `[style.bar]`, `[style.launcher]`, `[style.panel]` for radius, opacity,
  padding and text size, plus layouts per surface: the launcher as a list or a grid, quick settings
  as tiles or a column.
- **niri eye candy as presets**: focus-ring gradients, animation curves, open and close effects,
  shaders where niri has them, each with a preview.
- **Fonts, icons, cursor and GTK theme** pickers that write the files apps actually read.
- **The same colors in apps**: a theme can carry small templates (GTK accent CSS, Qt colors, any
  app's config) that Apolline fills with the palette whenever it changes. Apolline ships none for
  terminals; the Apolline terminal (its own project) reads the palette itself.
- **Wallpapers**: a gallery of `~/.local/share/apolline/wallpapers` and any folder you add,
  a different wallpaper per monitor.

### 4. First run and finding things
- A welcome window on the first start: connect to niri (the include line), pick a wallpaper
  and accent, optionally add modes from templates, see the five shortcuts that matter.
- A searchable cheat sheet of every shortcut, Apolline's and niri's, grouped by what they do — done (`Mod+/`).
- A polkit agent, so apps that ask for an admin password get a dialog in Apolline colors instead of
  silently failing.
- A login screen for greetd that looks like the lock screen: users, session, keyboard layout, power.
- The launcher answers more than apps: math (`2^10`, `15% of 340`) and emoji by name.

### 5. Displays and comfort
- Settings → Displays: resolution, refresh rate, scale, arrangement, VRR, from niri's own output list.
- Night light on a schedule.
- Sound: output and input device picker in quick settings.
- Wi-Fi and Bluetooth beyond on and off: nearby networks with a password field, pairing new devices,
  forgetting old ones.
- A click on the clock: the month, and today's weather from Open-Meteo, no account needed.
- A bar module of your own: a command that prints a new line whenever its text changes.

### 6. Getting it to people
- Screenshots and a short video in the README, `apolline-git` on the AUR, CI that builds and tests
  on every push.
- Idle CPU and memory in the README, measured next to Noctalia and DankMaterialShell.
- CI against every niri release, replaying recorded IPC sessions, so a niri update never breaks the bar.
- A Fedora COPR after the AUR.

## Not now

- **Plugins.** They freeze the API while it still changes every week. A bar module of your own covers
  most of what people write plugins for.
- **A dock.** niri's strip plus windows and pinned apps on the bar already do its job.
- **An AI panel, a process monitor, a notepad.** Other apps do these well.
- **Later:** screen recording with "save the last 30 seconds" for games, and live wallpapers (video,
  shaders) in an `apolline wall` child process the shell drives over JSON lines on stdin and stdout.
