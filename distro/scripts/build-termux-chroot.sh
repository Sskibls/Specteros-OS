#!/data/data/com.termux/files/usr/bin/bash
#
# PhantomKernel OS - Termux Chroot Builder
# Creates a Debian chroot with X11 desktop for Termux
#

set -euo pipefail

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║   PhantomKernel OS - Termux Chroot Desktop Builder          ║"
echo "║   Run Debian XFCE desktop directly on Android             ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# Configuration
CHROOT_DIR="$HOME/phantomkernel-chroot"
SPECTEROS_BIN="$HOME/phantomkernel-bin"

# Colors
BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check requirements
log "Checking requirements..."

if ! command -v proot &> /dev/null; then
    error "proot not found. Install with: pkg install proot"
    exit 1
fi

if ! command -v wget &> /dev/null; then
    error "wget not found. Install with: pkg install wget"
    exit 1
fi

success "Requirements met"

# Create directories
log "Creating directories..."
mkdir -p "$CHROOT_DIR" "$SPECTEROS_BIN"

# Download Debian rootfs
log "Downloading Debian rootfs..."
wget -q --show-progress \
    http://cdimage.debian.org/debian-cd/current-live/amd64/iso-hybrid/debian-live-12.4.0-amd64-xfce.iso \
    -O "$HOME/phantomkernel-os.iso" || {
    warn "Could not download ISO. Using proot-debian instead..."
    
    # Use proot-debian as fallback
    if command -v pkg &> /dev/null; then
        pkg install proot-distro -y
        proot-distro install debian
        ln -sf $PREFIX/tmp/proot-distro/debian "$CHROOT_DIR"
    fi
}

# Copy PhantomKernel binaries
log "Installing PhantomKernel components..."
if [[ -d "$HOME/os/target/release" ]]; then
    cp "$HOME/os/target/release"/phantomkernel-tui "$SPECTEROS_BIN/"
    cp "$HOME/os/target/release"/phantomkernel-shell "$SPECTEROS_BIN/"
    cp "$HOME/os/target/release/gkctl" "$SPECTEROS_BIN/" 2>/dev/null || true
    chmod +x "$SPECTEROS_BIN"/*
    success "PhantomKernel binaries installed"
fi

# Create launch script
log "Creating launch script..."
cat > "$HOME/start-phantomkernel.sh" << 'EOF'
#!/data/data/com.termux/files/usr/bin/bash
# PhantomKernel OS Launcher for Termux

export DISPLAY="127.0.0.1:0"
export PULSE_SERVER="127.0.0.1"

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║         PhantomKernel OS - Termux Edition                   ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# Start X11 server (termux-x11)
if command -v termux-x11 &> /dev/null; then
    echo "Starting X11 server..."
    termux-x11 :0 -ac &
    sleep 2
fi

# Start PulseAudio server
if command -v pulseaudio &> /dev/null; then
    pulseaudio --start --exit-idle-time=-1
fi

# Launch PhantomKernel TUI
echo "Launching PhantomKernel TUI..."
cd ~/phantomkernel-bin
./phantomkernel-tui

# Or launch desktop
# ./startxfce4
EOF

chmod +x "$HOME/start-phantomkernel.sh"

# Create VNC launcher
cat > "$HOME/start-vnc.sh" << 'EOF'
#!/data/data/com.termux/files/usr/bin/bash
# PhantomKernel VNC Server

# Install TigerVNC if needed
if ! command -v vncserver &> /dev/null; then
    pkg install tigervnc -y
fi

# Set VNC password
vncpasswd << PASSWD
phantomkernel
phantomkernel
n
PASSWD

# Start VNC server
vncserver :1 -geometry 1920x1080 -depth 24

echo "VNC server started on :1"
echo "Connect to: localhost:5901"
echo "Password: phantomkernel"
EOF

chmod +x "$HOME/start-vnc.sh"

# Create installation guide
cat > "$HOME/SPECTEROS-README.md" << 'EOF'
# PhantomKernel OS - Termux Edition

## Quick Start

### Option 1: Direct TUI (Recommended)

```bash
./start-phantomkernel.sh
```

This launches the PhantomKernel TUI dashboard.

### Option 2: VNC Desktop

```bash
# First time setup
./start-vnc.sh

# Then connect with VNC client to localhost:5901
# Password: phantomkernel
```

### Option 3: Termux:X11 (Best Performance)

```bash
# Install termux-x11
pkg install x11-repo
pkg install termux-x11

# Start X11 session
termux-x11 :0 &

# Then run:
./start-phantomkernel.sh
```

## Features

- ✅ PhantomKernel TUI Dashboard
- ✅ Interactive Shell
- ✅ All security daemons
- ✅ Persona Shards
- ✅ Network monitoring
- ✅ Audit logging

## Requirements

- Termux (latest version)
- proot (`pkg install proot`)
- wget (`pkg install wget`)
- Optional: termux-x11 for GUI

## Controls

| Key | Action |
|-----|--------|
| q | Quit TUI |
| p | Panic Mode |
| m | Mask Mode |
| t | Travel Mode |
| k | Kill Switch |
| 1-4 | Switch tabs |

## Troubleshooting

### Display issues
```bash
export DISPLAY="127.0.0.1:0"
```

### Permission denied
```bash
chmod +x ~/phantomkernel-bin/*
```

### Cannot connect to X11
```bash
pkg install termux-x11-nightly
termux-x11 :0 &
```

## Next Steps

For full ISO build, run on Debian/Ubuntu system:
```bash
sudo ./distro/scripts/build-debian-iso.sh
```
EOF

# Summary
echo ""
success "Setup complete!"
echo ""
echo "═══════════════════════════════════════════════════════════"
echo "Installation Complete"
echo "═══════════════════════════════════════════════════════════"
echo ""
echo "Files created:"
echo "  • ~/start-phantomkernel.sh  - Launch TUI"
echo "  • ~/start-vnc.sh          - Start VNC server"
echo "  • ~/SPECTEROS-README.md - Full documentation"
echo ""
echo "To start:"
echo "  ./start-phantomkernel.sh"
echo ""
echo "For VNC desktop:"
echo "  ./start-vnc.sh"
echo "  Then connect VNC client to localhost:5901"
echo ""
echo "═══════════════════════════════════════════════════════════"
