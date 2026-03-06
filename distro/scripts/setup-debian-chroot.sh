#!/data/data/com.termux/files/usr/bin/bash
#
# PhantomKernel OS - Debian Chroot Launcher for Termux
#

set -euo pipefail

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║      PhantomKernel OS - Debian on Termux                    ║"
echo "║      Real Debian Desktop with GUI Apps                    ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# Configuration
SPECTEROS_DIR="$HOME/os"
BIN_DIR="$SPECTEROS_DIR/target/release"

# Check if PhantomKernel is built
if [[ ! -f "$BIN_DIR/phantomkernel-tui" ]]; then
    echo "Building PhantomKernel binaries..."
    cd "$SPECTEROS_DIR"
    cargo build --release -p phantomkernel-tui -p phantomkernel-shell
fi

# Copy binaries to Debian chroot
echo "Installing to Debian chroot..."
CHROOT_BIN="/data/data/com.termux/files/usr/tmp/proot-distro/debian/opt/phantomkernel/bin"
mkdir -p "$CHROOT_BIN"

cp "$BIN_DIR/phantomkernel-tui" "$CHROOT_BIN/"
cp "$BIN_DIR/phantomkernel-shell" "$CHROOT_BIN/"
cp "$BIN_DIR/gkctl" "$CHROOT_BIN/" 2>/dev/null || true
chmod +x "$CHROOT_BIN"/*

# Create launcher script in Debian
cat > "/data/data/com.termux/files/usr/tmp/proot-distro/debian/usr/local/bin/phantomkernel" << 'EOF'
#!/bin/bash
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║         PhantomKernel OS - Debian Edition                   ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""
echo "Available commands:"
echo "  phantomkernel-tui    - Terminal dashboard"
echo "  phantomkernel-shell  - Interactive CLI"
echo "  gkctl              - System control"
echo ""
echo "Starting TUI..."
/opt/phantomkernel/bin/phantomkernel-tui
EOF

chmod +x "/data/data/com.termux/files/usr/tmp/proot-distro/debian/usr/local/bin/phantomkernel"

# Create desktop launcher
cat > "/data/data/com.termux/files/usr/tmp/proot-distro/debian/usr/share/applications/phantomkernel-tui.desktop" << 'EOF'
[Desktop Entry]
Version=1.0
Type=Application
Name=PhantomKernel TUI
Comment=PhantomKernel Terminal Dashboard
Exec=/opt/phantomkernel/bin/phantomkernel-tui
Icon=utilities-terminal
Terminal=true
Categories=System;Monitor;
EOF

# Install dependencies in Debian
echo "Installing dependencies in Debian..."
proot-distro login debian -- apt update
proot-distro login debian -- apt install -y \
    dbus-x11 \
    x11-xserver-utils \
    xdg-utils \
    2>/dev/null || true

# Summary
echo ""
echo "═══════════════════════════════════════════════════════════"
echo "Installation Complete"
echo "═══════════════════════════════════════════════════════════"
echo ""
echo "To run PhantomKernel OS in Debian chroot:"
echo ""
echo "  proot-distro login debian -- phantomkernel"
echo ""
echo "Or for full desktop with XFCE:"
echo ""
echo "  1. Install VNC in Debian:"
echo "     proot-distro login debian -- apt install -y xfce4 xfce4-goodies tigervnc-standalone-server"
echo ""
echo "  2. Start VNC:"
echo "     proot-distro login debian -- vncserver :1 -geometry 1920x1080"
echo ""
echo "  3. Connect VNC client to localhost:5901"
echo ""
echo "═══════════════════════════════════════════════════════════"
