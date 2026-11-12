# Working on Apolline

## Running it

```sh
cargo run -p apolline-shell                    # layer-shell session under Wayland, preview window elsewhere
cargo run -p apolline-shell -- --preview       # always a window: 1/2/3 modes, m switcher, space launcher, q quick settings
cargo run -p apolline-shell -- --preview --open launcher
cargo run -p apolline-shell -- settings        # the settings app, or settings --page keyboard
cargo run -p apolline-shell -- lock --preview  # the lock screen in a window
cargo run -p apolline-shell -- check           # validate ~/.config/apolline/config.toml
cargo run -p apolline-shell -- --mode play --screenshot shot.png
tools/headless.sh shot.png --mode play         # Linux: the real session under a headless sway, saved with grim
tools/preview.sh catalog/dracula dracula.jpg  # Linux: a catalog theme the same way, as CI renders its preview
tools/fake-controls.sh ~/fakebin               # stand-in wpctl and brightnessctl for quick settings
tools/fake-bus.sh                              # fake NetworkManager, BlueZ, fprintd, UPower and power profiles on a bus of their own
```

`apolline --help` leaves out what's only for this: `--preview`, `--open`, `--screenshot`, `fragment [CONFIG] [MODE]` (the niri fragment a config gives, as CI checks it), `pick` (a list to choose from, for scripts) and `greet` (the login screen greetd runs).

`cargo test --workspace` runs everything; tests never touch the niri you're in. [`docs/ROADMAP.md`](docs/ROADMAP.md) has what's next.


## Layout

| Path | What |
|---|---|
| `crates/theme` | colors and sizes, the accent picked from a wallpaper, contrast-checked in tests |
| `crates/config` | config model and its parser |
| `crates/niri` | niri IPC protocol and the state it describes, no I/O |
| `crates/shell` | the `apolline` binary: shared core, preview window, layer-shell session |
| `assets/` | wallpapers, Geist fonts (OFL), the default config, the niri fragment |
| `assets/themes` | the six bundled themes; `catalog/` has the rest, one folder each |
| `assets/icons` | the Lucide icons Apolline uses (ISC) |
| `assets/pam`, `assets/*.desktop`, `assets/apolline.svg` | what a package installs next to the binary |
| `packaging/arch` | the `apolline` (release) and `apolline-git` PKGBUILDs |
| `tools/` | wallpaper generator, headless screenshot script, a recorder of niri's event stream for the IPC tests, stand-ins for MPRIS, UPower, power profiles and the quick settings tools |

