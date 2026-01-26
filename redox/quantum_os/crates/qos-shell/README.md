# QOS-Shell - Quantum OS Desktop Shell

A Slint-based desktop environment for Quantum OS with QDB filesystem integration and Eustress 3D support.

## Architecture

```
┌─────────────────────────────────────────┐
│         Slint Desktop Shell             │
│    (File browser, settings, panels)     │
├─────────────────────────────────────────┤
│  Eustress 3D Viewport (IPC connection)  │
│    (Scene editing, visualization)       │
├─────────────────────────────────────────┤
│         QDB Storage Backend             │
└─────────────────────────────────────────┘
```

## Features

- **Slint UI** - Modern, declarative UI with Quantum OS theme
- **QDB Filesystem** - Browse quantum states and classical files
- **Quantum Preview** - View `.qft` file properties inline
- **Eustress Integration** - Launch 3D visualization via IPC

## Running

```bash
cargo run -p qos-shell
```

## UI Components

| Component | Description |
|-----------|-------------|
| `Sidebar` | Favorites, storage indicator |
| `FileBrowser` | QDB-backed file listing |
| `QuantumPreview` | `.qft` state preview panel |

## Theme

The shell uses a dark quantum-inspired theme:
- Background: `#0a0a12`
- Primary: `#6366f1` (indigo)
- Quantum: `#8b5cf6` (purple)
- Secondary: `#22d3ee` (cyan)

## Eustress IPC

The shell communicates with Eustress 3D via IPC:

```rust
pub enum EustressMessage {
    OpenFile(String),
    LoadQuantumState { path: String, num_qubits: usize },
    GetSceneInfo,
    Shutdown,
}
```

## License

MIT
