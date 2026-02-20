#!/bin/bash
# LCARS Voice Toggle Script
# - If app not running: start it
# - If app running: toggle recording via Unix socket

APP_NAME="lcars-voice"
PROJECT_DIR="$HOME/personal/claude-tools/lcars-voice"
SOCKET_PATH="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/lcars-voice.sock"

start_app() {
    cd "$PROJECT_DIR" || exit 1
    cargo tauri dev &>/dev/null &
}

# Check if socket exists and app is listening
if [[ -S "$SOCKET_PATH" ]]; then
    # Socket file exists - try to send toggle with a timeout
    if command -v socat &>/dev/null; then
        if ! echo "toggle" | socat -T2 - UNIX-CONNECT:"$SOCKET_PATH" 2>/dev/null; then
            # Connection failed or timed out - stale socket
            rm -f "$SOCKET_PATH"
            start_app
        fi
    elif command -v python3 &>/dev/null; then
        if ! timeout 2 python3 -c "
import socket
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.settimeout(2)
s.connect('$SOCKET_PATH')
s.send(b'toggle\n')
s.close()
" 2>/dev/null; then
            # Connection failed - stale socket
            rm -f "$SOCKET_PATH"
            start_app
        fi
    else
        notify-send "LCARS Voice" "Install socat: sudo apt install socat" 2>/dev/null
    fi
else
    # No socket - app not running, start it
    start_app
fi
