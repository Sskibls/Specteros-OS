#!/bin/bash
#
# SpecterOS - Debian-based Linux Distribution Builder
# Creates a real bootable Debian Live ISO with XFCE desktop
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DISTRO_NAME="specteros-os"
VERSION="0.2.0"
DEBIAN_SUITE="trixie"
ARCH="amd64"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║      SpecterOS - Debian Live ISO Builder                  ║"
echo "║      Real Linux Distribution with XFCE Desktop            ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# Check if running as root
if [[ $EUID -ne 0 ]]; then
    log_error "This script must be run as root"
    exit 1
fi

# Check required tools
log_step "Checking required tools..."
REQUIRED_TOOLS="debootstrap genisoimage xorriso grub-mkrescue"
MISSING=""

for tool in $REQUIRED_TOOLS; do
    if ! command -v "$tool" &> /dev/null; then
        MISSING="$MISSING $tool"
    fi
done

if [[ -n "$MISSING" ]]; then
    log_error "Missing tools:$MISSING"
    echo ""
    echo "Install with:"
    echo "  apt install -y debootstrap genisoimage xorriso grub2-common mtools"
    exit 1
fi

log_success "All required tools found"

# Create working directories
WORK_DIR="/var/tmp/specteros-build-$$"
ROOTFS="$WORK_DIR/rootfs"
ISO_ROOT="$WORK_DIR/iso_root"

mkdir -p "$WORK_DIR" "$ROOTFS" "$ISO_ROOT"

cleanup() {
    if [[ -d "$WORK_DIR" ]]; then
        rm -rf "$WORK_DIR"
    fi
}
trap cleanup EXIT

log_step "Creating SpecterOS (Debian $DEBIAN_SUITE)..."

# Bootstrap Debian base system (Debian 12+ needs non-free-firmware for firmware packages)
log_step "Bootstrapping Debian base system..."
debootstrap --include=systemd,grub2,linux-image-$ARCH,firmware-linux-nonfree \
    --components=main,contrib,non-free,non-free-firmware \
    $DEBIAN_SUITE "$ROOTFS" http://deb.debian.org/debian/ 2>&1 | tee /tmp/debootstrap.log

log_success "Debian base installed"

# Copy SpecterOS binaries
log_step "Installing SpecterOS components..."
if [[ -d "$PROJECT_ROOT/target/release" ]]; then
    mkdir -p "$ROOTFS/opt/specteros/bin"
    cp "$PROJECT_ROOT/target/release"/specteros-* "$ROOTFS/opt/specteros/bin/" 2>/dev/null || true
    cp "$PROJECT_ROOT/target/release/gkctl" "$ROOTFS/opt/specteros/bin/" 2>/dev/null || true
    cp "$PROJECT_ROOT/target/release/specteros-tui" "$ROOTFS/opt/specteros/bin/" 2>/dev/null || true
    log_success "SpecterOS binaries copied"
fi

# Install desktop environment (XFCE - lightweight)
log_step "Installing XFCE desktop environment..."
chroot "$ROOTFS" apt-get update

# Fix broken packages first
chroot "$ROOTFS" apt-get install -f -y 2>/dev/null || true

# Install desktop without LibreOffice (Java conflicts)
chroot "$ROOTFS" apt-get install -y \
    task-xfce-desktop \
    lightdm \
    firefox-esr \
    terminator \
    thunar \
    mousepad \
    pavucontrol \
    network-manager \
    network-manager-gnome \
    sudo \
    curl \
    wget \
    git \
    vim \
    htop \
    isolinux \
    syslinux \
    syslinux-efi \
    grub-pc-bin \
    grub-efi-amd64-bin \
    mtools \
    dosfstools \
    2>&1 | tee /tmp/apt-install.log

# Install LibreOffice separately with --force-bad-versions
chroot "$ROOTFS" apt-get install -y --fix-broken libreoffice-core libreoffice-common 2>&1 | tee -a /tmp/apt-install.log || log_warning "LibreOffice installation skipped due to dependencies"

log_success "Desktop environment installed"

