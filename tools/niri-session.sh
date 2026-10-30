#!/usr/bin/env bash
# Records niri's event stream while windows open, move and close, for the IPC test.
# Needs sway (headless, as the screen niri runs in), niri and foot. Usage: niri-session.sh OUT.jsonl
set -eu
out=$(realpath -m "${1:-niri-session.jsonl}")
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
# Arch gives sway a file capability, which a container won't run; a copy has none.
cp "$(command -v sway)" "$run/sway"
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
    "$run/sway" -c "$run/sway.conf" > "$run/sway.log" 2>&1 &
wait_for "$run/wayland-[0-9]"
outer=$(basename "$(compgen -G "$run/wayland-[0-9]" | head -1)")

: > "$run/niri.kdl"
WAYLAND_DISPLAY=$outer LIBGL_ALWAYS_SOFTWARE=1 niri -c "$run/niri.kdl" > "$run/niri.log" 2>&1 &
wait_for "$run/niri.*.sock"
export NIRI_SOCKET
NIRI_SOCKET=$(compgen -G "$run/niri.*.sock" | head -1)
inner=$(basename "$NIRI_SOCKET" | cut -d. -f2)

niri msg -j event-stream > "$out" &
sleep 1
act() { niri msg action "$@" >/dev/null; sleep 0.4; }
WAYLAND_DISPLAY=$inner foot -a session-one >/dev/null 2>&1 &
one=$!
sleep 1.5
WAYLAND_DISPLAY=$inner foot -a session-two >/dev/null 2>&1 &
two=$!
sleep 1.5
act focus-column-left
act focus-workspace 2
act focus-workspace 1
act move-window-to-workspace 2
act focus-workspace 1
act toggle-window-floating
act screenshot-screen
sleep 1
close() {
    kill "$1" 2>/dev/null || true
    for _ in $(seq 30); do
        kill -0 "$1" 2>/dev/null || break
        sleep 0.1
    done
    kill -9 "$1" 2>/dev/null || true
}
close $two
close $one
for _ in $(seq 100); do
    niri msg -j windows | grep -q '"session-' || break
    sleep 0.1
done
sleep 0.5
niri msg windows >&2
echo "recorded $(wc -l < "$out") events into $out"
