# V-OS OS - Quick Start Guide

## Prerequisites

- **Linux/WSL2**: Required for building
- **Rust nightly**: `rustup default nightly`
- **QEMU**: For testing

## Build Commands

```bash
# Build V-OS Desktop
make all CONFIG_NAME=v-os-desktop

# Build V-OS Server
make all CONFIG_NAME=v-os-server

# Build V-OS Minimal
make all CONFIG_NAME=v-os-minimal

# Run in QEMU
make qemu
```

## Windows Development

```powershell
# Setup WSL2 environment
.\scripts\v-os-dev.ps1 setup

# Build
.\scripts\v-os-dev.ps1 all

# Run in QEMU
.\scripts\v-os-dev.ps1 qemu
```

## Linux/macOS Development

```bash
# Build
./scripts/v-os-build.sh all

# Run in QEMU
./scripts/v-os-build.sh qemu
```

## Ports

| Service | Port |
|---------|------|
| SSH (QEMU forward) | 2222 |
| HTTP (if enabled) | 8080 |

## EustressEngine Submodule

```bash
# Update submodule to latest
git submodule update --remote --merge

# Re-apply Redox workspace config
cd quantum_os/external
.\build-eustress.ps1 setup
```

## V-OS (QDB Examples)

```bash
cd quantum_os
cargo run --example basic_usage -p qdb-rs
cargo run --example quantum_transactions -p qdb-rs
cargo run --example grover_search -p qdb-rs
cargo run --example logosq_integration -p qdb-rs
```

## Documentation

- [V-OS.md](V-OS.md) - Project overview
- [ROADMAP.md](ROADMAP.md) - Development roadmap
- [ARCHITECTURE.md](ARCHITECTURE.md) - System architecture
- [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) - Detailed guide
- [docs/DRIVERS.md](docs/DRIVERS.md) - Driver development
- [docs/SECURITY.md](docs/SECURITY.md) - Security model
- [docs/HYPERVISOR.md](docs/HYPERVISOR.md) - Hypervisor design
- [quantum_os/crates/qdb-rs/Start.md](quantum_os/crates/qdb-rs/Start.md) - QDB quick start
