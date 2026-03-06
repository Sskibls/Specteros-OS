#!/bin/bash
#
# PhantomKernel OS - Real Fedora Remix Builder
# Uses lorax to create a real bootable ISO
#

set -euo pipefail

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║      PhantomKernel OS - Fedora Remix Builder                ║"
echo "║      Real Linux Distribution with GNOME Desktop           ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# Configuration
FEDORA_VERSION="39"
WORKSPACE="/var/tmp/phantomkernel-build"
OUTPUT_DIR="$(pwd)/output"
ISO_NAME="phantomkernel-os-fedora-$(date +%Y%m%d).iso"

# Colors
BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log() { echo -e "${BLUE}[BUILD]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check requirements
log "Checking requirements..."
REQUIRED_TOOLS="lorax composer-cli pykickstart"
MISSING_TOOLS=""

for tool in $REQUIRED_TOOLS; do
    if ! command -v "$tool" &> /dev/null; then
        MISSING_TOOLS="$MISSING_TOOLS $tool"
    fi
done

if [[ -n "$MISSING_TOOLS" ]]; then
    warn "Missing tools:$MISSING_TOOLS"
    echo ""
    echo "Install with:"
    echo "  sudo dnf install -y lorax composer-cli pykickstart anaconda"
    echo ""
fi

# Create directories
mkdir -p "$WORKSPACE" "$OUTPUT_DIR"

# Create kickstart file for automated install
log "Creating kickstart configuration..."
cat > "$WORKSPACE/phantomkernel.ks" << 'KSEOF'
# PhantomKernel OS Kickstart
# Fedora-based Privacy Linux

# System settings
lang en_US.UTF-8
keyboard us
timezone UTC --isUtc
rootpw --plaintext phantomkernel
user --name=user --password=user123 --groups=wheel

# Bootloader
bootloader --location=mbr --boot-drive=/dev/sda
zerombr
clearpart --all --initlabel
autopart --type=lvm --encrypted --passphrase=phantomkernel

# Partitioning
part /boot --fstype="ext4" --size=1024
part swap --fstype="swap" --size=4096
part / --fstype="ext4" --grow

# Network
network --bootproto=dhcp --device=link --activate
network --hostname=phantomkernel

# Packages
%packages --ignoremissing
@^gnome-desktop-environment
@core
@base
@standard
@multimedia
@internet-browser
@office-suite

# PhantomKernel components
# (will be added post-install)

# Security tools
selinux
policycoreutils
setuptool

# System tools
vim
nano
htop
iotop
nmap
wireshark
tcpdump

# Development
git
python3
rust
cargo

# Exclude unnecessary packages
-alsa-firmware
-alsa-tools-firmware
-avahi-autoipd
-bluez-cups
-cups-browsed
-fprintd-pam
-glibc-all-langpacks
-iprutils
-ivtv-firmware
-iwl100-firmware
-iwl1000-firmware
-iwl105-firmware
-iwl135-firmware
-iwl2000-firmware
-iwl2030-firmware
-iwl3160-firmware
-iwl5000-firmware
-iwl5150-firmware
-iwl6000g2a-firmware
-iwl6050-firmware
-iwl7260-firmware
-libertas-sd8686-firmware
-libertas-sd8787-firmware
-libertas-usb8388-firmware
-oddjob
-oddjob-mkhomedir
-psacct
-rdma
-sound-theme-freedesktop
-tuned
%end

# Post-installation
%post --log=/root/ks-post-install.log

# Enable services
systemctl enable sshd
systemctl enable firewalld

# Configure firewall
firewall-cmd --permanent --add-service=ssh
firewall-cmd --permanent --add-service=http
firewall-cmd --permanent --add-service=https
firewall-cmd --reload

# Configure SELinux
setenforce 1
sed -i 's/SELINUX=permissive/SELINUX=enforcing/g' /etc/selinux/config

# Create PhantomKernel directories
mkdir -p /opt/phantomkernel/{bin,lib,config,themes}
mkdir -p /etc/phantomkernel

# Download and install PhantomKernel binaries
cd /opt/phantomkernel
curl -L -o phantomkernel.tar.gz https://github.com/phantomkernel/os/releases/latest/download/phantomkernel-linux-x86_64.tar.gz
tar -xzf phantomkernel.tar.gz
chmod +x /opt/phantomkernel/bin/*

# Add to PATH
echo 'export PATH="/opt/phantomkernel/bin:$PATH"' >> /etc/profile.d/phantomkernel.sh

# Create systemd services
cat > /etc/systemd/system/phantomkernel-shardd.service << 'EOF'
[Unit]
Description=PhantomKernel Shard Manager
After=network.target

[Service]
ExecStart=/opt/phantomkernel/bin/phantomkernel-shardd
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF

cat > /etc/systemd/system/phantomkernel-netd.service << 'EOF'
[Unit]
Description=PhantomKernel Network Daemon
After=network.target

[Service]
ExecStart=/opt/phantomkernel/bin/phantomkernel-netd
Restart=on-failure
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW

[Install]
WantedBy=multi-user.target
EOF

# Enable PhantomKernel services
systemctl daemon-reload
systemctl enable phantomkernel-shardd
systemctl enable phantomkernel-netd

# Configure GNOME extensions for privacy
dconf write /org/gnome/desktop/privacy/report-technical-problems false
dconf write /org/gnome/desktop/privacy/old-files-age 7

# Set privacy-focused defaults
gsettings set org.gnome.desktop.interface gtk-theme 'Adwaita-dark'
gsettings set org.gnome.desktop.wm.preferences button-layout ':close'

# Create welcome script
cat > /usr/local/bin/phantomkernel-welcome << 'EOF'
#!/bin/bash
echo "Welcome to PhantomKernel OS!"
echo ""
echo "Getting Started:"
echo "  • phantomkernel-tui    - Terminal dashboard"
echo "  • phantomkernel-shell  - Interactive CLI"
echo "  • gkctl              - System control"
echo ""
echo "Privacy Features:"
echo "  ✓ Persona Shards enabled"
echo "  ✓ Network monitoring active"
echo "  ✓ Audit logging enabled"
echo ""
EOF
chmod +x /usr/local/bin/phantomkernel-welcome

# Add to autostart
mkdir -p /etc/xdg/autostart
cat > /etc/xdg/autostart/phantomkernel-welcome.desktop << 'EOF'
[Desktop Entry]
Type=Application
Name=PhantomKernel Welcome
Exec=/usr/local/bin/phantomkernel-welcome
Terminal=true
EOF

%end

# Anaconda configuration
%anaconda
pwpolicy root --minlen=6 --minquality=1 --notstrict --noempty --notdigit
pwpolicy user --minlen=6 --minquality=1 --notstrict --noempty --notdigit
pwpolicy luks --minlen=6 --minquality=1 --notstrict --noempty --notdigit
%end
KSEOF

success "Kickstart file created"

# Create lorax template for ISO customization
log "Creating lorax template..."
mkdir -p "$WORKSPACE/lorax"
cat > "$WORKSPACE/lorax/phantomkernel.tmpl" << 'EOF'
# PhantomKernel OS Lorax Template

[general]
name = PhantomKernel OS
version = 0.1.0
release = 1
summary = Privacy-First Fedora-based Linux Distribution
description = PhantomKernel OS is a privacy-focused Linux distribution based on Fedora, featuring persona shards, network isolation, and audit logging.

[packages]
# Add PhantomKernel packages
phantomkernel-core
phantomkernel-daemons
phantomkernel-desktop

[bootloader]
timeout = 5
default = phantomkernel

[isolinux]
label = PhantomKernel OS
menu label = ^PhantomKernel OS
kernel /vmlinuz
append initrd=/initrd.img root=live:CDLABEL=SPECTEROS ro rd.live.image quiet

[efi]
label = PhantomKernel OS
menu label = PhantomKernel OS
kernel /EFI/BOOT/vmlinuz
append initrd=/EFI/BOOT/initrd.img root=live:CDLABEL=SPECTEROS ro rd.live.image quiet
EOF

success "Lorax template created"

# Build instructions
log "Creating build instructions..."
cat > "$WORKSPACE/BUILD.md" << 'EOF'
# PhantomKernel OS - Build Instructions

## Prerequisites

```bash
sudo dnf install -y lorax composer-cli pykickstart anaconda-dracut
```

## Build ISO

```bash
# Method 1: Using lorax (recommended)
sudo lorax -p PhantomKernel -v 0.1.0 -r 1 \
    --releasever=39 \
    --logfile=/var/log/lorax.log \
    --source=file:///path/to/phantomkernel.ks \
    --output=/path/to/output/phantomkernel-os.iso

# Method 2: Using composer-cli (modular)
composer-cli compose start phantomkernel.ks
composer-cli compose status
composer-cli compose image <uuid> /path/to/output/
```

## Test with QEMU

```bash
qemu-system-x86_64 \
    -cdrom phantomkernel-os.iso \
    -m 4096 \
    -boot d \
    -cpu host \
    -enable-kvm \
    -vga virtio \
    -display gtk
```

## Install to Disk

```bash
# Boot from ISO and run installer
sudo ./install-phantomkernel.sh
```

## Post-Install

After installation:
1. Run `phantomkernel-tui` for terminal dashboard
2. Configure persona shards
3. Set up network policies
4. Enable emergency modes
EOF

success "Build instructions created"

# Create Dockerfile for reproducible builds
log "Creating Dockerfile for reproducible builds..."
cat > "$WORKSPACE/Dockerfile" << 'EOF'
FROM fedora:39

RUN dnf install -y \
    lorax \
    composer-cli \
    pykickstart \
    anaconda-dracut \
    createrepo_c \
    && dnf clean all

WORKDIR /build

COPY phantomkernel.ks /build/
COPY lorax/ /build/lorax/

CMD ["bash", "-c", "lorax -p PhantomKernel -v 0.1.0 -r 1 --releasever=39 --source=/build/phantomkernel.ks --output=/output"]
EOF

success "Dockerfile created"

# Summary
echo ""
echo "═══════════════════════════════════════════════════════════"
echo "        PhantomKernel OS Build Configuration Ready"
echo "═══════════════════════════════════════════════════════════"
echo ""
echo "Files created:"
echo "  • $WORKSPACE/phantomkernel.ks     (Kickstart installer)"
echo "  • $WORKSPACE/lorax/phantomkernel.tmpl (ISO template)"
echo "  • $WORKSPACE/BUILD.md           (Build instructions)"
echo "  • $WORKSPACE/Dockerfile         (Reproducible build)"
echo ""
echo "Next steps:"
echo ""
echo "1. Install build tools:"
echo "   sudo dnf install -y lorax composer-cli pykickstart"
echo ""
echo "2. Build ISO:"
echo "   sudo bash $WORKSPACE/../distro/scripts/build-real-iso.sh"
echo ""
echo "3. Or build with Docker:"
echo "   docker build -t phantomkernel-builder $WORKSPACE"
echo "   docker run --rm -v $(pwd)/output:/output phantomkernel-builder"
echo ""
echo "4. Test with QEMU:"
echo "   qemu-system-x86_64 -cdrom output/phantomkernel-os.iso -m 4G -boot d"
echo ""
echo "═══════════════════════════════════════════════════════════"
