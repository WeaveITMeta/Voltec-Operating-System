# External Dependencies

## EustressEngine Submodule

The EustressEngine is integrated as a git submodule with sparse checkout to include only the engine-related crates.

### Included Crates

| Crate | Description |
|-------|-------------|
| `engine/` | Desktop 3D editor/studio (Bevy + egui) |
| `common/` | Shared scene format, types, utilities |
| `backend/` | Backend services |
| `bliss/` | Bliss utilities |
| `embedvec/` | Embedding vectors |
| `forge/` | Forge system |
| `forge-sdk/` | Forge SDK |
| `mcp/` | Model Context Protocol integration |
| `runtime/` | Runtime components |
| `server/` | Server components |
| `spatial-llm/` | Spatial LLM integration |

### Excluded (sparse checkout)

- `client/` - Player/renderer
- `player-android/` - Android player
- `player-ios/` - iOS player  
- `player-mobile/` - Mobile player
- `web/` - Web components

### Commands

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

### Using EustressEngine Crates

Reference crates via path in your `Cargo.toml`:

```toml
[dependencies]
eustress-common = { path = "../external/eustress-engine/eustress/crates/common" }
eustress-engine = { path = "../external/eustress-engine/eustress/crates/engine" }
```

### Sparse Checkout Configuration

The sparse checkout is configured in `.git/modules/redox/quantum_os/external/eustress-engine/info/sparse-checkout`:

```
/*
!eustress/crates/client/
!eustress/crates/player-android/
!eustress/crates/player-ios/
!eustress/crates/player-mobile/
!eustress/crates/web/
```

To modify, edit that file and run:
```bash
git -C redox/quantum_os/external/eustress-engine read-tree -mu HEAD
```
