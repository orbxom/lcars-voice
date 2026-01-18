#!/bin/bash
# Install Super+H keybinding to launch LCARS Voice

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LAUNCH_SCRIPT="$SCRIPT_DIR/lcars-launch.sh"

# Add custom keybinding
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/name "'LCARS Voice Launch'"
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/command "'$LAUNCH_SCRIPT'"
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/binding "'<Super>h'"

# Get existing keybindings and append ours
EXISTING=$(gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings)
if [[ "$EXISTING" == "@as []" ]]; then
    gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings \
        "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/']"
elif [[ "$EXISTING" != *"lcars-voice"* ]]; then
    NEW_LIST=$(echo "$EXISTING" | sed "s/]$/, '\/org\/gnome\/settings-daemon\/plugins\/media-keys\/custom-keybindings\/lcars-voice\/']/" )
    gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "$NEW_LIST"
fi

echo "Keybinding installed. Restart gsd-media-keys to activate:"
echo "  killall gsd-media-keys && /usr/libexec/gsd-media-keys &"
