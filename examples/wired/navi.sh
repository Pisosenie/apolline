#!/bin/sh
# A NAVI booting into the Wired: a slow log in the terminal's own reds.
red='\033[31m'; bright='\033[91m'; dim='\033[2m'; off='\033[0m'
say() { printf "%b\n" "$1"; sleep "${2:-0.08}"; }
clear
say "${bright}  ██╗    ██╗██╗██████╗ ███████╗██████╗ ${off}" 0.02
say "${bright}  ██║    ██║██║██╔══██╗██╔════╝██╔══██╗${off}" 0.02
say "${red}  ██║ █╗ ██║██║██████╔╝█████╗  ██║  ██║${off}" 0.02
say "${red}  ██║███╗██║██║██╔══██╗██╔══╝  ██║  ██║${off}" 0.02
say "${red}  ╚███╔███╔╝██║██║  ██║███████╗██████╔╝${off}" 0.02
say "${dim}   ╚══╝╚══╝ ╚═╝╚═╝  ╚═╝╚══════╝╚═════╝ ${off}" 0.3
say ""
say "${dim}NAVI ver. 7.0  copper / psyche${off}" 0.4
say "${red}[ ok ]${off} protocol 7 ............ loaded"
say "${red}[ ok ]${off} schumann resonance .... 7.83 Hz"
say "${red}[ ok ]${off} knights ............... silent"
say "${red}[ ok ]${off} layer 01..07 .......... mounted"
say "${bright}[ ?? ]${off} who is lain ........... unknown" 0.5
say ""
say "${dim}> present day, present time${off}" 0.6
say "${bright}> no matter where you go, everyone's connected.${off}"
say ""
exec sh -c 'printf "\033[31mlain@wired\033[0m:\033[2m~\033[0m$ "; cat > /dev/null'
