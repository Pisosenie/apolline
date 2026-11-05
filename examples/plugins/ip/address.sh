#!/bin/sh
# The public address as Waybar JSON, a plain line, or copied.
address=$(curl -fsS --max-time 5 "https://${APOLLINE_SERVICE:-ifconfig.me}" 2>/dev/null)
case "$1" in
--copy)
    [ -n "$address" ] && printf '%s' "$address" | wl-copy
    ;;
--line)
    echo "Public address ${address:-unknown}"
    ;;
*)
    if [ -z "$address" ]; then
        echo '{"text": "offline", "class": "warning"}'
    else
        printf '{"text": "%s %s", "tooltip": "From %s; a click copies it"}\n' \
            "${APOLLINE_LABEL:-IP}" "$address" "$APOLLINE_SERVICE"
    fi
    ;;
esac
