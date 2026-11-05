#!/bin/sh
# The Wired is always on: how many nodes answer, as Waybar JSON.
n=$(( $(date +%S) % 7 + 3 ))
printf '{"text": "WIRED %s", "tooltip": "%s nodes answering · layer 07", "class": "good"}\n' "$n" "$n"
