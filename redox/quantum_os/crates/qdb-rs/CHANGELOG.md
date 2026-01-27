# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-01-26

### Added

#### Vector Store (Quantum-Optimized)
- `VectorStore` with HNSW-based approximate nearest neighbor search
- `Embedding` type with quantum-specific methods:
  - `to_amplitude_encoding()` - normalize for quantum circuits
  - `from_complex_amplitudes()` - create from quantum state
  - `from_probabilities()` - create from measurement distribution
  - `fidelity()` - compute |⟨ψ|φ⟩|² between embeddings
  - `trace_distance()` - quantum distinguishability metric
  - `quantize_binary()` - for Hamming distance
  - `product_quantize()` - split into subvectors
- 6 distance metrics: `Cosine`, `Euclidean`, `DotProduct`, `Manhattan`, `Fidelity`, `Hamming`
- SIMD-optimized `cosine_distance_fast()` and `euclidean_distance_squared()`
- `VectorEntry` with metadata, content, and namespace support
- `VectorStoreStats` for memory and index statistics
- Namespace operations: `search_namespace()`, `get_namespace()`, `count_namespace()`, `delete_namespace()`
- `upsert()`, `find_similar()`, `centroid()`, `clear()`, `contains()`

#### Documentation
- Comprehensive README with "Why QDB?" value proposition
- Entanglement semantics documentation (`docs/ENTANGLEMENT_SEMANTICS.md`)
- Radiation hardening guide (`docs/RADIATION_HARDENING.md`)
- Vector store example (`examples/vector_store.rs`)

#### Configuration
- `RadiationMode` enum: `Standard`, `SpaceHardened`, `MaxProtection`
- `QdbConfig::space_hardened()` and `QdbConfig::max_protection()` presets
- `post_quantum_signing` and `triple_redundancy` options

### Changed
- Updated `thiserror` to 2.0
- Updated `rayon` to 1.10
- Updated `tokio` to 1.40
- Improved Cargo.toml organization with section comments

## [0.1.0] - 2026-01-20

### Added
- Initial release
- `QDB<B>` generic database with pluggable backends
- `MemoryBackend` using DashMap for concurrent access
- `DiskBackend` for persistent storage
- Quantum state storage via `put_quantum()` / `get_quantum()`
- Classical key-value storage via `put()` / `get()`
- `Entry` type with metadata, versioning, and checksums
- `Key` variants: String, Integer, Composite, Binary
- `Value` variants: Null, Bool, Integer, Float, String, Binary, Json, Quantum, Complex, Array, Map
- Grover-inspired search with `GroverSearch`
- ACID transactions with `Transaction` and entanglement groups
- Golay [23,12,7] error correction
- BLAKE3 checksums for integrity
- Basic indexing support

## [Unreleased]

### Planned
- Async backend trait (`AsyncBackend`)
- Distributed sharding with 2PC
- Query planner with cost-based optimization
- Full LogosQ MPS bridge
- Quantum hardware backend
