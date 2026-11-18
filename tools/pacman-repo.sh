#!/usr/bin/env bash
# Builds Apolline's pacman repository: the shell from its latest release, apolline-desktop,
# apolline-wall from its main branch and vif's prebuilt package, each signed, with a signed
# database. Runs as root in an archlinux container; what it makes lands in $OUT.
#
#   PACMAN_KEY   an armored private key with no passphrase, the one that signs
#   OUT          where the repository goes (default ./repo)
#   PACKAGES     the PKGBUILD folders to build, instead of the usual four (for trying it out)
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
out=$(realpath -m "${OUT:-repo}")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

[[ -n ${PACMAN_KEY:-} ]] || { echo "PACMAN_KEY is empty: nothing would be signed" >&2; exit 1; }

pacman -Syu --disable-sandbox --noconfirm --needed base-devel git cargo clang ncurses \
    pam libxkbcommon wayland libglvnd libpipewire >/dev/null
id builder >/dev/null 2>&1 || useradd -m builder
git config --system --add safe.directory '*'

if [[ -z ${PACKAGES:-} ]]; then
    git clone -q https://github.com/Pisosenie/apolline-wall "$work/apolline-wall"
    git clone -q https://github.com/Pisosenie/vif "$work/vif"
    cp -r "$root/packaging/arch/apolline" "$root/packaging/arch/apolline-desktop" "$work/"
    PACKAGES="$work/apolline $work/apolline-desktop $work/apolline-wall/packaging/arch $work/vif/packaging/arch/vif-bin"
fi

rm -rf "$out"
mkdir -p "$out"
chown -R builder "$work" "$out"
for folder in $PACKAGES; do
    chown -R builder "$folder"
    # Dependencies are the user's pacman's to resolve; the build ones are installed above.
    su builder -c "cd '$folder' && PKGDEST='$out' makepkg --nodeps --nocheck --noconfirm --clean"
done

export GNUPGHOME="$work/gnupg"
mkdir -m 700 "$GNUPGHOME"
printf '%s\n' "$PACMAN_KEY" | gpg --batch --quiet --import
key=$(gpg --list-secret-keys --with-colons | awk -F: '/^fpr/ { print $10; exit }')

cd "$out"
for package in *.pkg.tar.zst; do
    gpg --batch --yes --detach-sign --no-armor --local-user "$key" "$package"
done
repo-add --quiet --sign --key "$key" apolline.db.tar.zst ./*.pkg.tar.zst
# Release assets can't be links: the names pacman asks for are copies.
for link in apolline.db apolline.db.sig apolline.files apolline.files.sig; do
    [[ -L $link ]] && cp --remove-destination "$(readlink "$link")" "$link"
done
gpg --armor --export "$key" > apolline.gpg
echo "$key" > apolline.fingerprint
ls -1
