#!/bin/bash
# Install .desktop file and icons so GNOME shows the app icon
# (even when running via cargo tauri dev)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ICONS_DIR="$SCRIPT_DIR/../src-tauri/icons"

# Install icons into hicolor theme
install -Dm644 "$ICONS_DIR/32x32.png" "$HOME/.local/share/icons/hicolor/32x32/apps/lcars-voice.png"
install -Dm644 "$ICONS_DIR/64x64.png" "$HOME/.local/share/icons/hicolor/64x64/apps/lcars-voice.png"
install -Dm644 "$ICONS_DIR/128x128.png" "$HOME/.local/share/icons/hicolor/128x128/apps/lcars-voice.png"
install -Dm644 "$ICONS_DIR/128x128@2x.png" "$HOME/.local/share/icons/hicolor/256x256/apps/lcars-voice.png"
install -Dm644 "$ICONS_DIR/icon.png" "$HOME/.local/share/icons/hicolor/512x512/apps/lcars-voice.png"

# Install .desktop file
cat > "$HOME/.local/share/applications/lcars-voice.desktop" << 'EOF'
[Desktop Entry]
Name=LCARS Voice
Comment=Voice recording and transcription
Exec=lcars-voice
Icon=lcars-voice
Terminal=false
Type=Application
Categories=Utility;Audio;
StartupWMClass=lcars-voice
EOF

# Refresh caches
gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null
update-desktop-database "$HOME/.local/share/applications" 2>/dev/null

echo "Desktop entry and icons installed."
echo "You may need to log out and back in, or restart GNOME Shell (Alt+F2 → r) for the icon to appear."
