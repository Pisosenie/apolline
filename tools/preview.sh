#!/usr/bin/env bash
# A catalog theme as the session shows it, for its preview: under a headless sway, with a config
# of its own and nothing of yours. CI runs it on every theme a pull request brings.
# usage: tools/preview.sh catalog/FOLDER OUT.jpg   (Linux: sway and grim; build first)
set -eu
folder=$(realpath "$1")
out=$(realpath -m "$2")
root=$(cd "$(dirname "$0")/.." && pwd)
apolline=${APOLLINE:-${CARGO_TARGET_DIR:-$root/target}/debug/apolline}

run=$(mktemp -d)
chmod 700 "$run"
export XDG_RUNTIME_DIR=$run XDG_CONFIG_HOME=$run/config XDG_STATE_HOME=$run/state
export XDG_CACHE_HOME=$run/cache XDG_DATA_HOME=$run/data
unset WAYLAND_DISPLAY NIRI_SOCKET SWAYSOCK DISPLAY
cleanup() {
    kill $(jobs -p) 2>/dev/null || true
    wait 2>/dev/null || true
    rm -rf "$run"
}
trap cleanup EXIT

name=$(sed -n 's/^name *= *"\(.*\)"$/\1/p' "$folder/theme.toml" | head -1)
"$apolline" theme install "$folder" > /dev/null
"$apolline" theme "$name" > /dev/null

printf 'output HEADLESS-1 resolution 1920x1200 scale 2\ndefault_border none\n' > "$run/sway.conf"
# Arch gives sway a file capability, which a container won't run; a copy has none.
cp "$(command -v sway)" "$run/sway"
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
    "$run/sway" -c "$run/sway.conf" > "$run/sway.log" 2>&1 &
for _ in $(seq 100); do
    compgen -G "$run/wayland-[0-9]" > /dev/null && break
    sleep 0.1
done
WAYLAND_DISPLAY=$(basename "$(compgen -G "$run/wayland-[0-9]" | head -1)") || {
    tail -n 20 "$run/sway.log" >&2
    exit 1
}
export WAYLAND_DISPLAY

ICED_BACKEND=tiny-skia "$apolline" > "$run/apolline.log" 2>&1 &
sleep 5
case $out in
*.jpg | *.jpeg) grim -s 1 -t jpeg -q 88 "$out" ;;
*) grim -s 1 "$out" ;;
esac
if ! kill -0 $! 2> /dev/null; then
    echo "apolline didn't stay up for $name:" >&2
    tail -n 20 "$run/apolline.log" >&2
    exit 1
fi
echo "$out"
