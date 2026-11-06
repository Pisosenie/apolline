#!/usr/bin/env bash
# Apolline on Arch from nothing but a user with sudo:
#   curl -fsSL https://raw.githubusercontent.com/Pisosenie/apolline/main/install.sh | bash
# APOLLINE_FROM=git builds the newest commit instead of the latest release.
# APOLLINE_GREETER=keep leaves the display manager you have; otherwise greetd shows apolline greet.
# APOLLINE_LIVE=no skips apolline-wall, which draws live wallpapers: videos, shaders, Wallpaper Engine.
set -euo pipefail

from=${APOLLINE_FROM:-release}
greeter=${APOLLINE_GREETER:-apolline}
live=${APOLLINE_LIVE:-yes}
work=${APOLLINE_WORK:-$HOME/.cache/apolline-install}

say() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m==>\033[0m %s\n' "$*" >&2; exit 1; }

[[ -f /etc/arch-release ]] || die "this is for Arch Linux"
[[ $EUID -ne 0 ]] || die "run it as your user, it asks for sudo when it needs it"
command -v sudo >/dev/null || die "install sudo and add yourself to it first"

say "Tools to build with"
sudo pacman -Syu --needed --noconfirm base-devel git

mkdir -p "$work"
fetch() {
  if [[ -d $work/$1/.git ]]; then
    git -C "$work/$1" pull -q --ff-only
  else
    git clone -q "https://github.com/Pisosenie/$1" "$work/$1"
  fi
}

build() {
  say "Building $(basename "$1")"
  # Tests inside a running session would see its niri, so they get none.
  (cd "$1" && env -u NIRI_SOCKET -u WAYLAND_DISPLAY makepkg -si --needed --noconfirm "${@:2}")
}

fetch vif
build "$work/vif/packaging/arch/vif-bin"

fetch apolline
if [[ $from == git ]]; then
  # The release and the git package conflict; makepkg can't answer that question itself.
  pacman -Q apolline &>/dev/null && ! pacman -Q apolline-git &>/dev/null && sudo pacman -Rdd --noconfirm apolline
  build "$work/apolline/packaging/arch/apolline-git"
else
  pacman -Q apolline-git &>/dev/null && sudo pacman -Rdd --noconfirm apolline-git
  build "$work/apolline/packaging/arch/apolline"
fi
build "$work/apolline/packaging/arch/apolline-desktop"

if [[ $live != no ]]; then
  fetch apolline-wall
  build "$work/apolline-wall/packaging/arch"
fi

say "Services"
sudo systemctl enable NetworkManager.service bluetooth.service
systemctl --user enable pipewire.socket pipewire-pulse.socket wireplumber.service 2>/dev/null || true

if [[ $greeter != keep ]]; then
  current=$(readlink /etc/systemd/system/display-manager.service 2>/dev/null || true)
  if [[ -n $current && $current != *greetd* ]]; then
    say "Replacing $(basename "$current") with greetd and apolline greet"
    sudo systemctl disable "$(basename "$current")"
  fi
  sudo install -Dm644 /usr/share/apolline/greetd.toml /etc/greetd/config.toml
  sudo systemctl enable greetd.service
fi

say "Your folders and default apps"
xdg-user-dirs-update
xdg-mime default com.system76.CosmicFiles.desktop inode/directory

say "Apolline in your niri config"
apolline setup

say "Done. Reboot, log in to niri, and apolline doctor shows what works."
