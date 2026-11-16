# hypr2niri

Turns a Hyprland config into a niri one. It reads a `hyprland.conf` or `hyprland.lua` the way the
Hyprland it was written for reads it, from 0.30 to 0.57, and writes a niri `config.kdl` with a note
for every line: what came over, what niri does already, and what stays behind and why.

```sh
hypr2niri                                  # ~/.config/hypr/hyprland.lua or hyprland.conf, to the screen
hypr2niri hyprland.conf -o config.kdl      # then: niri validate -c config.kdl
```

Without installing anything, the same converter runs in the browser at
[pisosenie.github.io/apolline/hyprland](https://pisosenie.github.io/apolline/hyprland/).

## What it knows

| Hyprland | Read as |
|---|---|
| `windowrule = float, ^(kitty)$` (≤0.47), `windowrulev2` (≤0.52), `windowrule = match:class kitty, float on` and `windowrule { }` blocks (0.53+) | `window-rule` with `match app-id=…`, `exclude` for `negative:` |
| `layerrule`, `blurls`, 0.53's `match:namespace` | `layer-rule`, blur as `background-effect` |
| `monitor = …` lines, `monitorv2 { }` (0.50+), `desc:` | `output`; the refresh rate exact when niri runs |
| `drop_shadow`/`shadow_range` (≤0.44), `shadow { }` (0.45+) | `layout { shadow { } }` |
| binds of every flag, `bindd` titles, mouse wheel keys, submaps | `binds { }`; what's in a submap stays behind |
| `exec-once`, `exec`, `env`, cursor theme and size | `spawn-sh-at-startup`, `environment`, `cursor` |
| `hyprland.lua` (0.55+) | run in a sandbox: nothing starts, nothing is written |

It names the Hyprland versions a config fits, from its syntax. Split layouts, scratchpads, pinned
windows, submaps and per-device input have no place in niri and are listed with why. The output
needs niri 25.08 or newer; blur needs 26.04.

Part of [Apolline](https://github.com/Pisosenie/apolline), whose `apolline import hyprland` uses it.
MIT or Apache-2.0.
