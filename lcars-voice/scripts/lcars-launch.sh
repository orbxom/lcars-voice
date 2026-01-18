#!/bin/bash
# Launch LCARS Voice in dev mode if not running

PROJECT_DIR="$HOME/personal/claude-tools/lcars-voice"

if ! pgrep -f "lcars-voice" > /dev/null; then
    cd "$PROJECT_DIR/src-tauri" && cargo tauri dev &
fi
