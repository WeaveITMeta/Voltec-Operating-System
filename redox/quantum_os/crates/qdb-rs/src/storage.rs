//! Storage backends for QDB

use crate::entry::{Entry, Key, Value};
use crate::error::{QdbError, Result};
use dashmap::DashMap;
use parking_lot::RwLock;
use qft::{QftBuilder, QftFile};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

// =============================================================================
// Backend Trait
// =============================================================================

/// Storage backend trait for QDB
pub trait Backend: Send + Sync {
    /// Get an entry by key
    fn get(&self, key: &Key) -> Result<Option<Entry>>;

    /// Put an entry
    fn put(&self, entry: Entry) -> Result<()>;

    /// Delete an entry by key
    fn delete(&self, key: &Key) -> Result<bool>;

    /// Check if a key exists
    fn exists(&self, key: &Key) -> Result<bool>;

    /// Get all keys (for iteration)
    fn keys(&self) -> Result<Vec<Key>>;

    /// Get the number of entries
    fn len(&self) -> Result<usize>;

    /// Check if empty
    fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Flush any pending writes
    fn flush(&self) -> Result<()>;

    /// Close the backend
    fn close(&self) -> Result<()>;
}

// =============================================================================
// Memory Backend
// =============================================================================

/// In-memory storage backend using DashMap for concurrent access
pub struct MemoryBackend {
    data: DashMap<Vec<u8>, Entry>,
    closed: AtomicBool,
}

impl MemoryBackend {
    /// Create a new memory backend
    pub fn new() -> Self {
        Self {
            data: DashMap::new(),
            closed: AtomicBool::new(false),
        }
    }

    fn check_closed(&self) -> Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(QdbError::Closed);
        }
        Ok(())
    }
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for MemoryBackend {
    fn get(&self, key: &Key) -> Result<Option<Entry>> {
        self.check_closed()?;
        let key_bytes = key.to_bytes();
        Ok(self.data.get(&key_bytes).map(|e| e.value().clone()))
    }

    fn put(&self, entry: Entry) -> Result<()> {
        self.check_closed()?;
        let key_bytes = entry.key.to_bytes();
        self.data.insert(key_bytes, entry);
        Ok(())
    }

    fn delete(&self, key: &Key) -> Result<bool> {
        self.check_closed()?;
        let key_bytes = key.to_bytes();
        Ok(self.data.remove(&key_bytes).is_some())
    }

    fn exists(&self, key: &Key) -> Result<bool> {
        self.check_closed()?;
        let key_bytes = key.to_bytes();
        Ok(self.data.contains_key(&key_bytes))
    }

    fn keys(&self) -> Result<Vec<Key>> {
        self.check_closed()?;
        Ok(self.data.iter().map(|e| e.value().key.clone()).collect())
    }

    fn len(&self) -> Result<usize> {
        self.check_closed()?;
        Ok(self.data.len())
    }

    fn flush(&self) -> Result<()> {
        self.check_closed()?;
        Ok(()) // No-op for memory backend
    }

    fn close(&self) -> Result<()> {
        self.closed.store(true, Ordering::Release);
        self.data.clear();
        Ok(())
    }
}

// =============================================================================
// Disk Backend
// =============================================================================

/// Disk-based storage backend using QFT files
pub struct DiskBackend {
    path: PathBuf,
    index: RwLock<HashMap<Vec<u8>, PathBuf>>,
    closed: AtomicBool,
    entry_count: AtomicU64,
}

impl DiskBackend {
    /// Create or open a disk backend at the given path
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Create directory if it doesn't exist
        fs::create_dir_all(&path)?;

        let mut index = HashMap::new();
        let mut count = 0u64;

