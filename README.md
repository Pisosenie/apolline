<h1 align="center">Apolline</h1>

<p align="center"><b>A desktop shell for niri that looks finished on the first login.</b><br>
Bar, launcher, quick settings, notifications, clipboard, lock and login screens, live wallpapers and a settings window, in one Rust binary. Modes switch the whole desktop with a key, a game or the clock.</p>

<p align="center">
<a href="https://github.com/Pisosenie/apolline/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Pisosenie/apolline/actions/workflows/ci.yml/badge.svg"></a>
<a href="https://github.com/Pisosenie/apolline/releases"><img alt="Release" src="https://img.shields.io/github/v/release/Pisosenie/apolline?color=86bdf0"></a>
<img alt="License: MIT or Apache-2.0" src="https://img.shields.io/badge/license-MIT%20or%20Apache--2.0-5dd39e">
<img alt="For niri" src="https://img.shields.io/badge/made%20for-niri-c084fc">
<img alt="Arch Linux" src="https://img.shields.io/badge/packaged%20for-Arch-1793d1">
</p>

<p align="center">
<a href="#installing">Install</a> · <a href="#first-steps">First steps</a> · <a href="#coming-from-hyprland">From Hyprland</a> · <a href="#modes">Modes</a> · <a href="#themes">Themes</a> · <a href="docs/CONFIG.md">Everything else</a>
</p>

![One desktop in four themes: Mocha, Latte with the bar in islands and Hardline in Gruvbox from the catalog, and Paper with the bar at the bottom](docs/screenshots/looks.jpg)

