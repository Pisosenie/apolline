#!/usr/bin/env bash
# Screenshot the real layer-shell session under a throwaway headless sway.
# usage: tools/headless.sh OUT.png [apolline args...]   (Linux, needs sway and grim)
set -euo pipefail

out=$(realpath -m "$1")
shift
root=$(cd "$(dirname "$0")/.." && pwd)
target=${CARGO_TARGET_DIR:-$root/target}
runtime=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
sockets() { ls "$runtime" | grep -E '^wayland-[0-9]+$' || true; }

cargo build --manifest-path "$root/Cargo.toml" --quiet

conf=$(mktemp)
printf 'output HEADLESS-1 resolution 1440x900 scale 1\ndefault_border none\n' > "$conf"
before=$(sockets)
WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman sway -c "$conf" > /dev/null 2>&1 &
sway=$!
trap 'kill $sway 2> /dev/null; rm -f "$conf"' EXIT

for _ in $(seq 50); do
    display=$(comm -13 <(echo "$before") <(sockets) | head -1)
    [ -n "$display" ] && break
    sleep 0.1
done
export WAYLAND_DISPLAY=${display:?sway did not start}

"$target/debug/apolline" "$@" > /dev/null 2>&1 &
sleep 3
grim "$out"
kill $! 2> /dev/null
echo "$out"
