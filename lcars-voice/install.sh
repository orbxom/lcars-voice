#!/usr/bin/env bash
set -euo pipefail

# LCARS Voice Installer
# One-line install: curl -sSL https://raw.githubusercontent.com/orbxom/lcars-voice/main/lcars-voice/install.sh | bash
# With options:     curl -sSL ... | bash -s -- --cpu --no-keybinding

REPO="orbxom/lcars-voice"
APP="lcars-voice"
INSTALL_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor"

# --- Output helpers ---
if [[ -t 1 ]]; then
    GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; BOLD='\033[1m'; NC='\033[0m'
else
    GREEN=''; YELLOW=''; RED=''; BOLD=''; NC=''
fi

info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*" >&2; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }
step()  { echo -e "\n${BOLD}==> $*${NC}"; }

# --- Argument parsing ---
FORCE_VARIANT=""
INSTALL_KEYBINDING=true
INSTALL_DEPS=true

usage() {
    cat <<'USAGE'
LCARS Voice Installer

Usage: install.sh [OPTIONS]

Options:
  --cpu              Force CPU-only variant (no CUDA)
  --cuda             Force CUDA variant (requires NVIDIA GPU + drivers)
  --no-keybinding    Skip GNOME Super+Alt+H keybinding setup
  --no-deps          Skip runtime dependency installation
  --help, -h         Show this help

One-line install:
  curl -sSL https://raw.githubusercontent.com/orbxom/lcars-voice/main/lcars-voice/install.sh | bash

With options:
  curl -sSL https://raw.githubusercontent.com/orbxom/lcars-voice/main/lcars-voice/install.sh | bash -s -- --cpu
  curl -sSL https://raw.githubusercontent.com/orbxom/lcars-voice/main/lcars-voice/install.sh | bash -s -- --no-keybinding
  curl -sSL https://raw.githubusercontent.com/orbxom/lcars-voice/main/lcars-voice/install.sh | bash -s -- --cuda --no-deps
USAGE
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --cpu)           FORCE_VARIANT="cpu"; shift ;;
        --cuda)          FORCE_VARIANT="cuda"; shift ;;
        --no-keybinding) INSTALL_KEYBINDING=false; shift ;;
        --no-deps)       INSTALL_DEPS=false; shift ;;
        --help|-h)       usage ;;
        *)               error "Unknown option: $1. Use --help for usage." ;;
    esac
done

# --- Step 1: Detect GPU variant ---
step "Step 1/6: Detecting GPU variant..."

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

if [[ -n "$FORCE_VARIANT" ]]; then
    VARIANT="$FORCE_VARIANT"
else
    VARIANT=$(detect_variant)
fi
info "Selected variant: $VARIANT"

# --- Step 2: Find latest release ---
step "Step 2/6: Finding latest release..."

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

# --- Step 3: Install runtime dependencies ---
step "Step 3/6: Installing runtime dependencies..."

