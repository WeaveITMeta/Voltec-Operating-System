# qft-rs - Native Rust SDK for Quantum File Type

Production-grade Rust library for working with quantum states in the .qft format.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
qft = "1.0"

# With async support
qft = { version = "1.0", features = ["async"] }
```

## Quick Start

```rust
use qft::prelude::*;

fn main() -> Result<()> {
    // Create a 4-qubit state
    let mut state = QftFile::new(4)?;
    
    // Set to GHZ-like state |0000⟩ + |1111⟩
    state[0] = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0);
    state[15] = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0);
    
    // Save to disk
    state.save("state.qft")?;
    
    // Load and verify
    let loaded = QftFile::load("state.qft")?;
    assert!((state.fidelity(&loaded)? - 1.0).abs() < 1e-10);
    
    Ok(())
}
```

## Features

### Builder API

```rust
use qft::QftBuilder;

let state = QftBuilder::new(4)
    .bond_dimension(128)
    .golay(true)
    .metadata("author", "Alice")
    .metadata("experiment", "VQE ground state")
    .build()?;
```

### Common States

```rust
use qft::{bell_state, ghz_state, uniform_state, basis_state};

let bell = bell_state()?;                    // |00⟩ + |11⟩
let ghz = ghz_state(4)?;                     // |0000⟩ + |1111⟩
let uniform = uniform_state(3)?;             // Equal superposition
let basis = basis_state(4, 5)?;              // |0101⟩
```

### State Operations

```rust
let mut state = QftFile::new(4)?;

// Normalization
state.normalize()?;
assert!(state.is_normalized(1e-10));

// Fidelity
let other = QftFile::new(4)?;
let fid = state.fidelity(&other)?;

// Inner product
let overlap = state.inner_product(&other)?;

// Trace distance
let dist = state.trace_distance(&other)?;
```

### I/O Operations

```rust
// Binary format
state.save("state.qft")?;
let loaded = QftFile::load("state.qft")?;

// Bytes
let bytes = state.to_bytes()?;
let from_bytes = QftFile::from_bytes(&bytes)?;

// JSON
let json = state.to_json()?;
let from_json = QftFile::from_json(&json)?;
```

### Async I/O (feature-gated)

```rust
use qft::QftFile;

#[tokio::main]
async fn main() -> qft::Result<()> {
    let state = QftFile::new(4)?;
    state.save_async("state.qft").await?;
    
    let loaded = QftFile::load_async("state.qft").await?;
    Ok(())
}
```

### Configuration

```rust
use qft::{QftFile, QftConfig};

let config = QftConfig {
    bond_dimension: 128,
    golay_enabled: true,
    truncation_threshold: 1e-12,
};

let state = QftFile::with_config(4, config)?;
```

### Metadata

```rust
let mut state = QftFile::new(4)?;

state.set_metadata("author", "Alice");
state.set_metadata("date", "2026-01-25");

if let Some(author) = state.get_metadata("author") {
    println!("Author: {}", author);
}

for (key, value) in state.metadata() {
    println!("{}: {}", key, value);
}
```

## API Reference

### QftFile

| Method | Description |
|--------|-------------|
| `new(n)` | Create n-qubit state initialized to \|0...0⟩ |
| `from_amplitudes(vec)` | Create from complex amplitude vector |
| `from_real_imag(r, i)` | Create from separate real/imag arrays |
| `num_qubits()` | Get qubit count |
| `dimension()` | Get state dimension (2^n) |
| `amplitudes()` | Get amplitude slice |
| `get_amplitude(i)` | Get single amplitude |
| `set_amplitude(i, v)` | Set single amplitude |
| `norm()` | Calculate norm |
| `norm_squared()` | Calculate norm² |
| `is_normalized(tol)` | Check normalization |
| `normalize()` | Normalize in place |
| `inner_product(other)` | Calculate ⟨self\|other⟩ |
| `fidelity(other)` | Calculate \|⟨self\|other⟩\|² |
| `trace_distance(other)` | Calculate trace distance |
| `load(path)` | Load from file |
| `save(path)` | Save to file |
| `to_bytes()` | Serialize to bytes |
| `from_bytes(data)` | Deserialize from bytes |
| `to_json()` | Export to JSON |
| `from_json(json)` | Import from JSON |

### QftBuilder

| Method | Description |
|--------|-------------|
| `new(n)` | Create builder for n qubits |
| `bond_dimension(d)` | Set MPS bond dimension |
| `golay(bool)` | Enable/disable error correction |
| `truncation_threshold(t)` | Set SVD truncation threshold |
| `metadata(k, v)` | Add metadata entry |
| `amplitudes(vec)` | Set initial amplitudes |
| `build()` | Build the QftFile |

### Error Types

| Error | Description |
|-------|-------------|
| `InvalidQubits` | Qubit count not in 1-30 |
| `IndexOutOfRange` | Basis state index too large |
| `DimensionMismatch` | Array sizes don't match |
| `ZeroNorm` | Cannot normalize zero state |
| `InvalidFormat` | Not a valid .qft file |
| `Io` | File I/O error |
| `Serialization` | JSON serialization error |
| `ChecksumMismatch` | CRC verification failed |
| `GolayDecodeFailed` | Too many bit errors |

## Performance

| Operation | 10 qubits | 20 qubits | 25 qubits |
|-----------|-----------|-----------|-----------|
| Create | 0.01 ms | 0.5 ms | 16 ms |
| Save | 0.1 ms | 10 ms | 320 ms |
| Load | 0.05 ms | 5 ms | 160 ms |
| Fidelity | 0.02 ms | 2 ms | 64 ms |
| Normalize | 0.01 ms | 1 ms | 32 ms |

## Feature Flags

| Feature | Description |
|---------|-------------|
| `async` | Enable tokio-based async I/O |
| `full` | Enable all features |

## License

MIT OR Apache-2.0
