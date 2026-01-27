# QDB-RS Quick Start

## Most Important Things to Know

1. **Hybrid Database**: Stores both classical data AND quantum states
2. **Vector Store**: HNSW-based embeddings with quantum fidelity distance
3. **MPS Compression**: O(n χ²) storage instead of O(2ⁿ) for quantum states
4. **Grover Search**: O(√N) search complexity for unstructured queries
5. **Fault Tolerant**: Golay error correction + radiation hardening

## Quick Example

```rust
use qdb_rs::prelude::*;

// Classical + Quantum storage
let db = QDB::new(MemoryBackend::new());
db.put("key", "value")?;
db.put_quantum("bell", bell_state()?)?;

// Vector store with quantum fidelity
let mut store = VectorStore::with_dim(384);
store.put(VectorEntry::new("doc1", embedding))?;
let results = store.search(&query, 10);
```

## Run Examples

```bash
cargo run --example basic_usage
cargo run --example quantum_transactions
cargo run --example grover_search
cargo run --example logosq_integration
cargo run --example vector_store
```

## Run Tests

```bash
cargo test -p qdb-rs
cargo test -p qdb-rs vector::  # Vector store tests only
```

## Run Benchmarks

```bash
cargo bench -p qdb-rs
```

## Build

```bash
cargo build -p qdb-rs
cargo build -p qdb-rs --release
```

## Key Types

| Type | Purpose |
|------|---------|
| `QDB<B>` | Main database with pluggable backend |
| `VectorStore` | Embedding storage with ANN search |
| `Embedding` | Vector with quantum methods (fidelity, amplitude encoding) |
| `DistanceMetric` | Cosine, Euclidean, DotProduct, Manhattan, Fidelity, Hamming |
| `Entry` | Database entry with metadata and checksums |

## Distance Metrics

| Metric | Use Case |
|--------|----------|
| `Cosine` | Text embeddings (default) |
| `Fidelity` | Quantum state similarity |
| `Euclidean` | Image embeddings |
| `Hamming` | Binary vectors |
