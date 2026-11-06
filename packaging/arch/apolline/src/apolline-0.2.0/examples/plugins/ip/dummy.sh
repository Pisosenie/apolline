#!/bin/sh
# A tile's state is a file: there is on, gone is off.
file="${XDG_RUNTIME_DIR:-/tmp}/apolline-ip-dummy"
case "$1" in
state)
    if [ -e "$file" ]; then
        echo '{"on": true, "detail": "Since '"$(date -r "$file" +%H:%M)"'"}'
    else
        echo off
    fi
    ;;
toggle)
    if [ -e "$file" ]; then rm "$file"; else touch "$file"; fi
    ;;
esac
