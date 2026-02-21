#!/usr/bin/env bash
set -euo pipefail

REPO="orbxom/claude-tools"
APP="lcars-voice"
INSTALL_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor"

info()  { echo "[INFO]  $*"; }
warn()  { echo "[WARN]  $*" >&2; }
error() { echo "[ERROR] $*" >&2; exit 1; }

# --- Step 1: Detect GPU ---
detect_variant() {
    if command -v nvidia-smi &>/dev/null && nvidia-smi &>/dev/null; then
        echo "cuda"
    elif lspci 2>/dev/null | grep -qi 'nvidia'; then
        warn "NVIDIA GPU detected but nvidia-smi not found."
        warn "Install NVIDIA drivers for GPU acceleration, then re-run this script."
        echo "cpu"
    else
        echo "cpu"
    fi
}

VARIANT=$(detect_variant)
info "Selected variant: $VARIANT"

# --- Step 2: Find latest release ---
get_latest_tag() {
    curl -sSL "https://api.github.com/repos/$REPO/releases" \
        | grep -oP '"tag_name":\s*"\K'"$APP"'-v[^"]+' \
        | head -1
}

TAG=$(get_latest_tag)
if [[ -z "$TAG" ]]; then
    error "No $APP release found at https://github.com/$REPO/releases"
fi
VERSION="${TAG#${APP}-v}"
info "Latest release: $TAG (version $VERSION)"

# --- Step 3: Choose format and install ---
install_deb() {
    local deb_name="${APP}_${VERSION}_amd64-${VARIANT}.deb"
    local url="https://github.com/$REPO/releases/download/$TAG/$deb_name"
    local tmp
    tmp=$(mktemp "/tmp/${APP}-XXXXX.deb")

    info "Downloading $deb_name ..."
    curl -sSL --fail -o "$tmp" "$url" || error "Download failed: $url"

    info "Installing .deb (requires sudo) ..."
    sudo dpkg -i "$tmp" || sudo apt-get install -f -y
    rm -f "$tmp"
    info "Installed to $(command -v lcars-voice || echo '/usr/bin/lcars-voice')"
}

install_appimage() {
    local ai_name="${APP}_${VERSION}_amd64-${VARIANT}.AppImage"
    local url="https://github.com/$REPO/releases/download/$TAG/$ai_name"

    # Check FUSE requirement
    if ! command -v fusermount &>/dev/null && [[ ! -f /usr/lib/libfuse.so.2 ]]; then
        warn "AppImage requires FUSE. You may need: sudo apt install libfuse2"
    fi

    mkdir -p "$INSTALL_DIR"

    info "Downloading $ai_name ..."
    curl -sSL --fail -o "$INSTALL_DIR/$APP.AppImage" "$url" || error "Download failed: $url"
    chmod +x "$INSTALL_DIR/$APP.AppImage"

    # Create wrapper script so 'lcars-voice' works in PATH
    cat > "$INSTALL_DIR/$APP" << 'WRAPPER'
#!/bin/bash
exec "$HOME/.local/bin/lcars-voice.AppImage" "$@"
WRAPPER
    chmod +x "$INSTALL_DIR/$APP"
    info "Installed to $INSTALL_DIR/$APP"
}

if command -v dpkg &>/dev/null && command -v apt-get &>/dev/null; then
    install_deb
else
    install_appimage
fi

# --- Step 4: Install desktop entry (AppImage path; deb handles this automatically) ---
install_desktop_entry() {
    mkdir -p "$DESKTOP_DIR"
    cat > "$DESKTOP_DIR/${APP}.desktop" << 'EOF'
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
    update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
    info "Desktop entry installed."
}

install_desktop_entry

# --- Step 5: Install toggle script ---
install_toggle() {
    mkdir -p "$INSTALL_DIR"
    local toggle_path="$INSTALL_DIR/${APP}-toggle"
    cat > "$toggle_path" << 'TOGGLE'
#!/bin/bash
APP_NAME="lcars-voice"
SOCKET_PATH="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/lcars-voice.sock"

# Find binary
if command -v lcars-voice &>/dev/null; then
    BINARY="$(command -v lcars-voice)"
elif [[ -x "/usr/bin/lcars-voice" ]]; then
    BINARY="/usr/bin/lcars-voice"
elif [[ -x "$HOME/.local/bin/lcars-voice" ]]; then
    BINARY="$HOME/.local/bin/lcars-voice"
else
    notify-send "LCARS Voice" "Binary not found. Re-run install script." 2>/dev/null
    exit 1
fi

start_app() {
    export DISPLAY="${DISPLAY:-:1}"
    "$BINARY" &>/dev/null &
    disown
}

if [[ -S "$SOCKET_PATH" ]]; then
    if command -v socat &>/dev/null; then
        if ! echo "toggle" | socat -T2 - UNIX-CONNECT:"$SOCKET_PATH" 2>/dev/null; then
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
            rm -f "$SOCKET_PATH"
            start_app
        fi
    else
        notify-send "LCARS Voice" "Install socat: sudo apt install socat" 2>/dev/null
    fi
else
    start_app
fi
TOGGLE
    chmod +x "$toggle_path"
    info "Toggle script installed to $toggle_path"
}

install_toggle

# --- Step 6: Optional keybinding setup ---
echo ""
echo "=== Installation complete ==="
echo "  Variant: $VARIANT"
echo "  Run:     lcars-voice"
echo "  Toggle:  lcars-voice-toggle"
echo ""

if [[ -t 0 ]]; then
    read -rp "Set up Super+Alt+H keybinding for GNOME? [y/N] " answer
    if [[ "$answer" =~ ^[Yy]$ ]]; then
        TOGGLE_PATH="$INSTALL_DIR/${APP}-toggle"

        dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/name \
            "'LCARS Voice Toggle'"
        dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/command \
            "'$TOGGLE_PATH'"
        dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/binding \
            "'<Super><Alt>h'"

        EXISTING=$(gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings 2>/dev/null || echo "@as []")
        if [[ "$EXISTING" == "@as []" ]]; then
            gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings \
                "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/']"
        elif [[ "$EXISTING" != *"lcars-voice"* ]]; then
            NEW_LIST=$(echo "$EXISTING" | sed "s/]$/, '\/org\/gnome\/settings-daemon\/plugins\/media-keys\/custom-keybindings\/lcars-voice\/']/" )
            gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "$NEW_LIST"
        fi
        info "Keybinding installed: Super+Alt+H"
    fi
else
    echo "  To set up the GNOME keybinding, download and run this script directly:"
    echo "  curl -sSL https://raw.githubusercontent.com/$REPO/master/$APP/install.sh -o /tmp/install-lcars.sh && bash /tmp/install-lcars.sh"
fi