install_runtime_deps() {
    if [[ "$INSTALL_DEPS" != true ]]; then
        info "Skipping dependency installation (--no-deps)"
        return
    fi

    if ! command -v apt-get &>/dev/null; then
        warn "Cannot auto-install dependencies (apt-get not found)."
        warn "Please manually install: xclip, libnotify (notify-send), socat"
        return
    fi

    local deps_needed=()

    command -v xclip &>/dev/null       || deps_needed+=(xclip)
    command -v notify-send &>/dev/null || deps_needed+=(libnotify-bin)
    command -v socat &>/dev/null       || deps_needed+=(socat)
    command -v dconf &>/dev/null       || deps_needed+=(dconf-cli)

    if (( ${#deps_needed[@]} == 0 )); then
        info "All runtime dependencies already installed."
        return
    fi

    info "Installing: ${deps_needed[*]}"
    sudo apt-get update -qq
    sudo apt-get install -y -qq "${deps_needed[@]}"
}

install_runtime_deps

# --- Step 4: Download and install ---
step "Step 4/6: Downloading and installing lcars-voice..."

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

    # Auto-install FUSE if needed and possible
    if ! command -v fusermount &>/dev/null && [[ ! -f /usr/lib/libfuse.so.2 ]]; then
        if command -v apt-get &>/dev/null && [[ "$INSTALL_DEPS" == true ]]; then
            info "Installing libfuse2 (required for AppImage)..."
            sudo apt-get install -y -qq libfuse2
        else
            warn "AppImage requires FUSE. Install libfuse2 manually."
        fi
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

# --- Step 5: Desktop integration ---
step "Step 5/6: Setting up desktop integration..."

install_icons() {
    local base_url="https://raw.githubusercontent.com/$REPO/main/$APP/src-tauri/icons"

    info "Installing application icons..."

    local -a sizes=("32x32" "64x64" "128x128" "256x256" "512x512")
    local -a files=("32x32.png" "64x64.png" "128x128.png" "128x128@2x.png" "icon.png")

    for i in "${!sizes[@]}"; do
        local size="${sizes[$i]}"
        local file="${files[$i]}"
        local target_dir="${ICON_DIR}/${size}/apps"
        mkdir -p "$target_dir"
        curl -sSL --fail -o "${target_dir}/lcars-voice.png" "${base_url}/${file}" 2>/dev/null || \
            warn "Failed to download ${size} icon"
    done

    gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
}

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

    install_icons

    update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
    info "Desktop entry installed."
}

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

install_desktop_entry
install_toggle

# --- Step 6: Keybinding setup ---
step "Step 6/6: Configuring keybinding..."

install_keybinding() {
    if [[ "$INSTALL_KEYBINDING" != true ]]; then
        info "Skipping keybinding setup (--no-keybinding)"
        return
    fi

    if ! command -v dconf &>/dev/null; then
        warn "dconf not found. Keybinding setup requires GNOME."
        warn "Manually bind Super+Alt+H to: $INSTALL_DIR/${APP}-toggle"
        return
    fi

    local desktop="${XDG_CURRENT_DESKTOP:-unknown}"
    case "$desktop" in
        *GNOME*|*Unity*|*Budgie*|*Cinnamon*|*Pantheon*)
            ;;
        *)
            warn "Desktop environment '$desktop' may not support GNOME keybindings."
            warn "Attempting anyway. Manually bind Super+Alt+H to: $INSTALL_DIR/${APP}-toggle"
            ;;
    esac

    local toggle_path="$INSTALL_DIR/${APP}-toggle"
    local kb_path="/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice"

    dconf write "${kb_path}/name" "'LCARS Voice Toggle'"
    dconf write "${kb_path}/command" "'$toggle_path'"
    dconf write "${kb_path}/binding" "'<Super><Alt>h'"

    local existing
    existing=$(gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings 2>/dev/null || echo "@as []")

    if [[ "$existing" == "@as []" ]]; then
        gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings \
            "['${kb_path}/']"
    elif [[ "$existing" != *"lcars-voice"* ]]; then
        local new_list
        new_list=$(echo "$existing" | sed "s|]$|, '${kb_path}/']|")
        gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "$new_list"
    fi

    info "Keybinding installed: Super+Alt+H"
}

install_keybinding

# --- Summary ---
echo ""
echo -e "${GREEN}=== Installation complete ===${NC}"
echo "  Variant:    $VARIANT"
echo "  Version:    $VERSION"
echo "  Binary:     $(command -v lcars-voice 2>/dev/null || echo "$INSTALL_DIR/$APP")"
echo "  Toggle:     $INSTALL_DIR/${APP}-toggle"
[[ "$INSTALL_KEYBINDING" == true ]] && echo "  Keybinding: Super+Alt+H"
echo ""
echo "  Run 'lcars-voice' to start, or press Super+Alt+H to toggle."
echo ""
