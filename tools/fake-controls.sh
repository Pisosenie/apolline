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
    status)
        sink=\$(cat $state/sink 2>/dev/null || echo 47)
        mark() { [ "\$1" = "\$sink" ] && echo "*" || echo " "; }
        printf 'Audio\n ├─ Sinks:\n │  %s   47. Speakers                [vol: 0.45]\n │  %s   52. WH-1000XM5              [vol: 0.55]\n │\n ├─ Sources:\n │  *   48. Built-in Microphone     [vol: 1.00]\n │\n └─ Streams:\n' "\$(mark 47)" "\$(mark 52)" ;;
    set-default) echo "\$last" > $state/sink ;;
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
    *"IN-USE,SSID,SIGNAL,SECURITY device wifi list"*)
        [ "\$(cat $state/wifi)" = enabled ] && printf '*:%s:82:WPA2\n :Neighbours:64:WPA2\n :Cafe\\: Free:41:\n' "\$(cat $state/ssid 2>/dev/null || echo Polina)" ;;
    *"device wifi list"*) [ "\$(cat $state/wifi)" = enabled ] && printf 'no:Neighbours\nyes:%s\n' "\$(cat $state/ssid 2>/dev/null || echo Polina)" ;;
    "-t -f NAME,TYPE connection show") printf 'Polina:802-11-wireless\nWired:802-3-ethernet\n' ;;
    "connection up id "*) shift 3; echo "\$*" > $state/ssid ;;
    "--ask device wifi connect "*)
        read -r password
        if [ "\$password" = polina ]; then shift 4; echo "\$*" > $state/ssid; echo "Device 'wlan0' successfully activated"
        else echo "Error: Connection activation failed: Secrets were required, but not provided." >&2; exit 4; fi ;;
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
    "devices Connected") [ "\$(cat $state/bluetooth)" = yes ] && [ "\$(cat $state/bt_connected 2>/dev/null || echo yes)" = yes ] && echo "Device 88:C9:E8:00:00:01 WH-1000XM5" ;;
    "devices Paired") printf 'Device 88:C9:E8:00:00:01 WH-1000XM5\nDevice 00:1B:66:00:00:02 Keychron K2\n' ;;
    "connect 88:C9:E8:00:00:01") echo yes > $state/bt_connected ;;
    "disconnect 88:C9:E8:00:00:01") echo no > $state/bt_connected ;;
    "connect "*) echo "Failed to connect: org.bluez.Error.Failed br-connection-page-timeout"; exit 1 ;;
esac
exit 0
EOF

chmod +x "$dir"/wpctl "$dir"/brightnessctl "$dir"/nmcli "$dir"/bluetoothctl
echo "$dir"
