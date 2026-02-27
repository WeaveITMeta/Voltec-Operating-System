# Getting Started with V-OS

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Quick Start](#quick-start)
3. [Building from Source](#building-from-source)
4. [Running in QEMU](#running-in-qemu)
5. [Running on Real Hardware](#running-on-real-hardware)
6. [Development Workflow](#development-workflow)
7. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Linux Host (Recommended)

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    cmake \
    curl \
    file \
    git \
    gperf \
    libfuse-dev \
    libhtml-parser-perl \
    libpng-dev \
    libjpeg-dev \
    meson \
    nasm \
    ninja-build \
    pkg-config \
    qemu-system-x86 \
    qemu-utils \
    syslinux-utils \
    xdg-utils \
    podman

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup default nightly
rustup component add rust-src
```

### Windows Host (via WSL2)

```powershell
# Install WSL2
wsl --install

# After restart, run the setup script
.\scripts\v-os-dev.ps1 setup
```

### macOS Host

```bash
# Install Homebrew dependencies
brew install \
    cmake \
    coreutils \
    findutils \
    gnu-sed \
    gnu-tar \
    nasm \
    qemu \
    podman

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup default nightly
rustup component add rust-src
```

---

## Quick Start

### Clone the Repository

```bash
git clone https://github.com/v-os-os/v-os.git
cd v-os
git submodule update --init --recursive
```

### Build and Run

```bash
# Build V-OS Desktop (default)
make all

# Run in QEMU
make qemu
```

That's it! You should see V-OS booting in a QEMU window.

---

## Building from Source

### Build Configurations

| Configuration | Description | Command |
|---------------|-------------|---------|
| `v-os-desktop` | Full desktop with GUI | `make all CONFIG_NAME=v-os-desktop` |
| `v-os-server` | Headless server | `make all CONFIG_NAME=v-os-server` |
| `v-os-minimal` | Minimal for testing | `make all CONFIG_NAME=v-os-minimal` |
| `desktop` | Original Redox desktop | `make all CONFIG_NAME=desktop` |

### Target Architectures

| Architecture | Target | Command |
|--------------|--------|---------|
| x86_64 | `x86_64-unknown-redox` | `make all ARCH=x86_64` |
| ARM64 | `aarch64-unknown-redox` | `make all ARCH=aarch64` |
| RISC-V | `riscv64gc-unknown-redox` | `make all ARCH=riscv64gc` |

### Build Options

```bash
# Full build with Podman (recommended)
make all PODMAN_BUILD=1

# Native build (requires all dependencies)
make all PODMAN_BUILD=0

# Parallel build
make all -j$(nproc)

# Debug build (no stripping)
make all REPO_DEBUG=1

# Offline build (no network)
make all REPO_OFFLINE=1
```

### Build Outputs

```
build/
└── x86_64/
    └── v-os-desktop/
        ├── harddrive.img      # Bootable disk image
        ├── redox-live.iso     # Live ISO (make live)
        ├── filesystem/        # Mounted filesystem
        └── repo/              # Package repository
```

---

## Running in QEMU

### Basic QEMU

```bash
make qemu
```

### QEMU Options

```bash
# With more memory
make qemu QEMU_MEM=4096

# With more CPUs
make qemu QEMU_SMP=4

# With networking
make qemu QEMU_NET=yes

# With GPU acceleration (if available)
make qemu QEMU_GPU=virtio

# Headless (serial console only)
make qemu_no_gui
```

### Manual QEMU Command

```bash
qemu-system-x86_64 \
    -machine q35 \
    -m 2048 \
    -smp 2 \
    -drive file=build/x86_64/v-os-desktop/harddrive.img,format=raw \
    -serial mon:stdio \
    -device virtio-net-pci,netdev=net0 \
    -netdev user,id=net0,hostfwd=tcp::2222-:22
```

### Debugging with GDB

```bash
# Terminal 1: Start QEMU with GDB server
make qemu QEMU_GDB=yes

# Terminal 2: Connect GDB
make gdb
```

---

## Running on Real Hardware

### Create Bootable USB

```bash
# Build live ISO
make live

# Write to USB (replace /dev/sdX with your USB device)
sudo dd if=build/x86_64/v-os-desktop/redox-live.iso of=/dev/sdX bs=4M status=progress
sync
```

### UEFI Boot

V-OS supports UEFI boot. Ensure your system is configured for UEFI mode.

### Dual Boot Setup

```bash
# Run dual-boot setup script
./scripts/dual-boot.sh
```

### Hardware Compatibility

See [HARDWARE.md](../HARDWARE.md) for tested hardware.

---

## Development Workflow

### Project Structure

```
v-os/
├── config/              # Build configurations
│   ├── v-os-desktop.toml
│   ├── v-os-server.toml
│   └── v-os-minimal.toml
├── docs/                # Documentation
├── mk/                  # Makefile includes
├── recipes/             # Package recipes
│   └── v-os/       # V-OS-specific packages
├── scripts/             # Build scripts
└── src/                 # Cookbook source
```

### Adding a Package

1. Create recipe directory:
```bash
mkdir -p recipes/mypackage
```

2. Create `recipe.toml`:
```toml
[source]
git = "https://github.com/user/mypackage.git"

[build]
template = "cargo"
```

3. Add to configuration:
```toml
# config/v-os-desktop.toml
[packages]
mypackage = {}
```

4. Build:
```bash
make r.mypackage
```

### Modifying the Kernel

The kernel is in a separate repository. To modify:

```bash
# Fetch kernel source
make f.kernel

# Edit kernel source
cd recipes/core/kernel/source

# Rebuild
make r.kernel
make image
```

### Testing Changes

```bash
# Run tests
cargo test --all

# Run in QEMU
make qemu

# Run specific package tests
make t.mypackage
```

### Code Style

```bash
# Format code
cargo fmt --all

# Run linter
cargo clippy --all-targets --all-features
```

---

## Troubleshooting

### Build Fails with "Permission Denied"

```bash
# Fix permissions
chmod +x scripts/*.sh

# Or use Podman build
make all PODMAN_BUILD=1
```

### "Rust version mismatch"

```bash
# Update Rust toolchain
rustup update nightly
rustup default nightly
```

### "FUSE not available"

```bash
# Install FUSE
sudo apt-get install libfuse-dev fuse3

# Or build without mounting
make all FSTOOLS_NO_MOUNT=1
```

### QEMU "No bootable device"

```bash
# Rebuild image
make image

# Check image exists
ls -la build/x86_64/v-os-desktop/harddrive.img
```

### "Out of disk space"

```bash
# Clean build artifacts
make clean

# Full clean
make distclean
```

### Podman Issues

```bash
# Reset Podman
podman system reset

# Rebuild container
rm -f build/container.tag
make all PODMAN_BUILD=1
```

### Getting Help

- **Documentation**: [docs/](../docs/)
- **Issues**: [GitHub Issues](https://github.com/v-os-os/v-os/issues)
- **Chat**: [Matrix](https://matrix.to/#/#v-os:matrix.org)

---

## Next Steps

- Read the [ARCHITECTURE.md](ARCHITECTURE.md) to understand the system design
- Check [ROADMAP.md](../ROADMAP.md) for planned features
- See [CONTRIBUTING.md](../CONTRIBUTING.md) to contribute
- Explore [DRIVERS.md](DRIVERS.md) to write drivers
- Review [SECURITY.md](SECURITY.md) for security model
- Follow [VIRTUALBOX_GUIDE.md](VIRTUALBOX_GUIDE.md) to build and boot in VirtualBox from Windows