# Configure system
log_step "Configuring system..."

# Set hostname
echo "specteros" > "$ROOTFS/etc/hostname"

# Set hosts
cat > "$ROOTFS/etc/hosts" << EOF
127.0.0.1   localhost
127.0.1.1   specteros
::1         localhost ip6-localhost ip6-loopback
EOF

# Set timezone
echo "UTC" > "$ROOTFS/etc/timezone"
chroot "$ROOTFS" dpkg-reconfigure -f noninteractive tzdata

# Set root password
echo "root:specteros" | chroot "$ROOTFS" chpasswd

# Create user
chroot "$ROOTFS" useradd -m -s /bin/bash -G sudo,audio,video,dialout user
echo "user:user" | chroot "$ROOTFS" chpasswd

# Configure sudo
echo "user ALL=(ALL) NOPASSWD:ALL" > "$ROOTFS/etc/sudoers.d/user"
chmod 440 "$ROOTFS/etc/sudoers.d/user"

# Enable services
chroot "$ROOTFS" systemctl enable lightdm
chroot "$ROOTFS" systemctl enable NetworkManager

# Create SpecterOS configuration
mkdir -p "$ROOTFS/etc/specteros"
cat > "$ROOTFS/etc/specteros/config.toml" << 'EOF'
[general]
hostname = "specteros"
theme = "fsociety"
log_level = "info"
audit_enabled = true

[shards]
default_shards = ["work", "anon", "burner", "lab"]

[network]
default_route = "direct"
kill_switch_default = false
dns_over_https = true
ipv6_disabled = false

[security]
secure_boot = false
tpm_required = false
full_disk_encryption = false
selinux = false
apparmor = true

[desktop]
environment = "xfce"
auto_start_tui = false
show_privacy_indicator = true
EOF

# Create systemd services for SpecterOS
mkdir -p "$ROOTFS/etc/systemd/system"
for daemon in shardd netd policyd auditd guardian; do
    cat > "$ROOTFS/etc/systemd/system/specteros-${daemon}.service" << EOF
[Unit]
Description=SpecterOS ${daemon^}
After=network.target

[Service]
Type=simple
ExecStart=/opt/specteros/bin/specteros-${daemon}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
done

# Create welcome script
cat > "$ROOTFS/usr/local/bin/specteros-welcome" << 'EOF'
#!/bin/bash
# SpecterOS OS Welcome Script

if [[ ! -f ~/.specteros-welcomed ]]; then
    zenity --info --title="Welcome to SpecterOS OS" \
        --text="Welcome to SpecterOS OS!\n\nA privacy-focused Linux distribution.\n\nQuick Start:\n• specteros-tui - Terminal dashboard\n• specteros-shell - Interactive CLI\n• Settings - System configuration\n\nPrivacy Features:\n✓ Persona Shards\n✓ Network Kill Switch\n✓ Audit Logging" \
        --width=400
    
    touch ~/.specteros-welcomed
fi
EOF
chmod +x "$ROOTFS/usr/local/bin/specteros-welcome"

# Add to XFCE autostart
mkdir -p "$ROOTFS/etc/xdg/autostart"
cat > "$ROOTFS/etc/xdg/autostart/specteros-welcome.desktop" << 'EOF'
[Desktop Entry]
Type=Application
Name=SpecterOS Welcome
Exec=/usr/local/bin/specteros-welcome
Terminal=false
X-GNOME-Autostart-enabled=true
EOF

# Create GRUB configuration
log_step "Configuring GRUB bootloader..."
chroot "$ROOTFS" grub-install /dev/sdX 2>/dev/null || true

cat > "$ROOTFS/boot/grub/grub.cfg" << 'EOF'
set timeout=5
set default=0

menuentry "SpecterOS OS (Live)" {
    linux /live/vmlinuz boot=live quiet splash
    initrd /live/initrd.img
}

menuentry "SpecterOS OS (Live - Failsafe)" {
    linux /live/vmlinuz boot=live quiet failsafe
    initrd /live/initrd.img
}

