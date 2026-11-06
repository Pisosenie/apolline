# Working on Apolline

Apolline is a whole desktop around niri: bar, launcher, quick settings, notifications, lock and login screens, a dock, plugins, themes and modes, and a settings window that writes the config. It is one Rust binary, `apolline`, that runs as the shell and as each of its helpers. Read `README.md` for what it does, `CHANGELOG.md` for what each release added and `docs/ROADMAP.md` for the plan. The wallpaper engine is a separate project, `~/src/apolline-wall` (and its scene renderer `~/src/apolline-scene`), each with its own `AGENTS.md`.

## The workspace

- `crates/niri` (`apolline-niri`): talking to niri over its socket, workspaces, windows, outputs, events.
- `crates/config` (`apolline-config`): the TOML config, reading it with diagnostics (a typo gets a line and a suggestion, the rest applies), editing it in place with comments kept (`edit.rs`), pieces of the bar, the dock, plugin manifests.
- `crates/theme` (`apolline-theme`): colors, the ten palettes, a palette from the wallpaper (`accent_of`), terminal colors.
- `crates/shell` (`apolline-shell`, the `apolline` binary), iced 0.14 with iced_layershell and iced_sessionlock:
  - `main.rs`: subcommands. No argument is the shell; `setup`, `check`, `doctor`, `fragment`, `theme`, `pick`, `plugin`, `settings`, `lock`, `greet`, `timer`, and `apolline <overlay>` to open one in a running shell through the control socket (`control.rs`).
  - `shell/`: the shell's state and messages, split by topic (`handle.rs`, `views.rs`, `cards.rs`, `dock.rs`, `modes.rs`, `moving.rs` for animations, `config_watch.rs`).
  - Overlays and pieces, one file each: `bar.rs`, `launcher.rs`, `panel.rs` (quick settings), `calendar.rs`, `clips.rs`, `switcher.rs`, `machine_view.rs`, `dock_view.rs`, `notify.rs`, `osd.rs`, `polkit.rs`, `tray.rs`, …
  - `lock.rs` and `auth.rs`: the lock screen and its own PAM conversation (raw `pam::ffi`). The typed password answers the first hidden prompt; anything more PAM asks (a code, a key touch) is asked in the same field. An expired password still unlocks, and `setcred` refreshes credentials, never deletes them. The service is `APOLLINE_PAM_SERVICE`, else `apolline` if `/etc/pam.d/apolline` exists, else `login`.
  - `greet.rs`, `greetd.rs`: the login screen for greetd.
  - `settings/`: the settings window, one file per page.
  - `plugins/`: plugins in any language, a folder with `plugin.toml`; `apolline plugin install|list|enable|disable|remove`.
  - `motion.rs` (`Tween`, `Speed`), `lit.rs` (`crate::lit::button`, a drop-in for iced's button whose hover look fades; use it instead of `iced::widget::button`).
- `assets/`: the default `config.toml`, `niri.kdl` (the fragment niri includes), themes, templates for GTK/Qt/terminals, locales (`ru.toml`: every user-visible English string is a key), icons, fonts, the PAM file.
- `examples/plugins/ip` (a plugin), `examples/wired` (a full rice).
- `tools/`: `headless.sh OUT.png` screenshots the real layer-shell session under a headless sway; `fake-bus.sh` and `fake-*.py` stand in for UPower, MPRIS, power profiles, the tray and controls on a bus of their own; `niri-session.sh` records a niri session for the `live_session` test.
- `packaging/arch/apolline` (release, from the tag's tarball) and `packaging/arch/apolline-git`, each with a `.SRCINFO`.

## Before saying something is done

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI (`.github/workflows/ci.yml`) runs these in an `archlinux:latest` container as root, with no init to reap processes, then checks every config in `tests/configs` gives niri a fragment `niri validate` accepts, and builds the package with makepkg. A test that passes on macOS can fail there, so run the tests on Linux too, the way CI does:

```bash
sudo unshare -pf --mount-proc env -i PATH=/usr/bin:$HOME/.cargo/bin HOME=/root \
  CARGO_HOME=$HOME/.cargo RUSTUP_HOME=$HOME/.rustup CARGO_TARGET_DIR=/root/target-apolline \
  cargo test --workspace -q
```

Check the change live when it shows on screen: run the shell in niri (nested niri with the winit backend works), or `tools/headless.sh`. With a fresh config the shell locks after five idle minutes; set `[idle] lock_after = 0` in a test config, and before trusting a screenshot check `pgrep -af "^apolline lock"`. Test the lock with a PAM service of your own (`APOLLINE_PAM_SERVICE`), never with a real password.

## Releasing

Releases are tags; there is no GitHub Release page.

1. Everything merged to `main` (work happens on `next` and feature branches, `main` only fast-forwards), the version in `Cargo.toml` and `pkgver` in `packaging/arch/apolline/PKGBUILD` raised, `CHANGELOG.md` dated.
2. CI green on that commit on `main`.
3. `git tag -a vX.Y.Z -m "Apolline X.Y.Z"`, push the tag.
4. `sha256sum` of `https://github.com/Pisosenie/apolline/archive/vX.Y.Z.tar.gz` into the PKGBUILD, `makepkg --printsrcinfo > .SRCINFO`, build it with `makepkg` once, commit "The checksum of the X.Y.Z archive", push.

A pushed tag is never moved. If a release turns out broken, the fix is the next version.

## How the code is written

- Comments in English and few: only where the code can't say it. It should read as written by hand. Danil writes in Russian; answer him in Russian.
- Nothing runs at rest: no polling where an event exists, animations stop when they arrive, a redraw only when something changed.
- The user's files are theirs: one include line in niri's config, writes only behind a switch they turned on, every such write listed and undoable.
- Every option has a control in Settings, a sentence saying what it does, and the lines it writes.
- Every user-visible string goes through `i18n` and gets its Russian line in `assets/locales/ru.toml`.
- Files stay under about 800 lines; split by topic when they grow. Match the code around it.

## Git

- Commit and push only when Danil says so.
- The message is one plain sentence ("Show real niri workspaces in the bar"), no `feat:` prefixes, no body or a line or two at most, no co-author lines.
- The author is `Danil Zorin <45851636+Pisosenie@users.noreply.github.com>`; set it locally in each clone, the global git email is a work one. Push to `git@github.com:Pisosenie/apolline.git`.
- Never commit `.idea/` or `target/`.

## How Danil works

- Work as a senior engineer: understand the whole thing, then decide and recommend; don't hand him a menu or ask the same thing twice. Within a planned stage go from step to step without asking, and when something is left you can do yourself, do it instead of reporting it.
- Code clean, light and maintainable: the simplest structure that reads well.
- Check your own changes before calling them done: a value threaded into one branch but not its siblings, an extraction that left the original behind, a comment claiming more than the code does, a default that takes too much.
- Estimates in the pace where the agent writes the code: hours to days.
- Parallel agents cost his limit: count them before starting, keep it small.

`~/.claude/rules/*.md` (from everything-claude-code) asks for TDD with 80% coverage, `feat:` commits and parallel agents always. For this project the rules above win.
