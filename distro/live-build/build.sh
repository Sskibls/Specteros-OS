#!/bin/bash
# SpecterOS Live ISO Builder using live-build

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
BUILD_DIR="$HOME/specteros-live"

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║      SpecterOS - Debian Live ISO Builder                  ║"
echo "║      Using live-build for fast builds                     ║"
echo "╚═══════════════════════════════════════════════════════════╝"

# Check if running as root
if [[ $EUID -ne 0 ]]; then
    echo "This script must be run as root"
    exit 1
fi

# Install dependencies if needed
if ! command -v lb &> /dev/null; then
    echo "[INSTALL] Installing live-build..."
    apt update
    apt install -y live-build debootstrap squashfs-tools isolinux syslinux-common dosfstools mtools xorriso grub-pc-bin grub-efi-amd64-bin
fi

# Create build directory
echo "[SETUP] Creating build directory..."
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

# Configure if not already configured
if [[ ! -d config ]]; then
    echo "[CONFIG] Setting up live-build configuration..."
    lb config \
        --mode debian \
        --distribution trixie \
        --archive-areas "main contrib non-free non-free-firmware" \
        --mirror-bootstrap http://deb.debian.org/debian \
        --mirror-chroot http://deb.debian.org/debian \
        --bootappend-live "boot=live components quiet splash" \
        --iso-application "SpecterOS" \
        --iso-publisher "SpecterOS Project" \
        --iso-volume "SPECTEROS" \
        --apt-indices false \
        --memtest none
fi

# Create package list
echo "[PACKAGES] Creating package list..."
mkdir -p config/package-lists
cat > config/package-lists/specteros.list.chroot << 'EOF'
# XFCE Desktop (minimal)
xfce4
xfce4-terminal
thunar
mousepad
lightdm
lightdm-gtk-greeter

# Applications
firefox-esr
pavucontrol
network-manager
network-manager-gnome

# System
sudo
curl
wget
git
vim
htop
python3
python3-gi
python3-gi-cairo
python3-psutil
wmctrl

# Firmware
firmware-linux
firmware-linux-nonfree
firmware-iwlwifi

# Calamares Installer
calamares
calamares-settings-debian
grub-pc
grub-efi-amd64
partition-manager

# Whisker menu
whiskermenu
EOF

# Copy SpecterOS binaries for the hook
echo "[BINARIES] Copying SpecterOS binaries..."
mkdir -p "$BUILD_DIR/specteros-binaries"
if [[ -d "$PROJECT_ROOT/target/release" ]]; then
    cp "$PROJECT_ROOT/target/release"/specteros-* "$BUILD_DIR/specteros-binaries/" 2>/dev/null || true
    cp "$PROJECT_ROOT/target/release/gkctl" "$BUILD_DIR/specteros-binaries/" 2>/dev/null || true
    echo "  Copied $(ls "$BUILD_DIR/specteros-binaries" | wc -l) binaries"
else
    echo "  Warning: No SpecterOS binaries found in $PROJECT_ROOT/target/release"
fi

# Copy hooks
echo "[HOOKS] Installing SpecterOS hooks..."
mkdir -p config/hooks/live

# Custom UI (hand-crafted, not AI slop)
cp "$SCRIPT_DIR/config/hooks/live/specteros-custom-ui.hook.chroot" config/hooks/live/

# Hardened browser
cp "$SCRIPT_DIR/config/hooks/live/specteros-browser.hook.chroot" config/hooks/live/

# SpecterOS binaries hook
cp "$SCRIPT_DIR/config/hooks/live/specteros.hook.chroot" config/hooks/live/

# Calamares installer
cp "$SCRIPT_DIR/config/hooks/live/specteros-calamares.hook.chroot" config/hooks/live/

chmod +x config/hooks/live/*.hook.chroot

# Clean previous build (optional - comment out for faster rebuilds)
echo "[CLEAN] Cleaning previous build..."
lb clean --purge || true

# Build
echo "[BUILD] Building ISO... (this may take 15-20 minutes)"
lb build

# Rename output
echo "[OUTPUT] Renaming output file..."
if [[ -f live-image-amd64.hybrid.iso ]]; then
    OUTPUT_NAME="specteros-os-0.2.0-$(date +%Y%m%d).iso"
    mv live-image-amd64.hybrid.iso "$BUILD_DIR/$OUTPUT_NAME"
    
    # Generate checksum
    cd "$BUILD_DIR"
    sha256sum "$OUTPUT_NAME" > "$OUTPUT_NAME.sha256"
    
    echo ""
    echo "═══════════════════════════════════════════════════════════"
    echo "           SpecterOS ISO Build Complete!"
    echo "═══════════════════════════════════════════════════════════"
    echo ""
    echo "Output: $BUILD_DIR/$OUTPUT_NAME"
    echo "SHA256: $(cat $OUTPUT_NAME.sha256)"
    echo ""
    echo "To test with QEMU:"
    echo "  qemu-system-x86_64 -cdrom $BUILD_DIR/$OUTPUT_NAME -m 4096 -boot d"
    echo ""
    echo "═══════════════════════════════════════════════════════════"
else
    echo "ERROR: ISO build failed!"
    exit 1
fi