        // Scan existing files to build index
        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                if file_path.extension().map_or(false, |e| e == "qdb") {
                    if let Ok(key_bytes) = Self::read_key_from_file(&file_path) {
                        index.insert(key_bytes, file_path);
                        count += 1;
                    }
                }
            }
        }

        Ok(Self {
            path,
            index: RwLock::new(index),
            closed: AtomicBool::new(false),
            entry_count: AtomicU64::new(count),
        })
    }

    fn check_closed(&self) -> Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(QdbError::Closed);
        }
        Ok(())
    }

    fn key_to_filename(key: &Key) -> String {
        let hash = key.hash();
        format!("{}.qdb", hex::encode(&hash[..16]))
    }

    fn read_key_from_file(path: &Path) -> Result<Vec<u8>> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // Read header
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != b"QDB\x01" {
            return Err(QdbError::BackendError("Invalid QDB file".to_string()));
        }

        // Read key length
        let mut key_len_bytes = [0u8; 4];
        reader.read_exact(&mut key_len_bytes)?;
        let key_len = u32::from_le_bytes(key_len_bytes) as usize;

        // Read key
        let mut key_bytes = vec![0u8; key_len];
        reader.read_exact(&mut key_bytes)?;

        Ok(key_bytes)
    }

    fn write_entry_to_file(path: &Path, entry: &Entry) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Magic number
        writer.write_all(b"QDB\x01")?;

        // Key
        let key_bytes = entry.key.to_bytes();
        writer.write_all(&(key_bytes.len() as u32).to_le_bytes())?;
        writer.write_all(&key_bytes)?;

        // Entry metadata
        let metadata_json = serde_json::to_vec(&EntryMetadata {
            entry_type: entry.entry_type,
            created_at: entry.created_at,
            modified_at: entry.modified_at,
            version: entry.version,
            metadata: entry.metadata.clone(),
        })?;
        writer.write_all(&(metadata_json.len() as u32).to_le_bytes())?;
        writer.write_all(&metadata_json)?;

        // Value
        let value_bytes = entry.value.to_bytes()?;
        writer.write_all(&(value_bytes.len() as u64).to_le_bytes())?;
        writer.write_all(&value_bytes)?;

        // Checksum
        let mut hasher = blake3::Hasher::new();
        hasher.update(&key_bytes);
        hasher.update(&metadata_json);
        hasher.update(&value_bytes);
        let checksum = hasher.finalize();
        writer.write_all(checksum.as_bytes())?;

        writer.flush()?;
        Ok(())
    }

    fn read_entry_from_file(path: &Path) -> Result<Entry> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // Magic number
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != b"QDB\x01" {
            return Err(QdbError::BackendError("Invalid QDB file".to_string()));
        }

        // Key
        let mut key_len_bytes = [0u8; 4];
        reader.read_exact(&mut key_len_bytes)?;
        let key_len = u32::from_le_bytes(key_len_bytes) as usize;
        let mut key_bytes = vec![0u8; key_len];
        reader.read_exact(&mut key_bytes)?;

        // Entry metadata
        let mut meta_len_bytes = [0u8; 4];
        reader.read_exact(&mut meta_len_bytes)?;
        let meta_len = u32::from_le_bytes(meta_len_bytes) as usize;
        let mut meta_bytes = vec![0u8; meta_len];
        reader.read_exact(&mut meta_bytes)?;

        // Value
        let mut value_len_bytes = [0u8; 8];
        reader.read_exact(&mut value_len_bytes)?;
        let value_len = u64::from_le_bytes(value_len_bytes) as usize;
        let mut value_bytes = vec![0u8; value_len];
        reader.read_exact(&mut value_bytes)?;

        // Checksum
        let mut stored_checksum = [0u8; 32];
        reader.read_exact(&mut stored_checksum)?;

        // Verify checksum
        let mut hasher = blake3::Hasher::new();
        hasher.update(&key_bytes);
        hasher.update(&meta_bytes);
        hasher.update(&value_bytes);
        let computed_checksum = *hasher.finalize().as_bytes();

        if stored_checksum != computed_checksum {
            return Err(QdbError::ChecksumMismatch("File corrupted".to_string()));
        }

        // Deserialize
        let key = deserialize_key(&key_bytes)?;
        let metadata: EntryMetadata = serde_json::from_slice(&meta_bytes)?;
        let value = deserialize_value(&value_bytes, metadata.entry_type)?;

        Ok(Entry {
            key,
            value,
            entry_type: metadata.entry_type,
            created_at: metadata.created_at,
            modified_at: metadata.modified_at,
            version: metadata.version,
            metadata: metadata.metadata,
            checksum: Some(stored_checksum),
        })
    }
}

impl Backend for DiskBackend {
    fn get(&self, key: &Key) -> Result<Option<Entry>> {
        self.check_closed()?;
        let key_bytes = key.to_bytes();
        let index = self.index.read();

        if let Some(path) = index.get(&key_bytes) {
            Ok(Some(Self::read_entry_from_file(path)?))
        } else {
            Ok(None)
        }
    }

