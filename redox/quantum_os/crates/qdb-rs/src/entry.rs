//! Entry types for QDB - Key, Value, and Entry definitions

use crate::error::{QdbError, Result};
use num_complex::Complex64;
use qft::QftFile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// =============================================================================
// Key Types
// =============================================================================

/// A database key - supports string, integer, and composite keys
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    /// String key
    String(String),
    /// Integer key
    Integer(i64),
    /// Composite key (multiple parts)
    Composite(Vec<Key>),
    /// Binary key (hash)
    Binary(Vec<u8>),
}

impl Key {
    /// Create a string key
    pub fn string(s: impl Into<String>) -> Self {
        Key::String(s.into())
    }

    /// Create an integer key
    pub fn integer(i: i64) -> Self {
        Key::Integer(i)
    }

    /// Create a composite key
    pub fn composite(parts: Vec<Key>) -> Self {
        Key::Composite(parts)
    }

    /// Create a binary key from bytes
    pub fn binary(data: impl AsRef<[u8]>) -> Self {
        Key::Binary(data.as_ref().to_vec())
    }

    /// Hash the key to a fixed-size binary representation
    pub fn hash(&self) -> [u8; 32] {
        let bytes = self.to_bytes();
        *blake3::hash(&bytes).as_bytes()
    }

    /// Convert key to bytes for hashing
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Key::String(s) => {
                let mut v = vec![0u8]; // type tag
                v.extend(s.as_bytes());
                v
            }
            Key::Integer(i) => {
                let mut v = vec![1u8];
                v.extend(&i.to_le_bytes());
                v
            }
            Key::Composite(parts) => {
                let mut v = vec![2u8];
                for part in parts {
                    let part_bytes = part.to_bytes();
                    v.extend(&(part_bytes.len() as u32).to_le_bytes());
                    v.extend(part_bytes);
                }
                v
            }
            Key::Binary(data) => {
                let mut v = vec![3u8];
                v.extend(data);
                v
            }
        }
    }

    /// Map key to qubit index for quantum operations
    /// Uses hash to distribute keys uniformly across 2^n indices
    pub fn to_qubit_index(&self, num_qubits: usize) -> usize {
        let hash = self.hash();
        let mut index: usize = 0;
        for (i, &byte) in hash.iter().take(8).enumerate() {
            index |= (byte as usize) << (i * 8);
        }
        index % (1 << num_qubits)
    }
}

impl From<&str> for Key {
    fn from(s: &str) -> Self {
        Key::String(s.to_string())
    }
}

impl From<String> for Key {
    fn from(s: String) -> Self {
        Key::String(s)
    }
}

impl From<i64> for Key {
    fn from(i: i64) -> Self {
        Key::Integer(i)
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Key::String(s) => write!(f, "{}", s),
            Key::Integer(i) => write!(f, "{}", i),
            Key::Composite(parts) => {
                let strs: Vec<String> = parts.iter().map(|p| p.to_string()).collect();
                write!(f, "({})", strs.join(", "))
            }
            Key::Binary(data) => write!(f, "0x{}", hex::encode(data)),
        }
    }
}

// =============================================================================
// Value Types
// =============================================================================

/// A database value - supports classical data and quantum states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    /// Null value
    Null,
    /// Boolean value
    Bool(bool),
    /// Integer value
    Integer(i64),
    /// Floating point value
    Float(f64),
    /// String value
    String(String),
    /// Binary data
    Binary(Vec<u8>),
    /// JSON document
    Json(serde_json::Value),
    /// Quantum state (serialized QFT)
    #[serde(skip)]
    Quantum(Box<QftFile>),
    /// Complex number
    Complex { re: f64, im: f64 },
    /// Array of values
    Array(Vec<Value>),
    /// Map of key-value pairs
    Map(HashMap<String, Value>),
}

impl Value {
    /// Create a quantum value from a QFT file
    pub fn quantum(qft: QftFile) -> Self {
        Value::Quantum(Box::new(qft))
    }

    /// Create a complex value
    pub fn complex(re: f64, im: f64) -> Self {
        Value::Complex { re, im }
    }

    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Check if value is a quantum state
    pub fn is_quantum(&self) -> bool {
        matches!(self, Value::Quantum(_))
    }

    /// Get as quantum state if applicable
    pub fn as_quantum(&self) -> Option<&QftFile> {
        match self {
            Value::Quantum(qft) => Some(qft),
            _ => None,
        }
    }

    /// Get as mutable quantum state if applicable
    pub fn as_quantum_mut(&mut self) -> Option<&mut QftFile> {
        match self {
            Value::Quantum(qft) => Some(qft),
            _ => None,
        }
    }

    /// Convert to Complex64 if applicable
    pub fn to_complex(&self) -> Option<Complex64> {
        match self {
            Value::Complex { re, im } => Some(Complex64::new(*re, *im)),
            Value::Float(f) => Some(Complex64::new(*f, 0.0)),
            Value::Integer(i) => Some(Complex64::new(*i as f64, 0.0)),
            _ => None,
        }
    }

    /// Serialize value to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        match self {
            Value::Quantum(qft) => qft.to_bytes().map_err(QdbError::from),
            _ => serde_json::to_vec(self).map_err(QdbError::from),
        }
    }

    /// Get the type name of this value
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::Binary(_) => "binary",
            Value::Json(_) => "json",
            Value::Quantum(_) => "quantum",
            Value::Complex { .. } => "complex",
            Value::Array(_) => "array",
            Value::Map(_) => "map",
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Null
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Integer(i)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl From<QftFile> for Value {
    fn from(qft: QftFile) -> Self {
        Value::Quantum(Box::new(qft))
    }
}

