//! # qdb-rs - Quantum Database
//!
//! A hybrid quantum-classical database leveraging the QFT format for polynomial
//! storage complexity and fault-tolerant error correction.
//!
//! ## Table of Contents
//! 1. Core Types - QDB, Entry, Key, Value
//! 2. Storage Backends - Memory, Disk, Hybrid
//! 3. Query Engine - Grover-inspired search, tensor contractions
//! 4. Error Correction - Golay-protected persistence
//! 5. Transactions - Quantum-aware ACID semantics
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     QDB API Layer                           │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Key-Value API  │  Document API  │  Relational API          │
//! ├─────────────────────────────────────────────────────────────┤
//! │                   Query Engine                              │
//! │  - Grover search (O(√N))                                    │
//! │  - Tensor contractions                                      │
//! │  - Variational optimization                                 │
//! ├─────────────────────────────────────────────────────────────┤
//! │                   Storage Layer                             │
//! │  - QFT serialization (MPS tensors)                          │
//! │  - Golay error correction                                   │
//! │  - Adaptive bond dimension                                  │
//! ├─────────────────────────────────────────────────────────────┤
//! │                   Backend Abstraction                       │
//! │  Memory  │  Disk  │  Distributed  │  Quantum Hardware       │
//! └─────────────────────────────────────────────────────────────┘
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod entry;
pub mod error;
pub mod index;
pub mod query;
pub mod storage;
pub mod transaction;

pub use entry::{Entry, EntryType, Key, Value};
pub use error::{QdbError, Result};
pub use storage::{Backend, DiskBackend, MemoryBackend, QDB};

/// Prelude for convenient imports
pub mod prelude {
    pub use super::entry::{Entry, EntryType, Key, Value};
    pub use super::error::{QdbError, Result};
    pub use super::query::{Query, QueryResult};
    pub use super::storage::{Backend, QDB};
    pub use qft::prelude::*;
}
