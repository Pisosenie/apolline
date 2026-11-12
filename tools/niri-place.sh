#!/usr/bin/env bash
# Opens three windows in a real niri (under a headless sway) and runs the test that has a mode
# put them in their places. Usage: niri-place.sh TEST_BINARY   (the shell's unit test binary)
set -eu
test=$(realpath "$1")
run=$(mktemp -d)
chmod 700 "$run"
export XDG_RUNTIME_DIR=$run
unset WAYLAND_DISPLAY NIRI_SOCKET SWAYSOCK DISPLAY
cleanup() {
    kill $(jobs -p) 2>/dev/null || true
    wait 2>/dev/null || true
    rm -rf "$run"
}
trap cleanup EXIT

wait_for() {
    for _ in $(seq 100); do
        compgen -G "$1" >/dev/null && return 0
        sleep 0.1
    done
    echo "gave up waiting for $1" >&2
    tail -n 20 "$run"/*.log >&2 2>/dev/null || true
    exit 1
}

printf 'output HEADLESS-1 resolution 1280x800\n' > "$run/sway.conf"
cp "$(command -v sway)" "$run/sway"
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
    "$run/sway" -c "$run/sway.conf" > "$run/sway.log" 2>&1 &
wait_for "$run/wayland-[0-9]"
outer=$(basename "$(compgen -G "$run/wayland-[0-9]" | head -1)")

printf 'layout {\n    gaps 10\n}\n' > "$run/niri.kdl"
WAYLAND_DISPLAY=$outer LIBGL_ALWAYS_SOFTWARE=1 niri -c "$run/niri.kdl" > "$run/niri.log" 2>&1 &
wait_for "$run/niri.*.sock"
export NIRI_SOCKET
NIRI_SOCKET=$(compgen -G "$run/niri.*.sock" | head -1)
inner=$(basename "$NIRI_SOCKET" | cut -d. -f2)

for app in place-a place-b place-c; do
    WAYLAND_DISPLAY=$inner foot -a "$app" >/dev/null 2>&1 &
    for _ in $(seq 150); do
        niri msg -j windows | grep -q "\"$app\"" && break
        sleep 0.1
    done
done
sleep 0.5
APOLLINE_NIRI_PLACE=1 "$test" places::tests::a_real_niri_puts_them_in_their_places --exact --nocapture
niri msg windows
