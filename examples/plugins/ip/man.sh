#!/bin/sh
# Man pages matching the search, one JSON line each; Enter opens the page in a terminal.
[ -n "$1" ] || exit 0
tab=$(printf '\t')
man -k "$1" 2>/dev/null | head -n 30 |
    sed -nE 's/^([^ ,(]+) ?\(([^)]+)\)[^-]* - (.*)$/\1\t\2\t\3/p' |
    while IFS="$tab" read -r name section about; do
        about=$(printf '%s' "$about" | sed 's/\\/\\\\/g; s/"/\\"/g')
        printf '{"title": "%s(%s)", "about": "%s", "icon": "book", "terminal": "man %s %s"}\n' \
            "$name" "$section" "$about" "$section" "$name"
    done
