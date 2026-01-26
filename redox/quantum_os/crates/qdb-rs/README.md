# qdb-rs - Quantum Database

A hybrid quantum-classical database leveraging the QFT format for polynomial storage complexity and fault-tolerant error correction.

## Features

- **Quantum State Storage**: Store and retrieve quantum states using the `.qft` format
- **MPS Compression**: O(n χ²) storage complexity via Matrix Product State decomposition
- **Golay Error Correction**: [23,12,7] classical Golay codes for fault-tolerant persistence
- **Grover-Inspired Search**: O(√N) search complexity for unstructured queries
- **ACID Transactions**: Quantum-aware transaction semantics with entanglement groups
- **Multiple Backends**: Memory, disk, and extensible backend abstraction

## Installation

```toml
[dependencies]
qdb-rs = "0.1"
```

## Quick Start

```rust
use qdb_rs::prelude::*;

fn main() -> Result<()> {
    // Create an in-memory database
    let db = QDB::new(MemoryBackend::new());

    // Store classical data
    db.put("user:1", "Alice")?;
    db.put("user:2", "Bob")?;

    // Store quantum states
    let bell = bell_state()?;
    db.put_quantum("entangled_pair", bell)?;

    // Query
    let value = db.get("user:1")?;
    println!("User 1: {:?}", value);

    // Retrieve quantum state
    let state = db.get_quantum("entangled_pair")?;
    println!("Quantum state: {} qubits", state.unwrap().num_qubits());

    Ok(())
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     QDB API Layer                           │
├─────────────────────────────────────────────────────────────┤
│  Key-Value API  │  Document API  │  Quantum State API       │
├─────────────────────────────────────────────────────────────┤
│                   Query Engine                              │
│  - Grover search (O(√N))                                    │
│  - Tensor contractions for joins                            │
│  - Variational optimization                                 │
├─────────────────────────────────────────────────────────────┤
│                   Storage Layer                             │
│  - QFT serialization (MPS tensors)                          │
│  - Golay error correction                                   │
│  - Adaptive bond dimension                                  │
├─────────────────────────────────────────────────────────────┤
│                   Backend Abstraction                       │
│  Memory  │  Disk  │  Distributed  │  Quantum Hardware       │
└─────────────────────────────────────────────────────────────┘
```

## Storage Backends

### Memory Backend

Fast, in-memory storage using `DashMap` for concurrent access:

```rust
let db = QDB::new(MemoryBackend::new());
```

### Disk Backend

Persistent storage with checksums and Golay protection:

```rust
let db = QDB::new(DiskBackend::open("./data")?);
```

## Quantum Operations

### Storing Quantum States

```rust
use qft::QftBuilder;

// Create a custom quantum state
let state = QftBuilder::new(4)
    .bond_dimension(128)
    .golay(true)
    .metadata("experiment", "VQE")
    .build()?;

db.put_quantum("vqe_ground_state", state)?;
```

### Grover Search

```rust
use qdb_rs::query::GroverSearch;

// Search for entries matching a predicate
let search = GroverSearch::new(1000, |idx| idx % 7 == 0);
let candidates = search.search()?;
```

## Transactions

### Basic Transactions

```rust
let manager = TransactionManager::new(backend);
let tx = manager.begin();

tx.put("key1", "value1")?;
tx.put("key2", "value2")?;
tx.commit()?;
```

### Quantum Transactions with Entanglement

```rust
let tx = manager.begin();
let qtx = QuantumTransaction::from_transaction(tx);

// Mark keys as entangled - must be committed together
qtx.entangle(vec![Key::string("q1"), Key::string("q2")])?;

qtx.put("q1", state1)?;
qtx.put("q2", state2)?;
qtx.commit()?; // Fails if not all entangled keys are written
```

## Query API

```rust
use qdb_rs::query::{Query, Filter, OrderBy, SortDirection};

let results = Query::new()
    .key_prefix("user:".to_string())
    .entry_type(EntryType::Document)
    .order_by(OrderBy::CreatedAt(SortDirection::Descending))
    .limit(10)
    .execute(&backend)?;

for entry in results.iter() {
    println!("{}: {:?}", entry.key, entry.value);
}
```

## Indexing

```rust
use qdb_rs::index::{Index, IndexManager};

let manager = IndexManager::new();

// Hash index for exact lookups
manager.add_index(Index::hash("user_idx", "_key"))?;

// B-tree index for range queries
manager.add_index(Index::btree("version_idx", "_version"))?;

// Quantum index for Grover search
manager.add_index(Index::quantum("quantum_idx", "state_type", 10))?;
```

## Comparison with Classical Databases

| Aspect | Classical (RocksDB/PostgreSQL) | QDB |
|--------|-------------------------------|-----|
| Storage | O(N) | O(n χ²) via MPS |
| Search | O(log N) indexed, O(N) scan | O(√N) Grover |
| Error Correction | Checksums | Golay [23,12,7] |
| Quantum States | Not supported | Native `.qft` format |
| Entanglement | N/A | Transaction groups |

## License

MIT OR Apache-2.0

## References

1. Grover, L. K. (1996). "A fast quantum mechanical algorithm for database search."
2. Peruzzo, A., et al. (2014). "A variational eigenvalue solver on a photonic quantum processor."
3. QFT Specification v1.2.0 - MACROHARD Quantum OS
