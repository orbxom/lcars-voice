#!/bin/bash
# Launch LCARS Voice in dev mode if not running

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

if ! pgrep -f "lcars-voice" > /dev/null; then
    cd "$PROJECT_DIR/src-tauri" && cargo tauri dev &
fi
