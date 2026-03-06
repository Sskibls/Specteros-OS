# Specteros OS - Debian Live ISO Builder
# Build a real bootable Linux ISO with XFCE desktop
# Usage: docker build -t specteros-builder . && docker run --rm -v $(pwd)/output:/output --privileged specteros-builder

FROM debian:bookworm

LABEL maintainer="Specteros OS Team"
LABEL description="Specteros OS Debian Live ISO Builder"

# Prevent interactive prompts during build
ENV DEBIAN_FRONTEND=noninteractive
ENV TZ=UTC

# Install build dependencies
RUN apt-get update && apt-get install -y \
    debootstrap \
    genisoimage \
    xorriso \
    grub2-common \
    grub-pc-bin \
    grub-efi-amd64-bin \
    mtools \
    dosfstools \
    squashfs-tools \
    isolinux \
    syslinux-efi \
    wget \
    curl \
    ca-certificates \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Create working directory
WORKDIR /build

# Copy build scripts
COPY scripts/ /build/scripts/
COPY core/ /build/core/
COPY ui/ /build/ui/

# Make scripts executable
RUN chmod +x /build/scripts/*.sh

# Build argument for project root
ARG PROJECT_ROOT=/build

# Default command
CMD ["bash", "/build/scripts/build-debian-live.sh"]
