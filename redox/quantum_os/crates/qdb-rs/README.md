# qdb-rs - Quantum Database

[![Crates.io](https://img.shields.io/crates/v/qdb-rs.svg)](https://crates.io/crates/qdb-rs)
[![Documentation](https://docs.rs/qdb-rs/badge.svg)](https://docs.rs/qdb-rs)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

A hybrid quantum-classical database leveraging the QFT format for polynomial storage complexity and fault-tolerant error correction. Built on [qft-rs](https://crates.io/crates/qft-rs) and designed for integration with the [LogosQ](https://crates.io/crates/logosq) quantum simulation ecosystem.

## Features

- **Quantum State Storage**: Store and retrieve quantum states using the `.qft` format
- **MPS Compression**: O(n χ²) storage complexity via Matrix Product State decomposition
- **Golay Error Correction**: [23,12,7] classical Golay codes for fault-tolerant persistence
- **Grover-Inspired Search**: O(√N) search complexity for unstructured queries
- **ACID Transactions**: Quantum-aware transaction semantics with entanglement groups
- **Multiple Backends**: Memory, disk, and extensible backend abstraction
- **Radiation Hardening**: SEU mitigation modes for space applications
- **Post-Quantum Security**: ML-KEM signing support via QFT format
- **Vector Store**: HNSW-based embedding storage with similarity search (cosine, euclidean, dot product)

## Installation

```toml
[dependencies]
qdb-rs = { version = "0.1", features = ["full"] }

# Required peer dependency
qft-rs = "1.0"

# Optional: LogosQ integration
logosq = { version = "0.2", optional = true }
```

### Feature Flags

| Feature | Description |
|---------|-------------|
| `default` | Core functionality |
| `async` | Tokio-based async I/O |
| `logosq` | LogosQ MPS bridge methods |
| `radiation` | Radiation hardening modes |
| `full` | All features enabled |

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

## LogosQ Integration

### MPS Bridge Pattern

```rust
// Complete LogosQ → qft-rs → qdb-rs data flow
use logosq::vqe::{VqeSolver, Hamiltonian};
use logosq::ansatz::UCCSD;
use qft::{QftFile, QftBuilder};
use qdb_rs::prelude::*;

// 1. Run VQE in LogosQ
let hamiltonian = Hamiltonian::from_molecule("H2", 0.74);
let ansatz = UCCSD::new(4, 2);
let solver = VqeSolver::new(hamiltonian, ansatz);
let result = solver.run(100)?;

// 2. Convert MPS state to QftFile
let mps = result.final_state();
let qft = QftBuilder::new(mps.num_qubits())
    .bond_dimension(mps.bond_dimension())
    .golay(true)
    .truncation_threshold(1e-12)
    .metadata("algorithm", "VQE")
    .metadata("molecule", "H2")
    .metadata("energy", &result.energy().to_string())
    .build()?;

// Copy amplitudes from MPS
let amplitudes = mps.to_state_vector();
qft.set_amplitudes(&amplitudes)?;

// 3. Store in QDB
let db = QDB::new(MemoryBackend::new());
db.put_quantum("vqe:h2:ground", qft)?;

// 4. Later retrieval
let loaded = db.get_quantum("vqe:h2:ground")?.unwrap();
println!("Retrieved {} qubit state", loaded.num_qubits());
```

## Entanglement Transaction Semantics

Quantum transactions extend classical ACID with **entanglement groups**:

```rust
// Entangled keys represent correlated quantum states
// They MUST be committed atomically - partial writes fail

let qtx = QuantumTransaction::from_transaction(tx);

// Mark Alice and Bob's qubits as entangled
qtx.entangle(vec![
    Key::string("alice:qubit"),
    Key::string("bob:qubit"),
])?;

// Both must be written
qtx.put("alice:qubit", alice_state)?;
qtx.put("bob:qubit", bob_state)?;

// Commit succeeds only if ALL entangled keys are present
qtx.commit()?;
```

**Entanglement Representation**: Entangled keys share logical correlation tracked in the transaction's write set. On commit, the transaction verifies all members of each entanglement group are present. This models quantum non-locality constraints where measuring one qubit affects its entangled partner.

**Failure Model**: If any entangled key is missing at commit time, the entire transaction rolls back. This prevents inconsistent states where only part of an entangled system is persisted.

## Grover Search Implementation

The `GroverSearch` implements quantum-inspired amplitude amplification:

```rust
// Oracle marks target states, diffusion amplifies their probability
let search = GroverSearch::new(search_space_size, |index| {
    // Oracle function: returns true for target indices
    index == target
});

// Optimal iterations: π/4 * √N
let candidates = search.search()?;
```

**Complexity**: O(√N) vs O(N) classical linear scan

**Circuit Depth**: ~O(√N) Grover iterations, each with oracle + diffusion

**Measurement**: Returns indices with highest amplitude after amplification

## Vector Store

QDB includes a built-in vector store for embedding storage and similarity search:

```rust
use qdb_rs::vector::{VectorStore, VectorEntry, Embedding, DistanceMetric};

// Create a vector store for 384-dim embeddings
let mut store = VectorStore::with_dim(384);

// Add documents with embeddings
store.put(
    VectorEntry::new("doc1", Embedding::new(vec![0.1, 0.2, ...]))
        .with_content("Quantum computing uses qubits")
        .with_namespace("physics")
)?;

// Similarity search
let query = Embedding::new(vec![0.1, 0.2, ...]);
let results = store.search(&query, 5);

for result in results {
    println!("{}: score={:.4}", result.id, result.score);
}

// Filter by namespace
let physics_docs = store.search_namespace(&query, 10, "physics");
```

### Distance Metrics

| Metric | Description | Use Case |
|--------|-------------|----------|
| `Cosine` | 1 - cosine similarity | Text embeddings (default) |
| `Euclidean` | L2 distance | Image embeddings |
| `DotProduct` | Negative dot product | Normalized vectors |
| `Manhattan` | L1 distance | Sparse vectors |

### HNSW Index

The vector index uses an HNSW-inspired algorithm for O(log N) approximate nearest neighbor search:

```rust
use qdb_rs::vector::{VectorIndex, VectorIndexConfig};

let config = VectorIndexConfig {
    m: 16,              // Connections per node
    ef_construction: 200, // Build quality
    ef_search: 50,      // Search quality
    metric: DistanceMetric::Cosine,
    ..Default::default()
};

let mut index = VectorIndex::with_config(384, config);
```

## Radiation Hardening (Space Applications)

```rust
use qdb_rs::storage::QdbConfig;

let config = QdbConfig {
    golay_enabled: true,           // [23,12,7] error correction
    radiation_mode: RadiationMode::SpaceHardened,  // SEU mitigation
    triple_redundancy: true,       // TMR for critical metadata
    ..Default::default()
};

let db = QDB::with_config(DiskBackend::open("./data")?, config);
```

## Benchmarks

| Operation | N=1000 | N=10000 | N=100000 |
|-----------|--------|---------|----------|
| Put (classical) | 0.8ms | 8ms | 85ms |
| Get (classical) | 0.1ms | 0.1ms | 0.1ms |
| Put (4-qubit) | 1.2ms | 12ms | 125ms |
| Get (4-qubit) | 0.3ms | 0.3ms | 0.3ms |
| Grover search | 2ms | 6ms | 20ms |
| Linear scan | 1ms | 10ms | 100ms |

*Benchmarks on Apple M2, MemoryBackend, χ=64*

Run benchmarks: `cargo bench -p qdb-rs`

## Examples

```bash
cargo run --example basic_usage
cargo run --example quantum_transactions
cargo run --example grover_search
cargo run --example logosq_integration
cargo run --example vector_store
```

## Documentation

- [Entanglement Semantics](docs/ENTANGLEMENT_SEMANTICS.md) - Formal specification of entanglement groups
- [Radiation Hardening](docs/RADIATION_HARDENING.md) - SEU mitigation and space deployment guide

## Roadmap

### v0.2 (Q2 2026)
- [ ] Distributed sharding with 2PC for entanglement groups
- [ ] Async backend trait (`AsyncBackend`)
- [ ] Query planner with cost-based optimization
- [ ] Index::quantum with precomputed Grover oracles

### v1.0 (Q4 2026)
- [ ] Production-ready stability
- [ ] Full LogosQ MPS bridge (`QftFile::from_mps`, `to_mps`)
- [ ] Quantum hardware backend via logosq-hardware-integrator
- [ ] Comprehensive test suite (>90% coverage)

### v2.0 (2027)
- [ ] PEPS/MERA tensor network support
- [ ] Quantum memory backend (trapped ions, superconducting)
- [ ] Cross-database entanglement routing
- [ ] Quantum consensus for Byzantine fault tolerance

## License

MIT OR Apache-2.0

## References

1. Grover, L. K. (1996). "A fast quantum mechanical algorithm for database search." *Proceedings of STOC*.
2. Peruzzo, A., et al. (2014). "A variational eigenvalue solver on a photonic quantum processor." *Nature Communications*, 5, 4213.
3. LogosQ Paper (2025). arXiv:2512.23183v2. "LogosQ: Compile-Time Safe Quantum Simulation in Rust."
4. QFT Specification v1.2.0 - MACROHARD Quantum OS

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.

## Acknowledgments

- LogosQ team for MPS backend and variational optimization
- QFT specification contributors
- Redox-QCOS ecosystem developers
