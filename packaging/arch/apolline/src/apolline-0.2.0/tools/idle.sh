#!/usr/bin/env bash
# What a shell costs while nobody touches it: CPU over a minute and memory, the same way for any of them.
# usage: tools/idle.sh [SECONDS] NAME...   e.g. tools/idle.sh 60 apolline quickshell qs dms
# Leave the mouse and keyboard alone while it runs. Memory is PSS, so shared libraries count once.
set -euo pipefail

seconds=60
if [[ ${1:-} =~ ^[0-9]+$ ]]; then
    seconds=$1
    shift
fi
[ $# -gt 0 ] || set -- apolline
hz=$(getconf CLK_TCK)

pids() { pgrep -x "$1" || true; }
ticks() {
    local sum=0
    for pid in "$@"; do
        [ -r "/proc/$pid/stat" ] || continue
        read -r -a f < <(sed 's/^.*) //' "/proc/$pid/stat")
        sum=$((sum + f[11] + f[12]))
    done
    echo "$sum"
}
pss() {
    local sum=0
    for pid in "$@"; do
        kb=$(awk '/^Pss:/ {print $2}' "/proc/$pid/smaps_rollup" 2>/dev/null || echo 0)
        sum=$((sum + kb))
    done
    echo "$sum"
}

declare -A before
for name in "$@"; do
    mapfile -t list < <(pids "$name")
    before[$name]=$(ticks "${list[@]}")
done
echo "measuring for ${seconds}s; hands off..."
sleep "$seconds"

printf '%-14s %6s %8s %10s\n' process pids "cpu %" "pss MiB"
for name in "$@"; do
    mapfile -t list < <(pids "$name")
    if [ ${#list[@]} -eq 0 ]; then
        printf '%-14s %6s\n' "$name" "-"
        continue
    fi
    used=$(( $(ticks "${list[@]}") - ${before[$name]} ))
    cpu=$(awk -v t="$used" -v hz="$hz" -v s="$seconds" 'BEGIN { printf "%.2f", 100 * t / hz / s }')
    mem=$(awk -v kb="$(pss "${list[@]}")" 'BEGIN { printf "%.1f", kb / 1024 }')
    printf '%-14s %6s %8s %10s\n' "$name" "${#list[@]}" "$cpu" "$mem"
done
