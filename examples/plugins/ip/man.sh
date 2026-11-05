#!/bin/sh
# Man pages matching the search, one JSON line each; Enter opens the page in a terminal.
[ -n "$1" ] || exit 0
tab=$(printf '\t')
# For the shell, in single quotes: one inside is ended, escaped and begun again.
quote() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }
# For JSON, backslashes and double quotes escaped.
json() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'; }
man -k "$1" 2>/dev/null | head -n 30 |
    sed -nE 's/^([^ ,(]+) ?\(([^)]+)\)[^-]* - (.*)$/\1\t\2\t\3/p' |
    while IFS="$tab" read -r name section about; do
        page="man $(quote "$section") $(quote "$name")"
        printf '{"title": "%s(%s)", "about": "%s", "icon": "book", "terminal": "%s"}\n' \
            "$(json "$name")" "$(json "$section")" "$(json "$about")" "$(json "$page")"
    done
