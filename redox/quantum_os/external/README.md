# External Dependencies

## EustressEngine Submodule

The EustressEngine is integrated as a git submodule with sparse checkout to include only the engine-related crates.

### Build Status

| Crate | Status | Notes |
|-------|--------|-------|
| `common/` | ✅ Builds | Shared scene format, types, utilities |
| `bevy-camera-controller/` | ✅ Builds | Camera utilities |
| `runtime/` | ✅ Builds | Runtime components |
| `backend/` | ✅ Builds | Backend services |
| `forge/` | ✅ Builds | Forge system |
| `forge-sdk/` | ✅ Builds | Forge SDK |
| `mcp/` | ✅ Builds | Model Context Protocol integration |
| `bliss/` | ✅ Builds | Bliss utilities |
| `embedvec/` | ✅ Builds | Embedding vectors |
| `spatial-llm/` | ✅ Builds | Spatial LLM integration |
| `engine/` | ⚠️ Disabled | Depends on eustress-networking |
| `server/` | ⚠️ Disabled | Depends on eustress-networking |
| `eustress-networking/` | ❌ Upstream issue | Bevy 0.17 API incompatibility |

### Excluded (sparse checkout)

- `client/` - Player/renderer
- `player-android/` - Android player
- `player-ios/` - iOS player  
- `player-mobile/` - Mobile player
- `web/` - Web components

---

## Quick Start

### First Time Setup

```powershell
# From redox/quantum_os/external/
.\build-eustress.ps1 setup
```

This will:
1. Backup the original `Cargo.toml`
2. Copy the Redox-compatible workspace configuration

### Building

```powershell
# Check compilation
.\build-eustress.ps1 check

# Build specific crate
cd eustress-engine/eustress
cargo build -p eustress-common
```

### Restore Original

```powershell
.\build-eustress.ps1 restore
```

---

## Using EustressEngine Crates

Reference crates via path in your `Cargo.toml`:

```toml
[dependencies]
eustress-common = { path = "../external/eustress-engine/eustress/crates/common" }
bevy-camera-controller = { path = "../external/eustress-engine/eustress/crates/common/bevy-camera-controller" }
```

### Available Features

The `eustress-common` crate provides:
- Scene format and serialization
- Realism systems (fluids, particles, deformation)
- Point cloud processing
- Physics integration (Avian 0.4)

---

## Submodule Commands

```bash
# Clone with submodules (for new clones)
git clone --recurse-submodules https://github.com/WeaveITMeta/Quantum-Operating-System.git

# Initialize submodule (if cloned without --recurse-submodules)
git submodule update --init --recursive

# Update to latest EustressEngine
git submodule update --remote --merge

# Check submodule status
git submodule status
```

---

## Technical Details

### Redox Workspace Configuration

The file `Cargo.redox.toml` contains a modified workspace that excludes:
- Crates not available due to sparse checkout
- Crates with upstream Bevy 0.17 API incompatibilities

### Sparse Checkout Configuration

Located in `.git/modules/redox/quantum_os/external/eustress-engine/info/sparse-checkout`:

```
/*
!eustress/crates/client/
!eustress/crates/player-android/
!eustress/crates/player-ios/
!eustress/crates/player-mobile/
!eustress/crates/web/
```

### Known Issues

1. **eustress-networking**: Has Bevy 0.17 API incompatibilities (upstream issue)
   - Affects: `engine/`, `server/` crates
   - Workaround: Use `eustress-common` directly without networking

2. **Sparse checkout warnings**: Git may show warnings about cone mode
   - These are informational and don't affect functionality
