# Apolline

A desktop shell for Wayland built around modes. One key switches the whole desktop — wallpaper, accent, open apps, notifications and performance — between Work, Play and Evening.

Early days. Inside a Wayland session on Linux it draws the wallpaper and the bar as layer-shell surfaces; anywhere else it opens a preview window. Under [niri](https://github.com/YaLTeR/niri) the bar shows live workspaces and the keyboard layout, and a click on a workspace switches to it. Battery comes from UPower and the playing track from any MPRIS player. Arch first.

## Try it

```sh
cargo run -p apolline-shell                    # layer-shell session under Wayland, preview window elsewhere
cargo run -p apolline-shell -- --preview       # always a window: 1/2/3 modes, m switcher, space launcher, q quick settings
cargo run -p apolline-shell -- --preview --open launcher
cargo run -p apolline-shell -- settings        # the settings app, or settings --page keyboard
cargo run -p apolline-shell -- check           # validate ~/.config/apolline/config.toml
cargo run -p apolline-shell -- --mode play --screenshot shot.png
tools/headless.sh shot.png --mode play         # Linux: the real session under a headless sway, saved with grim
tools/fake-controls.sh ~/fakebin               # stand-in wpctl, nmcli, bluetoothctl, brightnessctl for quick settings
```

## With niri

```sh
apolline setup
```

It writes Apolline's niri fragment to `~/.config/apolline/niri.kdl` and adds one `include` line at the end of your niri config, after saving a copy of it. The fragment starts Apolline with the session, matches window gaps, borders and corners to the shell, puts the wallpaper behind the overview, opens the settings window floating, and binds three keys:

| Key | Opens |
|---|---|
| `Mod+Space` | the launcher: apps and games from `.desktop` files (Steam games included), modes, settings, lock |
| `Mod+A` | quick settings: Wi-Fi, Bluetooth, do not disturb, screenshot, volume, brightness, mode, the player, power |
| `Mod+Shift+M` | the mode switcher |

Change them in settings → Shortcuts, which also tells you what each one replaces in your niri config. Quick settings use `wpctl` (PipeWire), `brightnessctl`, `nmcli` (NetworkManager) and `bluetoothctl`; whatever is missing just isn't shown.

Switch modes from the switcher (`apolline switch`) or from anywhere with `apolline mode work|play|evening|next`. Each switch recolors window borders to the mode's accent and opens the mode's apps that aren't running yet (`open` in the config).

Play goes further: animations and window shadows are off, games (Steam and Proton, gamescope, Wine) drive the monitor's variable refresh rate, and the power profile goes to performance while you play — and back to what it was when you leave, unless you changed it yourself in between. Variable refresh rate is set for monitors your niri config doesn't already describe; for those, add `variable-refresh-rate on-demand=true` to their `output` block.

## Notifications

Apolline is a notification daemon too, unless another one (mako, dunst, …) already holds the name — then it stays out of the way. Each mode decides what reaches the screen: Work holds low-priority ones, Play holds everything except critical (calls, alarms), Evening lets all through. Held notifications wait behind a counter in the bar and come out when you switch to a mode that allows them, or when you click the counter.

## Settings

`apolline settings` covers modes, window look, the bar, notifications, keyboard layouts and repeat, touchpad and mouse, shortcuts, and a health check of the config — with switches instead of syntax and a live preview where it helps. Every change is saved right away and the shell picks it up; the panel on the right shows exactly what was written. It edits the file in place, so your own comments and order stay, and it refuses to touch a file it can't parse.

## Config

Keyboard, touchpad and mouse stay with your niri config until you set them in Apolline; after that Apolline writes them into its niri fragment.

The config is forgiving and live: changes apply as soon as you save, a typo shows up as a card with the line and a suggestion, and everything else still applies. The card goes away when you fix it. See [`assets/config.toml`](assets/config.toml).

The three default wallpapers are built into the binary; point `wallpaper` at your own file to use something else.

## Layout

| Path | What |
|---|---|
| `crates/theme` | design tokens per mode, contrast-checked in tests |
| `crates/config` | config model and its parser |
| `crates/niri` | niri IPC protocol and the state it describes, no I/O |
| `crates/shell` | the `apolline` binary: shared core, preview window, layer-shell session |
| `assets/` | wallpapers, Geist fonts (OFL), the default config, the niri fragment |
| `assets/icons` | the Lucide icons Apolline uses (ISC) |
| `tools/` | wallpaper generator, headless screenshot script, stand-ins for MPRIS, UPower, power profiles and the quick settings tools |

## License

MIT OR Apache-2.0
