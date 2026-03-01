#!/bin/bash
# LCARS Voice Toggle Script
# - If app not running: start it
# - If app running: toggle recording via Unix socket

APP_NAME="lcars-voice"
SOCKET_PATH="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/lcars-voice.sock"
LOG="/tmp/lcars-toggle.log"

# Find binary: installed location, PATH, or dev build
if command -v lcars-voice &>/dev/null; then
    BINARY="$(command -v lcars-voice)"
elif [[ -x "/usr/bin/lcars-voice" ]]; then
    BINARY="/usr/bin/lcars-voice"
elif [[ -x "$HOME/.local/bin/lcars-voice" ]]; then
    BINARY="$HOME/.local/bin/lcars-voice"
else
    notify-send "LCARS Voice" "Binary not found. Run the install script." 2>/dev/null
    exit 1
fi

start_app() {
    # Ensure display is set for GUI apps launched from GNOME keybindings
    export DISPLAY="${DISPLAY:-:1}"
    echo "$(date) Starting $BINARY" >> "$LOG"
    "$BINARY" >> "$LOG" 2>&1 &
    disown
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