    fn put(&self, entry: Entry) -> Result<()> {
        self.check_closed()?;
        let key_bytes = entry.key.to_bytes();
        let filename = Self::key_to_filename(&entry.key);
        let file_path = self.path.join(&filename);

        let is_new = {
            let index = self.index.read();
            !index.contains_key(&key_bytes)
        };

        Self::write_entry_to_file(&file_path, &entry)?;

        {
            let mut index = self.index.write();
            index.insert(key_bytes, file_path);
        }

        if is_new {
            self.entry_count.fetch_add(1, Ordering::Relaxed);
        }

        Ok(())
    }

    fn delete(&self, key: &Key) -> Result<bool> {
        self.check_closed()?;
        let key_bytes = key.to_bytes();

        let path = {
            let mut index = self.index.write();
            index.remove(&key_bytes)
        };

        if let Some(path) = path {
            fs::remove_file(path)?;
            self.entry_count.fetch_sub(1, Ordering::Relaxed);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn exists(&self, key: &Key) -> Result<bool> {
        self.check_closed()?;
        let key_bytes = key.to_bytes();
        let index = self.index.read();
        Ok(index.contains_key(&key_bytes))
    }

    fn keys(&self) -> Result<Vec<Key>> {
        self.check_closed()?;
        let index = self.index.read();
        let mut keys = Vec::with_capacity(index.len());

        for path in index.values() {
            if let Ok(entry) = Self::read_entry_from_file(path) {
                keys.push(entry.key);
            }
        }

        Ok(keys)
    }

    fn len(&self) -> Result<usize> {
        self.check_closed()?;
        Ok(self.entry_count.load(Ordering::Relaxed) as usize)
    }

    fn flush(&self) -> Result<()> {
        self.check_closed()?;
        Ok(()) // Files are flushed on write
    }

    fn close(&self) -> Result<()> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }
}

// =============================================================================
// QDB - Main Database Interface
// =============================================================================

/// Quantum Database - main interface
pub struct QDB<B: Backend = MemoryBackend> {
    backend: Arc<B>,
    config: QdbConfig,
    stats: QdbStats,
}

/// Radiation hardening mode for space applications
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RadiationMode {
    /// Standard mode - no radiation hardening
    #[default]
    Standard,
    /// Space hardened - SEU mitigation enabled
    SpaceHardened,
    /// Maximum protection - TMR + ECC + scrubbing
    MaxProtection,
}

/// QDB configuration
#[derive(Debug, Clone)]
pub struct QdbConfig {
    /// Default bond dimension for quantum states
    pub default_bond_dimension: usize,
    /// Enable Golay error correction
    pub golay_enabled: bool,
    /// Truncation threshold for MPS
    pub truncation_threshold: f64,
    /// Maximum entry size in bytes
    pub max_entry_size: usize,
    /// Read-only mode
    pub read_only: bool,
    /// Radiation hardening mode
    pub radiation_mode: RadiationMode,
    /// Enable triple modular redundancy for critical metadata
    pub triple_redundancy: bool,
    /// Enable ML-KEM post-quantum signatures
    pub post_quantum_signing: bool,
}

impl Default for QdbConfig {
    fn default() -> Self {
        Self {
            default_bond_dimension: 64,
            golay_enabled: true,
            truncation_threshold: 1e-10,
            max_entry_size: 1024 * 1024 * 100, // 100 MB
            read_only: false,
            radiation_mode: RadiationMode::Standard,
            triple_redundancy: false,
            post_quantum_signing: false,
        }
    }
}

impl QdbConfig {
    /// Create a space-hardened configuration
    pub fn space_hardened() -> Self {
        Self {
            golay_enabled: true,
            radiation_mode: RadiationMode::SpaceHardened,
            triple_redundancy: true,
            ..Default::default()
        }
    }