menuentry "SpecterOS OS (Install)" {
    linux /install/vmlinuz quiet
    initrd /install/initrd.gz
}
EOF

# Create initramfs
log_step "Creating initramfs..."
chroot "$ROOTFS" update-initramfs -u -k all

# Copy kernel and initrd to ISO root
mkdir -p "$ISO_ROOT/live"
cp "$ROOTFS/boot/vmlinuz"* "$ISO_ROOT/live/vmlinuz"
cp "$ROOTFS/boot/initrd"* "$ISO_ROOT/live/initrd.img"

# Create SquashFS for live system
log_step "Creating live filesystem..."
chroot "$ROOTFS" apt-get install -y squashfs-tools
mksquashfs "$ROOTFS" "$ISO_ROOT/live/filesystem.squashfs" -comp xz -b 1024k

# Create ISO
log_step "Building ISO image..."
mkdir -p "$PROJECT_ROOT/output"

# Copy ISOLINUX bootloader and create boot directory
mkdir -p "$ISO_ROOT/isolinux"
cp "$ROOTFS/usr/lib/ISOLINUX/isohdpfx.bin" "$ISO_ROOT/"
cp "$ROOTFS/usr/lib/ISOLINUX/isolinux.bin" "$ISO_ROOT/isolinux/"
cp "$ROOTFS/usr/lib/ISOLINUX/ldlinux.c32" "$ISO_ROOT/isolinux/"
cp "$ROOTFS/usr/lib/syslinux/modules/bios/menu.c32" "$ISO_ROOT/isolinux/"
cp "$ROOTFS/usr/lib/syslinux/modules/bios/chain.c32" "$ISO_ROOT/isolinux/"
cat > "$ISO_ROOT/isolinux/isolinux.cfg" << 'EOF'
UI menu.c32
PROMPT 0
TIMEOUT 50
LABEL specteros-live
  MENU LABEL SpecterOS (Live)
  LINUX /live/vmlinuz
  INITRD /live/initrd.img
  APPEND boot=live quiet splash
LABEL specteros-failsafe
  MENU LABEL SpecterOS (Failsafe)
  LINUX /live/vmlinuz
  INITRD /live/initrd.img
  APPEND boot=live quiet failsafe
EOF

xorriso -as mkisofs \
    -iso-level 3 \
    -rock \
    -J \
    -l \
    -D \
    -N \
    -V "SPECTEROS" \
    -b isolinux/isolinux.bin \
    -c isolinux/boot.cat \
    -no-emul-boot \
    -boot-load-size 4 \
    -boot-info-table \
    -eltorito-alt-boot \
    -e EFI/efi.img \
    -no-emul-boot \
    -isohybrid-mbr "$ISO_ROOT/isohdpfx.bin" \
    -o "$PROJECT_ROOT/output/specteros-os-debian-$(date +%Y%m%d).iso" \
    "$ISO_ROOT" 2>&1 | tee /tmp/iso-build.log

log_success "ISO created: $PROJECT_ROOT/output/specteros-os-debian-$(date +%Y%m%d).iso"

# Generate checksum
cd "$PROJECT_ROOT/output"
sha256sum specteros-os-debian-*.iso > specteros-os-debian-$(date +%Y%m%d).iso.sha256
log_info "SHA256: $(cat specteros-os-debian-*.sha256)"

# Summary
echo ""
echo "═══════════════════════════════════════════════════════════"
echo "           SpecterOS OS Build Complete"
echo "═══════════════════════════════════════════════════════════"
echo ""
echo "Output: $PROJECT_ROOT/output/specteros-os-debian-$(date +%Y%m%d).iso"
ls -lh "$PROJECT_ROOT/output/"*.iso
echo ""
echo "To test with QEMU:"
echo "  qemu-system-x86_64 -cdrom $PROJECT_ROOT/output/specteros-os-debian-*.iso -m 2G -boot d"
echo ""
echo "To install to disk:"
echo "  Boot from ISO and select 'Install' option"
echo "═══════════════════════════════════════════════════════════"
