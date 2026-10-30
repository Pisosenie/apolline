# Apolline

A desktop shell for Wayland built around modes. One key switches the whole desktop — wallpaper, accent, open apps, notifications and performance — between Work, Play and Evening.

Early days. Inside a Wayland session on Linux it draws the wallpaper and the bar as layer-shell surfaces; anywhere else it opens a preview window. Under [niri](https://github.com/YaLTeR/niri) the bar shows live workspaces and the keyboard layout, and a click on a workspace switches to it. Battery comes from UPower and the playing track from any MPRIS player. Arch first.

## Try it

```sh
cargo run -p apolline-shell                    # layer-shell session under Wayland, preview window elsewhere
cargo run -p apolline-shell -- --preview       # always a window: 1/2/3 switch modes, ←/→ workspaces, Esc quits
cargo run -p apolline-shell -- check           # validate ~/.config/apolline/config.toml
cargo run -p apolline-shell -- --mode play --screenshot shot.png
tools/headless.sh shot.png --mode play         # Linux: the real session under a headless sway, saved with grim
```

## With niri

```sh
apolline setup
```

It writes Apolline's niri fragment to `~/.config/apolline/niri.kdl` and adds one `include` line at the end of your niri config, after saving a copy of it. The fragment starts Apolline with the session, matches window gaps, borders and corners to the shell, puts the wallpaper behind the overview, and binds `Mod+Shift+M` to the next mode.

Switch modes from anywhere with `apolline mode work|play|evening|next`. Each switch recolors window borders to the mode's accent and opens the mode's apps that aren't running yet (`open` in the config).

## Config

The config is forgiving: a typo is reported with the line and a suggestion, and everything else still applies. See [`assets/config.toml`](assets/config.toml).

## Layout

| Path | What |
|---|---|
| `crates/theme` | design tokens per mode, contrast-checked in tests |
| `crates/config` | config model and its parser |
| `crates/niri` | niri IPC protocol and the state it describes, no I/O |
| `crates/shell` | the `apolline` binary: shared core, preview window, layer-shell session |
| `assets/` | wallpapers, the default config, the niri fragment |
| `tools/` | wallpaper generator, headless screenshot script, fake MPRIS player and UPower battery |

## License

MIT OR Apache-2.0
