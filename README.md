# 👻 SpecterOS - Privacy-First Linux Distribution

<div align="center">

[![Website](https://img.shields.io/badge/Website-specter--os.web.app-blue?style=for-the-badge)](https://specter-os.web.app)
[![Version](https://img.shields.io/badge/Version-0.2.0-green?style=for-the-badge)](https://github.com/Sskibls/Specter-OS/releases)
[![License](https://img.shields.io/badge/License-Apache%202.0-red?style=for-the-badge)](LICENSE)

**🌐 Official Website: [https://specter-os.web.app](https://specter-os.web.app)**

</div>

---

**A secure, privacy-focused Debian-based Linux operating system**

---

## 🎯 Project Overview

SpecterOS is a **real, bootable Linux distribution** based on Debian 13 (Trixie) with:

- ✅ **XFCE Desktop Environment** - Lightweight, fast, customizable
- ✅ **Persona Shards** - Isolated environments for different identities
- ✅ **Network Kill Switch** - Instant network disconnect
- ✅ **Audit Logging** - Tamper-evident security events
- ✅ **Encrypted Storage** - LUKS2 full disk encryption option
- ✅ **Privacy Tools** - DNS over HTTPS, Tor integration, leak protection

---

## 🏗️ Build SpecterOS ISO

### Option 1: On Debian/Ubuntu System

```bash
# Install build tools
sudo apt update
sudo apt install -y debootstrap genisoimage xorriso grub2-common mtools squashfs-tools

# Build the ISO
cd /path/to/specteros/distro
sudo ./scripts/build-debian-live.sh

# Output: ./output/specteros-debian-YYYYMMDD.iso
```

### Option 2: With Docker (Anywhere)

```bash
cd /path/to/specteros/distro

# Build builder image
docker build -t specteros-builder .

# Run build
docker run --rm -v $(pwd)/output:/output --privileged specteros-builder

# Output: ./output/specteros-debian-YYYYMMDD.iso
```

---

## 🖥️ Test the ISO

### QEMU (Local)
```bash
qemu-system-x86_64 -cdrom output/specteros-debian-*.iso -m 4096 -boot d
```

### QEMU + VNC (Remote)
```bash
qemu-system-x86_64 -cdrom output/specteros-debian-*.iso -m 4096 -boot d -vnc :0
# Connect VNC client to: localhost:5900
```

### VirtualBox/VMware
- Create new VM → Linux → Debian 64-bit
- Memory: 4GB, Disk: 64GB
- Select ISO as boot disk
- Start VM

---

## 📦 What's Included

### Base System
| Component | Version |
|-----------|---------|
| Base | Debian 12 (Bookworm) |
| Kernel | Linux 6.1 LTS |
| Desktop | XFCE 4.18 |
| Bootloader | GRUB2 |

### SpecterOS Components
| Binary | Purpose |
|--------|---------|
| `specteros-tui` | Terminal dashboard |
| `specteros-shell` | Interactive CLI |
| `specteros-shardd` | Persona shard manager |
| `specteros-netd` | Network policy daemon |
| `specteros-policyd` | Permission broker |
| `specteros-auditd` | Audit logging |
| `specteros-guardian` | Emergency modes |
| `specteros-updated` | A/B updates |
| `spctl` | Control utility |

### Desktop Apps
- Firefox ESR (privacy-hardened)
- LibreOffice (Writer, Calc, Impress)
- Thunderbird (encrypted email)
- Thunar (file manager)
- Terminator (terminal)

---

## ⌨️ Default Credentials

| Account | Username | Password |
|---------|----------|----------|
| Live Session | user | user |
| Root (Live) | root | specter |

---

## 🎮 Keyboard Shortcuts

### SpecterOS Shortcuts
| Shortcut | Action |
|----------|--------|
| `Super + P` | **PANIC Mode** - Kill network, lock shards |
| `Super + M` | **MASK Mode** - Decoy desktop |
| `Super + T` | **TRAVEL Mode** - Ephemeral sessions |
| `Super + K` | **Kill Switch** - Block all network |
| `Super + L` | Lock screen |

---

## 🔐 Security Features

- ✅ **Persona Shards** - Work/Anon/Burner/Lab isolation
- ✅ **Mandatory Access Control** - AppArmor/SELinux
- ✅ **Full Disk Encryption** - LUKS2
- ✅ **Secure Boot** - UEFI secure boot support
- ✅ **Audit Framework** - Tamper-evident logging
- ✅ **Network Isolation** - Per-shard routing
- ✅ **DNS Privacy** - DNS over HTTPS
- ✅ **Kill Switch** - Hardware-level network disconnect

---

## 📊 System Requirements

### Minimum
- CPU: 2 cores (x86_64)
- RAM: 2 GB
- Disk: 20 GB

### Recommended
- CPU: 4 cores
- RAM: 4 GB
- Disk: 64 GB SSD

---

## 🛠️ Project Structure

```
specteros/
├── core/
│   ├── daemons/          # System daemons
│   │   ├── specteros-init/
│   │   ├── specteros-shardd/
│   │   ├── specteros-netd/
│   │   ├── specteros-policyd/
│   │   ├── specteros-airlockd/
│   │   ├── specteros-auditd/
│   │   ├── specteros-guardian/
│   │   └── specteros-updated/
│   └── libs/             # Security libraries
│       ├── sp-crypto/
│       ├── sp-policy/
│       ├── sp-ipc/
│       └── sp-audit/
├── ui/
│   ├── tui/              # Terminal UI
│   ├── desktop/          # GTK4 Desktop
│   └── themes/           # Visual themes
├── distro/
│   ├── scripts/          # Build scripts
│   ├── Dockerfile        # Docker builder
│   └── README-DEBIAN.md  # Build docs
├── packaging/
│   ├── installer/        # Install scripts
│   ├── image-build/      # ISO builders
│   └── vnc-web/          # Web VNC server
└── editions/
    ├── debian/           # Debian packages
    └── fedora/           # RPM packages
```

---

## 📄 License

Apache 2.0 - SpecterOS Project

---

## 🔗 Links

- Website: https://specter-os.web.app/
- Docs: https://specter-os.web.app/docs/
- GitHub: https://github.com/specteros/os
- Issues: https://github.com/specteros/os/issues

---

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md)

---

**SpecterOS - Your Privacy, Our Mission** 👻
