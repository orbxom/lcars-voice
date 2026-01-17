#!/bin/bash
# LCARS Voice Toggle Script
# Launches the app in dev mode if not already running

APP_NAME="lcars-voice"
PROJECT_DIR="$HOME/personal/claude-tools/lcars-voice"

# Check if already running
if pgrep -f "target/debug/$APP_NAME" > /dev/null; then
    echo "LCARS Voice is already running"
    exit 0
fi

# Start the app in dev mode
cd "$PROJECT_DIR" || exit 1
cargo tauri dev &
