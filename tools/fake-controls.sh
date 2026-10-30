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
args=("\$@"); last="\${args[-1]}"
target=volume; mute=muted
case "\$*" in *SOURCE*) target=mic; mute=mic_muted ;; esac
case "\$1" in
    get-volume)
        flag=""; [ "\$(cat $state/\$mute 2>/dev/null || echo 0)" = 1 ] && flag=" [MUTED]"
        echo "Volume: \$(cat $state/\$target 2>/dev/null || echo 1.00)\$flag" ;;
    set-volume)
        now=\$(cat $state/volume)
        case "\$last" in
            *%+) awk -v n="\$now" -v d="\${last%\%+}" 'BEGIN { v = n + d / 100; if (v > 1) v = 1; printf "%.2f\n", v }' > $state/volume ;;
            *%-) awk -v n="\$now" -v d="\${last%\%-}" 'BEGIN { v = n - d / 100; if (v < 0) v = 0; printf "%.2f\n", v }' > $state/volume ;;
            *) echo "\$last" > $state/volume ;;
        esac ;;
    set-mute)
        if [ "\$last" = toggle ]; then
            [ "\$(cat $state/\$mute 2>/dev/null || echo 0)" = 1 ] && echo 0 > $state/\$mute || echo 1 > $state/\$mute
        else
            echo "\$last" > $state/\$mute
        fi ;;
esac
EOF

cat > "$dir/brightnessctl" <<EOF
#!/usr/bin/env bash
echo "brightnessctl \$*" >> "$state/log"
args=("\$@"); last="\${args[-1]}"; now=\$(cat $state/brightness)
case "\$*" in
    *set*)
        case "\$last" in
            *%+) v=\$((now + \${last%\%+})); [ \$v -gt 100 ] && v=100 ;;
            *%-) v=\$((now - \${last%\%-})); [ \$v -lt 1 ] && v=1 ;;
            *) v=\${last%\%} ;;
        esac
        echo \$v > $state/brightness ;;
    *) echo "panel,backlight,\$now,\$now%,100" ;;
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
