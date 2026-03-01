#!/bin/bash
# Install Super+Alt+H keybinding for LCARS Voice
# - Starts the app if not running
# - Toggles recording if already running

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TOGGLE_SCRIPT="$(dirname "$SCRIPT_DIR")/lcars-voice-toggle.sh"

if [[ ! -x "$TOGGLE_SCRIPT" ]]; then
    echo "Error: Toggle script not found or not executable: $TOGGLE_SCRIPT"
    echo "Run: chmod +x $TOGGLE_SCRIPT"
    exit 1
fi

# Add custom keybinding
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/name "'LCARS Voice Toggle'"
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/command "'$TOGGLE_SCRIPT'"
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/binding "'<Super><Alt>h'"

# Get existing keybindings and append ours
EXISTING=$(gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings)
if [[ "$EXISTING" == "@as []" ]]; then
    gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings \
        "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/']"
elif [[ "$EXISTING" != *"lcars-voice"* ]]; then
    NEW_LIST=$(echo "$EXISTING" | sed "s/]$/, '\/org\/gnome\/settings-daemon\/plugins\/media-keys\/custom-keybindings\/lcars-voice\/']/" )
    gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "$NEW_LIST"
fi

echo "Keybinding installed: Super+Alt+H → LCARS Voice toggle"
echo ""
echo "To activate without logging out, run:"
echo "  killall gsd-media-keys; /usr/libexec/gsd-media-keys &"
