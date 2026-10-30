#!/usr/bin/env bash
# Install stand-ins for wpctl, nmcli, bluetoothctl and brightnessctl into DIR,
# so quick settings can be tried on a machine without audio, Wi-Fi or a backlight.
# usage: tools/fake-controls.sh DIR   then run apolline with PATH=DIR:$PATH
set -euo pipefail

dir=$(realpath -m "$1")
state=$dir/state
mkdir -p "$state"
echo 0.45 > "$state/volume"
echo 0 > "$state/muted"
echo 60 > "$state/brightness"
echo enabled > "$state/wifi"
echo yes > "$state/bluetooth"

cat > "$dir/wpctl" <<EOF
#!/usr/bin/env bash
echo "wpctl \$*" >> "$state/log"
case "\$1" in
    get-volume)
        muted=""; [ "\$(cat $state/muted)" = 1 ] && muted=" [MUTED]"
        echo "Volume: \$(cat $state/volume)\$muted" ;;
    set-volume) echo "\${@: -1}" > "$state/volume" ;;
    set-mute) echo "\${@: -1}" > "$state/muted" ;;
esac
EOF

cat > "$dir/brightnessctl" <<EOF
#!/usr/bin/env bash
echo "brightnessctl \$*" >> "$state/log"
case "\$*" in
    *set*) value="\${@: -1}"; echo "\${value%\\%}" > "$state/brightness" ;;
    *) echo "panel,backlight,\$(cat $state/brightness),\$(cat $state/brightness)%,100" ;;
esac
EOF

cat > "$dir/nmcli" <<EOF
#!/usr/bin/env bash
echo "nmcli \$*" >> "$state/log"
case "\$*" in
    "-t -f WIFI radio") cat "$state/wifi" ;;
    "radio wifi on") echo enabled > "$state/wifi" ;;
    "radio wifi off") echo disabled > "$state/wifi" ;;
    *"device wifi list"*) [ "\$(cat $state/wifi)" = enabled ] && printf 'no:Neighbours\nyes:Polina\n' ;;
esac
exit 0
EOF

cat > "$dir/bluetoothctl" <<EOF
#!/usr/bin/env bash
echo "bluetoothctl \$*" >> "$state/log"
case "\$*" in
    show) printf 'Controller 00:00:00:00:00:01 (public)\n\tPowered: %s\n' "\$(cat $state/bluetooth)" ;;
    "power on") echo yes > "$state/bluetooth" ;;
    "power off") echo no > "$state/bluetooth" ;;
    "devices Connected") [ "\$(cat $state/bluetooth)" = yes ] && echo "Device 88:C9:E8:00:00:01 WH-1000XM5" ;;
esac
exit 0
EOF

chmod +x "$dir"/wpctl "$dir"/brightnessctl "$dir"/nmcli "$dir"/bluetoothctl
echo "$dir"