Apolline is everything around your windows. [niri](https://github.com/YaLTeR/niri) tiles them; Apolline draws the bar, the wallpaper and the clock on it, opens apps, shows notifications, locks the screen and greets you at login, and gives you a settings window for all of it. It replaces waybar, rofi or fuzzel, mako, swaylock, swww, a polkit agent and a clipboard manager, and it is built to look like one thing instead of ten.

At rest it wakes once a minute, for the clock; everything else arrives as an event. That is 0.2% CPU and about 100 MB on a desktop, 0.2–0.4% in a VM without a GPU. When the GPU renderer can't start, every surface falls back to drawing on the CPU, so the lock screen still comes up.

## At a glance

| | |
|---|---|
| **Bar** | workspaces, windows and pinned apps, tray, media, clock with a calendar and the weather, CPU, memory, GPU and temperature, waiting updates, who's using the microphone or the screen; as a bar, islands or one pill, on any edge |
| **Launcher** `Mod+Space` | apps and Steam games, open windows, sums, units and currency, emoji, files, screenshots, a color picker, screen recording, timers, commands and the web |
| **Quick settings** `Mod+A` | Wi-Fi, Bluetooth, VPN, volume per app, microphone, brightness, night light, power profile, do not disturb, the player, past notifications |
| **Live wallpapers** | videos, Shadertoy shaders and Wallpaper Engine scenes from your Steam library, drawn by [apolline-wall](https://github.com/Pisosenie/apolline-wall) and paused while you're locked or playing |
| **Lock and login** | an ext-session-lock lock screen with what's playing, the weather and the next meeting, a wallpaper of its own if you like, and a greetd greeter in the same look |
| **Modes** | Work, Play, Evening or your own: a key, a game starting, the battery, the hour or a monitor switches wallpaper, accent, notifications, apps, performance and shortcuts |
| **Themes** | six in the box and more in a catalog to try on for a minute; a theme can carry a whole rice; GTK, Qt and KDE apps can follow your colors |
| **Settings** `apolline settings` | every option with switches and a live preview, beside the exact lines it writes to `config.toml`, comments kept |
| **And** | notifications with replies, clipboard history, screenshot markup, a system monitor, a polkit agent, glass blur under popups |

| | |
|---|---|
| ![The launcher finds the running Firefox before offering a new one](docs/screenshots/launcher.jpg) | ![The lock screen, with the weather and what's coming up](docs/screenshots/lock.jpg) |
| ![Switching modes](docs/screenshots/switch.jpg) | ![Settings → Modes, with the lines it writes](docs/screenshots/modes.jpg) |
| ![Settings → Appearance](docs/screenshots/themes.jpg) | ![The calendar under the clock: the weather and the day's events](docs/screenshots/calendar.jpg) |
| ![Notifications with a picture, a progress bar and the app's own buttons](docs/screenshots/notifications.jpg) | ![Bluetooth: your devices with the headphones' battery, and new ones nearby to pair](docs/screenshots/bluetooth.jpg) |
| ![Quick settings over the Ember look](docs/screenshots/panel.jpg) | ![The login screen in the lock's look: centered, blurred, your accent and font](docs/screenshots/greet.jpg) |
| ![The system monitor over the Wired rice: processor, memory and network as they go, and what's busiest](docs/screenshots/machine.jpg) | ![Wired, an example rice after Lain: vif through a CRT with fastfetch, a NAVI booting and btop](examples/wired/terminals.jpg) |

A whole rice to start from, wallpaper and terminals included, is in [examples/wired](examples/wired).

## Installing

**A fresh Arch**, with your user in sudo. One line builds and sets up the whole desktop: niri, Apolline, its terminal [vif](https://github.com/Pisosenie/vif), live wallpapers, sound, network, Bluetooth, the login screen, a file manager and fonts.

```sh
curl -fsSL https://raw.githubusercontent.com/Pisosenie/apolline/main/install.sh | bash
```

Reboot and log in to niri. The script is short and worth a read first: [`install.sh`](install.sh). It takes a few switches:

| Variable | Does |
|---|---|
| `APOLLINE_FROM=git` | builds the newest commit (`apolline-git`) instead of the latest release |
| `APOLLINE_GREETER=keep` | keeps the display manager you have; otherwise greetd shows `apolline greet` |
| `APOLLINE_LIVE=no` | skips apolline-wall, so no live wallpapers |

```sh
curl -fsSL https://raw.githubusercontent.com/Pisosenie/apolline/main/install.sh | APOLLINE_FROM=git bash
```

Run the same line again to update. Everything it pulls in from the Arch repos is the `apolline-desktop` package in [`packaging/arch`](packaging/arch).

**An Arch you already use**, with niri on it, by hand. vif goes first, because Apolline opens it as the terminal (`vif-bin` is prebuilt):

```sh
git clone https://github.com/Pisosenie/vif && (cd vif/packaging/arch/vif-bin && makepkg -si)
git clone https://github.com/Pisosenie/apolline && cd apolline
(cd packaging/arch/apolline && makepkg -si)   # the latest release; packaging/arch/apolline-git builds main
apolline setup                                # once, as you: adds Apolline to your niri config
apolline doctor                               # what works here, and what to install for the rest
```

`apolline setup` saves a copy of your niri config, adds one `include` line to it, and comments out niri's stock waybar line so there's one bar. For live wallpapers add apolline-wall:

```sh
git clone https://github.com/Pisosenie/apolline-wall && (cd apolline-wall/packaging/arch && makepkg -si)
```

**Other distributions** aren't packaged yet. Apolline builds with a current stable Rust (`cargo build --release -p apolline-shell`, the binary is `target/release/apolline`), and `apolline setup` and `apolline doctor` work the same; `apolline doctor` names what's missing.

## First steps

| To | Do |
|---|---|
| open an app | `Mod+Space`, type, Enter |
| change the wallpaper | Settings → Appearance; anything with ▶ is live |
| give the lock screen its own wallpaper | Settings → Lock and sleep → Wallpaper |
| try a whole new look | Settings → Appearance, or `apolline theme next` |
| come over from Hyprland | `apolline import hyprland --dry-run` to see, then without `--dry-run` |
| make a mode | Settings → Modes → a template; switch with `Mod+Shift+M` |
| see every key | `Mod+/` |
| take a screenshot | niri's `Print` keys, or *screenshot* in the launcher; a card offers to mark it up |
| keep a game from losing the focus to a stray key | `Mod+Shift+G`, and again to let go |
| lock | `Super+Alt+L` |
| restart the shell | `pkill -x apolline; niri msg action spawn -- apolline` |

Settings opens from quick settings, the launcher or `apolline settings`, and every change is live. The config is `~/.config/apolline/config.toml`, and it starts as [`assets/config.toml`](assets/config.toml); every option is in [`docs/CONFIG.md`](docs/CONFIG.md).

**If something goes wrong.** `apolline doctor` says what's missing and how to install it, and `apolline check` points at a line in the config that doesn't read. The shell is a separate process from niri, and a crash starts it again; after three crashes in a minute it starts in safe mode, on the defaults and drawing on the CPU, without touching your settings, and a card brings it back as usual once they're fixed (`apolline --safe` starts it that way by hand). What went wrong is in `~/.local/state/apolline/crashes.log`, and `apolline report` puts it in one file for an issue together with the versions, the GPU, the screens, what `apolline doctor` says and your config, with links, your home folder and your name taken out. The lock screen is a process of its own too, so a crash leaves the session locked; from a TTY (`Ctrl+Alt+F3`) run `apolline lock` again to get the password field back.

## Coming from Hyprland

```sh
apolline import hyprland --dry-run   # what would come over, line by line
apolline import hyprland             # bring it over
apolline import hyprland --undo      # put everything back
```

It reads `hyprland.lua` (run in a sandbox where nothing starts or gets written) or `hyprland.conf` with the files it sources. Your binds become niri's actions in `[keys]`, and the ones for a launcher, a lock, a clipboard or notifications move Apolline's own keys there instead; screens go to `[[display]]` at the refresh they really have, apps matched by name get window rules, keyboard layouts, touchpad, gaps, corners and the accent carry over, and the programs Hyprland started go to `~/.config/autostart` for niri. What niri or Apolline already does is said so, and what has no place here (split layouts, the scratchpad, submaps, Lua callbacks) is listed with why. The config is copied to `config.toml.before-hyprland` first.

## Modes

There are none until you make one, in Settings → Modes from a template (Work, Play, Evening, Focus, Presentation) or from nothing, or as `[[mode]]` in the config. A mode can change the wallpaper and accent, hold back notifications, open apps, keep the screen awake, add **its own shortcuts** that exist only while it's on, and turn on performance.

Switch from the bar, quick settings, the launcher, `Mod+Shift+M`, or `apolline mode NAME|next|none`. A mode can also switch itself on:

```toml
[[mode]]
name = "Play"
when = ["game"]                  # Steam, Proton, gamescope or Wine running

[[mode]]
name = "Evening"
when = ["20:00-07:00", "battery", "monitor HDMI-A-1"]
```

When the reason ends, the mode you had before comes back. Triggers act only when something changes, so switching by hand always wins.

A mode can also wear a theme (`theme = "Glass"`): its look applies while the mode is on and leaves with it, without touching the config; the mode's own wallpaper and accent still win. A switch recolors window borders, opens the mode's apps that aren't running yet and rewrites the mode's keys into niri.

A mode can also put **windows in their places**: lay them out once, then Settings → Modes → Windows in their places → Remember. Each time the mode comes on, the windows that are open move to their screen, workspace and column, with the column's width, and the rest start and are placed as niri opens them, for a minute while slow apps bring their own windows back. Windows the mode doesn't list stay where they are, to the right. By hand:

```toml
[[mode]]
name = "Work"
windows = [
    { app = "kitty", run = "kitty", screen = "DP-1", workspace = 1, column = 1, width = 40 },
    { app = "foot", run = "foot", screen = "DP-1", workspace = 1, column = 1 },   # under kitty
    { app = "firefox", title = "Mail", run = "firefox", workspace = 2, column = 1, width = 70 },
]
```

`app` is the app id niri shows, `title` picks one of several windows by a piece of its title, and a window without `run` is only moved, never started.

Performance turns off animations and window shadows, lets games (Steam and Proton, gamescope, Wine) drive the monitor's variable refresh rate, and sets the power profile to performance — and back to what it was when you leave, unless you changed it yourself in between. Variable refresh rate is set for monitors your niri config doesn't already describe; for those, add `variable-refresh-rate on-demand=true` to their `output` block.

## Themes

A theme is one small file: wallpaper, accent, palette, gaps, corners, borders, shadows, window opacity, animation speed, the bar's place, shape and look, and the size, corners and glass of the launcher, quick settings and popups. Settings → Appearance shows them as a gallery: six come with Apolline (Fog, Ember, Night; Mocha in Catppuccin; Paper, light and plain; Glass) and more wait in the catalog, Tonal, Latte, Hardline and Minimal among them. *Save my look* turns your current setup into one, and *Add someone's theme* takes a file somebody sent you. They live in `~/.config/apolline/themes`, so sharing a look is sending a file.

```sh
apolline theme            # list them, the applied one marked
apolline theme Glass      # apply one
apolline theme next       # the next one, handy on a key
apolline theme install ~/Downloads/dune   # a theme.toml, or a folder with one and its pictures
apolline theme install https://example.com/dune.toml   # or straight from a link
```

Settings → Appearance → More themes browses the catalog, themes people made with their wallpapers; nothing is fetched until you ask. **Try on** wears one for a minute: Keep, and it stays; Go back, close Settings or let the minute run out, and your config comes back as it was and the theme is gone again. The config from before waits in `config.toml.trying`, so even a shell that goes down wearing it starts again in yours.

A theme can be a whole setup, a rice: besides the looks, `[bar] left`, `center`, `right` and `tray` place the bar's pieces and `[dock]` sets the dock up. A theme without them leaves yours be. A Wallpaper Engine scene goes in by its Workshop id, `[theme] scene = "2847562910"`: with the scene in your Steam library the theme wears it, without it the theme's own picture stands in, and Settings offers the scene's page in Steam. Scenes are never copied into a theme; they're their authors'. Save my look saves all of it, bar and dock too.

The catalog is the `catalog/` folder of this repository: a theme is a folder there with its `theme.toml`, its pictures and a `preview.jpg`, listed in `catalog/index.toml`, and a pull request adds yours. The tests check every folder against the index, and CI renders each theme a pull request brings with `tools/preview.sh`, the real session under a headless sway, into the run's `previews`, so the picture in the catalog is what you'll get. `APOLLINE_CATALOG=http://127.0.0.1:8000` points Settings at a local copy while you make one.

How a theme file reads, folders with their own `niri.kdl`, palettes, colors of your own and templates for other apps: [`docs/CONFIG.md`](docs/CONFIG.md#making-a-theme).


## Everything else

The rest of what Apolline does, and every option it takes, is in [`docs/CONFIG.md`](docs/CONFIG.md):

- [With niri](docs/CONFIG.md#with-niri)
- [Windows](docs/CONFIG.md#windows)
- [Surfaces your way](docs/CONFIG.md#surfaces-your-way)
- [Your own bar modules](docs/CONFIG.md#your-own-bar-modules)
- [Plugins](docs/CONFIG.md#plugins)
- [Laying out the bar](docs/CONFIG.md#laying-out-the-bar)
- [The dock](docs/CONFIG.md#the-dock)
- [The terminal](docs/CONFIG.md#the-terminal)
- [Apps in your colors](docs/CONFIG.md#apps-in-your-colors)
- [Wallpapers](docs/CONFIG.md#wallpapers)
- [Lock screen](docs/CONFIG.md#lock-screen)
- [Night light](docs/CONFIG.md#night-light)
- [Calendar and weather](docs/CONFIG.md#calendar-and-weather)
- [Admin passwords](docs/CONFIG.md#admin-passwords)
- [Login screen](docs/CONFIG.md#login-screen)
- [Notifications](docs/CONFIG.md#notifications)
- [Settings](docs/CONFIG.md#settings)
- [Config](docs/CONFIG.md#config)
- [Making a theme](docs/CONFIG.md#making-a-theme)

Building it, running it in a window and the layout of the code: [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

MIT or Apache-2.0, at your option ([`LICENSE-MIT`](LICENSE-MIT), [`LICENSE-APACHE`](LICENSE-APACHE)). Geist is under the OFL, the icons come from Lucide (ISC).