    /// Create a maximum protection configuration
    pub fn max_protection() -> Self {
        Self {
            golay_enabled: true,
            radiation_mode: RadiationMode::MaxProtection,
            triple_redundancy: true,
            post_quantum_signing: true,
            ..Default::default()
        }
    }
}

/// Database statistics
#[derive(Debug, Default)]
pub struct QdbStats {
    pub reads: AtomicU64,
    pub writes: AtomicU64,
    pub deletes: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
}

impl<B: Backend> QDB<B> {
    /// Create a new QDB with the given backend
    pub fn new(backend: B) -> Self {
        Self {
            backend: Arc::new(backend),
            config: QdbConfig::default(),
            stats: QdbStats::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(backend: B, config: QdbConfig) -> Self {
        Self {
            backend: Arc::new(backend),
            config,
            stats: QdbStats::default(),
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &QdbConfig {
        &self.config
    }

    /// Get statistics
    pub fn stats(&self) -> &QdbStats {
        &self.stats
    }

    /// Get a reference to the backend for direct queries
    pub fn backend(&self) -> &B {
        &self.backend
    }

    // =========================================================================
    // Key-Value Operations
    // =========================================================================

    /// Get a value by key
    pub fn get(&self, key: impl Into<Key>) -> Result<Option<Value>> {
        self.stats.reads.fetch_add(1, Ordering::Relaxed);
        let entry = self.backend.get(&key.into())?;
        Ok(entry.map(|e| e.value))
    }

    /// Get a full entry by key
    pub fn get_entry(&self, key: impl Into<Key>) -> Result<Option<Entry>> {
        self.stats.reads.fetch_add(1, Ordering::Relaxed);
        self.backend.get(&key.into())
    }

    /// Put a key-value pair
    pub fn put(&self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        if self.config.read_only {
            return Err(QdbError::ReadOnly);
        }

        self.stats.writes.fetch_add(1, Ordering::Relaxed);
        let entry = Entry::new(key.into(), value.into());

        if entry.size_bytes() > self.config.max_entry_size {
            return Err(QdbError::CapacityExceeded(format!(
                "Entry size {} exceeds maximum {}",
                entry.size_bytes(),
                self.config.max_entry_size
            )));
        }

        self.backend.put(entry)
    }

    /// Put a full entry
    pub fn put_entry(&self, entry: Entry) -> Result<()> {
        if self.config.read_only {
            return Err(QdbError::ReadOnly);
        }

        self.stats.writes.fetch_add(1, Ordering::Relaxed);
        self.backend.put(entry)
    }

    /// Delete a key
    pub fn delete(&self, key: impl Into<Key>) -> Result<bool> {
        if self.config.read_only {
            return Err(QdbError::ReadOnly);
        }

        self.stats.deletes.fetch_add(1, Ordering::Relaxed);
        self.backend.delete(&key.into())
    }

    /// Check if a key exists
    pub fn exists(&self, key: impl Into<Key>) -> Result<bool> {
        self.backend.exists(&key.into())
    }

    /// Get all keys
    pub fn keys(&self) -> Result<Vec<Key>> {
        self.backend.keys()
    }

    /// Get the number of entries
    pub fn len(&self) -> Result<usize> {
        self.backend.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> Result<bool> {
        self.backend.is_empty()
    }

    // =========================================================================
    // Quantum State Operations
    // =========================================================================

    /// Store a quantum state
    pub fn put_quantum(&self, key: impl Into<Key>, qft: QftFile) -> Result<()> {
        if self.config.read_only {
            return Err(QdbError::ReadOnly);
        }

        self.stats.writes.fetch_add(1, Ordering::Relaxed);
        let entry = Entry::quantum(key.into(), qft);
        self.backend.put(entry)
    }

    /// Get a quantum state
    pub fn get_quantum(&self, key: impl Into<Key>) -> Result<Option<QftFile>> {
        self.stats.reads.fetch_add(1, Ordering::Relaxed);
        let entry = self.backend.get(&key.into())?;

        match entry {
            Some(e) => match e.value {
                Value::Quantum(qft) => Ok(Some(*qft)),
                _ => Err(QdbError::ValueError(
                    "Entry is not a quantum state".to_string(),
                )),
            },
            None => Ok(None),
        }
    }

    /// Create and store a new quantum state
    pub fn create_quantum_state(
        &self,
        key: impl Into<Key>,
        num_qubits: usize,
    ) -> Result<()> {
        let qft = QftBuilder::new(num_qubits)
            .bond_dimension(self.config.default_bond_dimension)
            .golay(self.config.golay_enabled)
            .truncation_threshold(self.config.truncation_threshold)
            .build()?;

        self.put_quantum(key, qft)
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    /// Put multiple entries
    pub fn put_batch(&self, entries: Vec<Entry>) -> Result<()> {
        if self.config.read_only {
            return Err(QdbError::ReadOnly);
        }

        for entry in entries {
            self.backend.put(entry)?;
            self.stats.writes.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Get multiple values by keys
    pub fn get_batch(&self, keys: Vec<Key>) -> Result<Vec<Option<Value>>> {
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            self.stats.reads.fetch_add(1, Ordering::Relaxed);
            let entry = self.backend.get(&key)?;
            results.push(entry.map(|e| e.value));
        }
        Ok(results)
    }

    // =========================================================================
    // Lifecycle
    // =========================================================================

    /// Flush pending writes
    pub fn flush(&self) -> Result<()> {
        self.backend.flush()
    }

    /// Close the database
    pub fn close(&self) -> Result<()> {
        self.backend.close()
    }
}

// =============================================================================
// Helper Types and Functions
// =============================================================================

use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
struct EntryMetadata {
    entry_type: crate::entry::EntryType,
    created_at: u64,
    modified_at: u64,
    version: u64,
    metadata: HashMap<String, String>,
}

fn deserialize_key(bytes: &[u8]) -> Result<Key> {
    if bytes.is_empty() {
        return Err(QdbError::InvalidKey("Empty key".to_string()));
    }

    match bytes[0] {
        0 => Ok(Key::String(
            String::from_utf8(bytes[1..].to_vec())
                .map_err(|e| QdbError::InvalidKey(e.to_string()))?,
        )),
        1 => {
            if bytes.len() < 9 {
                return Err(QdbError::InvalidKey("Invalid integer key".to_string()));
            }
            let i = i64::from_le_bytes(bytes[1..9].try_into().unwrap());
            Ok(Key::Integer(i))
        }
        2 => {
            let mut parts = Vec::new();
            let mut offset = 1;
            while offset < bytes.len() {
                if offset + 4 > bytes.len() {
                    break;
                }
                let part_len =
                    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
                offset += 4;
                if offset + part_len > bytes.len() {
                    break;
                }
                let part = deserialize_key(&bytes[offset..offset + part_len])?;
                parts.push(part);
                offset += part_len;
            }
            Ok(Key::Composite(parts))
        }
        3 => Ok(Key::Binary(bytes[1..].to_vec())),
        _ => Err(QdbError::InvalidKey(format!(
            "Unknown key type: {}",
            bytes[0]
        ))),
    }
}

fn deserialize_value(bytes: &[u8], entry_type: crate::entry::EntryType) -> Result<Value> {
    match entry_type {
        crate::entry::EntryType::QuantumState => {
            let qft = QftFile::from_bytes(bytes)?;
            Ok(Value::Quantum(Box::new(qft)))
        }
        _ => {
            let value: Value = serde_json::from_slice(bytes)?;
            Ok(value)
        }
    }
}

mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_backend() {
        let backend = MemoryBackend::new();
        let db = QDB::new(backend);

        db.put("key1", "value1").unwrap();
        assert!(db.exists("key1").unwrap());

        let value = db.get("key1").unwrap();
        assert!(matches!(value, Some(Value::String(s)) if s == "value1"));

        db.delete("key1").unwrap();
        assert!(!db.exists("key1").unwrap());
    }

    #[test]
    fn test_quantum_storage() {
        let backend = MemoryBackend::new();
        let db = QDB::new(backend);

        let qft = QftFile::new(4).unwrap();
        db.put_quantum("quantum_state", qft).unwrap();

        let loaded = db.get_quantum("quantum_state").unwrap().unwrap();
        assert_eq!(loaded.num_qubits(), 4);
    }

    #[test]
    fn test_batch_operations() {
        let backend = MemoryBackend::new();
        let db = QDB::new(backend);

        let entries = vec![
            Entry::new(Key::string("k1"), Value::Integer(1)),
            Entry::new(Key::string("k2"), Value::Integer(2)),
            Entry::new(Key::string("k3"), Value::Integer(3)),
        ];

        db.put_batch(entries).unwrap();
        assert_eq!(db.len().unwrap(), 3);

        let keys = vec![Key::string("k1"), Key::string("k2"), Key::string("k3")];
        let values = db.get_batch(keys).unwrap();
        assert_eq!(values.len(), 3);
    }
}
