#!/bin/bash
# LCARS Voice Toggle Script
# - If app not running: start it
# - If app running: toggle recording via Unix socket

APP_NAME="lcars-voice"
PROJECT_DIR="$HOME/personal/claude-tools/lcars-voice"
SOCKET_PATH="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/lcars-voice.sock"

# Check if socket exists and app is listening (most reliable indicator)
if [[ -S "$SOCKET_PATH" ]]; then
    # App is running - send toggle via socket
    if command -v socat &>/dev/null; then
        echo "toggle" | socat - UNIX-CONNECT:"$SOCKET_PATH" 2>/dev/null
    elif command -v python3 &>/dev/null; then
        python3 -c "
import socket
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect('$SOCKET_PATH')
s.send(b'toggle\n')
s.close()
" 2>/dev/null
    else
        notify-send "LCARS Voice" "Install socat: sudo apt install socat" 2>/dev/null
    fi
else
    # App not running - start it
    cd "$PROJECT_DIR" || exit 1
    cargo tauri dev &>/dev/null &
fi
