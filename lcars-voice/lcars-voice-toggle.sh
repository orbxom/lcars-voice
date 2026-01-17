#!/bin/bash
# LCARS Voice Toggle Script
# - If app not running: start it
# - If app running: toggle recording via xdotool

APP_NAME="lcars-voice"
PROJECT_DIR="$HOME/personal/claude-tools/lcars-voice"

# Check if already running
if pgrep -f "target/debug/$APP_NAME" > /dev/null; then
    # App is running - touch toggle file to trigger recording toggle
    touch /tmp/lcars-voice-toggle
else
    # App not running - start it
    cd "$PROJECT_DIR" || exit 1
    cargo tauri dev &
fi