impl From<serde_json::Value> for Value {
    fn from(v: serde_json::Value) -> Self {
        Value::Json(v)
    }
}

// =============================================================================
// Entry Types
// =============================================================================

/// Type of database entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryType {
    /// Classical key-value entry
    KeyValue,
    /// JSON document entry
    Document,
    /// Quantum state entry
    QuantumState,
    /// Tensor network entry (for relational joins)
    TensorNetwork,
}

/// A complete database entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// The entry key
    pub key: Key,
    /// The entry value
    pub value: Value,
    /// Entry type
    pub entry_type: EntryType,
    /// Creation timestamp (Unix epoch milliseconds)
    pub created_at: u64,
    /// Last modified timestamp
    pub modified_at: u64,
    /// Version number for optimistic concurrency
    pub version: u64,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
    /// Checksum for integrity verification
    #[serde(skip)]
    pub checksum: Option<[u8; 32]>,
}

impl Entry {
    /// Create a new entry
    pub fn new(key: Key, value: Value) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let entry_type = match &value {
            Value::Quantum(_) => EntryType::QuantumState,
            Value::Json(_) | Value::Map(_) => EntryType::Document,
            _ => EntryType::KeyValue,
        };

        Self {
            key,
            value,
            entry_type,
            created_at: now,
            modified_at: now,
            version: 1,
            metadata: HashMap::new(),
            checksum: None,
        }
    }

    /// Create a quantum state entry
    pub fn quantum(key: Key, qft: QftFile) -> Self {
        let mut entry = Self::new(key, Value::quantum(qft));
        entry.entry_type = EntryType::QuantumState;
        entry
    }

    /// Create a document entry
    pub fn document(key: Key, doc: serde_json::Value) -> Self {
        let mut entry = Self::new(key, Value::Json(doc));
        entry.entry_type = EntryType::Document;
        entry
    }

    /// Set metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Update the value and increment version
    pub fn update(&mut self, value: Value) {
        self.value = value;
        self.modified_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.version += 1;
        self.checksum = None;
    }

    /// Compute and cache checksum
    pub fn compute_checksum(&mut self) -> Result<[u8; 32]> {
        let value_bytes = self.value.to_bytes()?;
        let key_bytes = self.key.to_bytes();

        let mut hasher = blake3::Hasher::new();
        hasher.update(&key_bytes);
        hasher.update(&value_bytes);
        hasher.update(&self.version.to_le_bytes());

        let hash = *hasher.finalize().as_bytes();
        self.checksum = Some(hash);
        Ok(hash)
    }

    /// Verify checksum
    pub fn verify_checksum(&self) -> Result<bool> {
        if let Some(stored) = self.checksum {
            let value_bytes = self.value.to_bytes()?;
            let key_bytes = self.key.to_bytes();

            let mut hasher = blake3::Hasher::new();
            hasher.update(&key_bytes);
            hasher.update(&value_bytes);
            hasher.update(&self.version.to_le_bytes());

            let computed = *hasher.finalize().as_bytes();
            Ok(stored == computed)
        } else {
            Ok(true) // No checksum to verify
        }
    }

    /// Get the size of this entry in bytes (approximate)
    pub fn size_bytes(&self) -> usize {
        let key_size = self.key.to_bytes().len();
        let value_size = self.value.to_bytes().map(|b| b.len()).unwrap_or(0);
        let metadata_size: usize = self
            .metadata
            .iter()
            .map(|(k, v)| k.len() + v.len())
            .sum();

        key_size + value_size + metadata_size + 64 // overhead for timestamps, version, etc.
    }
}

// Hex encoding helper (minimal implementation)
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_string() {
        let key = Key::string("test");
        assert_eq!(key.to_string(), "test");
    }

    #[test]
    fn test_key_hash() {
        let key1 = Key::string("test");
        let key2 = Key::string("test");
        let key3 = Key::string("other");

        assert_eq!(key1.hash(), key2.hash());
        assert_ne!(key1.hash(), key3.hash());
    }

    #[test]
    fn test_key_qubit_index() {
        let key = Key::string("test");
        let idx = key.to_qubit_index(10);
        assert!(idx < 1024);
    }

    #[test]
    fn test_value_types() {
        assert!(Value::Null.is_null());
        assert!(!Value::Integer(42).is_null());

        let qft = QftFile::new(2).unwrap();
        let quantum_val = Value::quantum(qft);
        assert!(quantum_val.is_quantum());
    }

    #[test]
    fn test_entry_creation() {
        let entry = Entry::new(Key::string("key1"), Value::Integer(42));
        assert_eq!(entry.entry_type, EntryType::KeyValue);
        assert_eq!(entry.version, 1);
    }

    #[test]
    fn test_entry_update() {
        let mut entry = Entry::new(Key::string("key1"), Value::Integer(42));
        entry.update(Value::Integer(100));
        assert_eq!(entry.version, 2);
    }

    #[test]
    fn test_entry_checksum() {
        let mut entry = Entry::new(Key::string("key1"), Value::Integer(42));
        let checksum = entry.compute_checksum().unwrap();
        assert!(entry.verify_checksum().unwrap());

        // Modify without updating checksum
        entry.value = Value::Integer(100);
        assert!(!entry.verify_checksum().unwrap());
    }
}
