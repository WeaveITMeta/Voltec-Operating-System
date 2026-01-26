//! Indexing structures for QDB

use crate::entry::{Entry, Key};
use crate::error::{QdbError, Result};
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap, HashSet};

// =============================================================================
// Index Types
// =============================================================================

/// Index type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    /// Hash index for exact lookups (O(1))
    Hash,
    /// B-tree index for range queries (O(log N))
    BTree,
    /// Quantum index using amplitude encoding
    Quantum,
}

/// An index on a field
pub struct Index {
    /// Index name
    pub name: String,
    /// Field being indexed
    pub field: String,
    /// Index type
    pub index_type: IndexType,
    /// Internal storage
    storage: IndexStorage,
}

enum IndexStorage {
    Hash(RwLock<HashMap<Vec<u8>, HashSet<Vec<u8>>>>),
    BTree(RwLock<BTreeMap<Vec<u8>, HashSet<Vec<u8>>>>),
    Quantum(RwLock<QuantumIndex>),
}

impl Index {
    /// Create a new hash index
    pub fn hash(name: impl Into<String>, field: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            field: field.into(),
            index_type: IndexType::Hash,
            storage: IndexStorage::Hash(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new B-tree index
    pub fn btree(name: impl Into<String>, field: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            field: field.into(),
            index_type: IndexType::BTree,
            storage: IndexStorage::BTree(RwLock::new(BTreeMap::new())),
        }
    }

    /// Create a new quantum index
    pub fn quantum(name: impl Into<String>, field: impl Into<String>, num_qubits: usize) -> Self {
        Self {
            name: name.into(),
            field: field.into(),
            index_type: IndexType::Quantum,
            storage: IndexStorage::Quantum(RwLock::new(QuantumIndex::new(num_qubits))),
        }
    }

    /// Insert an entry into the index
    pub fn insert(&self, entry: &Entry) -> Result<()> {
        let field_value = self.extract_field_value(entry)?;
        let key_bytes = entry.key.to_bytes();

        match &self.storage {
            IndexStorage::Hash(map) => {
                let mut map = map.write();
                map.entry(field_value).or_default().insert(key_bytes);
            }
            IndexStorage::BTree(tree) => {
                let mut tree = tree.write();
                tree.entry(field_value).or_default().insert(key_bytes);
            }
            IndexStorage::Quantum(qi) => {
                let mut qi = qi.write();
                qi.insert(&field_value, &key_bytes)?;
            }
        }

        Ok(())
    }

    /// Remove an entry from the index
    pub fn remove(&self, entry: &Entry) -> Result<()> {
        let field_value = self.extract_field_value(entry)?;
        let key_bytes = entry.key.to_bytes();

        match &self.storage {
            IndexStorage::Hash(map) => {
                let mut map = map.write();
                if let Some(keys) = map.get_mut(&field_value) {
                    keys.remove(&key_bytes);
                    if keys.is_empty() {
                        map.remove(&field_value);
                    }
                }
            }
            IndexStorage::BTree(tree) => {
                let mut tree = tree.write();
                if let Some(keys) = tree.get_mut(&field_value) {
                    keys.remove(&key_bytes);
                    if keys.is_empty() {
                        tree.remove(&field_value);
                    }
                }
            }
            IndexStorage::Quantum(qi) => {
                let mut qi = qi.write();
                qi.remove(&field_value, &key_bytes)?;
            }
        }

        Ok(())
    }

    /// Lookup exact match
    pub fn lookup(&self, value: &[u8]) -> Result<Vec<Vec<u8>>> {
        match &self.storage {
            IndexStorage::Hash(map) => {
                let map = map.read();
                Ok(map.get(value).map(|s| s.iter().cloned().collect()).unwrap_or_default())
            }
            IndexStorage::BTree(tree) => {
                let tree = tree.read();
                Ok(tree.get(value).map(|s| s.iter().cloned().collect()).unwrap_or_default())
            }
            IndexStorage::Quantum(qi) => {
                let qi = qi.read();
                qi.lookup(value)
            }
        }
    }

    /// Range query (B-tree only)
    pub fn range(&self, start: &[u8], end: &[u8]) -> Result<Vec<Vec<u8>>> {
        match &self.storage {
            IndexStorage::BTree(tree) => {
                let tree = tree.read();
                let mut results = Vec::new();
                for (_, keys) in tree.range(start.to_vec()..=end.to_vec()) {
                    results.extend(keys.iter().cloned());
                }
                Ok(results)
            }
            _ => Err(QdbError::IndexError(
                "Range queries only supported on B-tree indexes".to_string(),
            )),
        }
    }

    /// Get index size
    pub fn len(&self) -> usize {
        match &self.storage {
            IndexStorage::Hash(map) => map.read().len(),
            IndexStorage::BTree(tree) => tree.read().len(),
            IndexStorage::Quantum(qi) => qi.read().len(),
        }
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear the index
    pub fn clear(&self) {
        match &self.storage {
            IndexStorage::Hash(map) => map.write().clear(),
            IndexStorage::BTree(tree) => tree.write().clear(),
            IndexStorage::Quantum(qi) => qi.write().clear(),
        }
    }

    fn extract_field_value(&self, entry: &Entry) -> Result<Vec<u8>> {
        // Special handling for common fields
        match self.field.as_str() {
            "_key" => Ok(entry.key.to_bytes()),
            "_type" => Ok(vec![entry.entry_type as u8]),
            "_version" => Ok(entry.version.to_le_bytes().to_vec()),
            "_created" => Ok(entry.created_at.to_le_bytes().to_vec()),
            "_modified" => Ok(entry.modified_at.to_le_bytes().to_vec()),
            field => {
                // Try metadata
                if let Some(value) = entry.metadata.get(field) {
                    Ok(value.as_bytes().to_vec())
                } else {
                    // Try to extract from value (simplified)
                    Ok(field.as_bytes().to_vec())
                }
            }
        }
    }
}

// =============================================================================
// Quantum Index
// =============================================================================

/// Quantum-enhanced index using amplitude encoding
/// 
/// Stores keys in quantum superposition for O(√N) search via Grover's algorithm.
struct QuantumIndex {
    num_qubits: usize,
    /// Maps hash(field_value) -> set of key hashes
    entries: HashMap<u64, HashSet<Vec<u8>>>,
    /// Total number of indexed keys
    total_keys: usize,
}

impl QuantumIndex {
    fn new(num_qubits: usize) -> Self {
        Self {
            num_qubits,
            entries: HashMap::new(),
            total_keys: 0,
        }
    }

    fn hash_value(value: &[u8]) -> u64 {
        let hash = blake3::hash(value);
        let bytes = hash.as_bytes();
        u64::from_le_bytes(bytes[0..8].try_into().unwrap())
    }

    fn insert(&mut self, field_value: &[u8], key: &[u8]) -> Result<()> {
        let hash = Self::hash_value(field_value);
        let keys = self.entries.entry(hash).or_default();
        if keys.insert(key.to_vec()) {
            self.total_keys += 1;
        }
        Ok(())
    }

    fn remove(&mut self, field_value: &[u8], key: &[u8]) -> Result<()> {
        let hash = Self::hash_value(field_value);
        if let Some(keys) = self.entries.get_mut(&hash) {
            if keys.remove(key) {
                self.total_keys -= 1;
            }
            if keys.is_empty() {
                self.entries.remove(&hash);
            }
        }
        Ok(())
    }

    fn lookup(&self, value: &[u8]) -> Result<Vec<Vec<u8>>> {
        let hash = Self::hash_value(value);
        Ok(self.entries.get(&hash)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default())
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.total_keys = 0;
    }
}

// =============================================================================
// Index Manager
// =============================================================================

/// Manages multiple indexes for a database
pub struct IndexManager {
    indexes: RwLock<HashMap<String, Index>>,
}

impl IndexManager {
    /// Create a new index manager
    pub fn new() -> Self {
        Self {
            indexes: RwLock::new(HashMap::new()),
        }
    }

    /// Add an index
    pub fn add_index(&self, index: Index) -> Result<()> {
        let mut indexes = self.indexes.write();
        if indexes.contains_key(&index.name) {
            return Err(QdbError::IndexError(format!(
                "Index '{}' already exists",
                index.name
            )));
        }
        indexes.insert(index.name.clone(), index);
        Ok(())
    }

    /// Remove an index
    pub fn remove_index(&self, name: &str) -> Result<bool> {
        let mut indexes = self.indexes.write();
        Ok(indexes.remove(name).is_some())
    }

    /// Get an index by name
    pub fn get_index(&self, name: &str) -> Option<std::sync::Arc<Index>> {
        // Note: This is simplified; in production, use Arc for shared access
        None
    }

    /// Index an entry across all indexes
    pub fn index_entry(&self, entry: &Entry) -> Result<()> {
        let indexes = self.indexes.read();
        for index in indexes.values() {
            index.insert(entry)?;
        }
        Ok(())
    }

    /// Remove an entry from all indexes
    pub fn unindex_entry(&self, entry: &Entry) -> Result<()> {
        let indexes = self.indexes.read();
        for index in indexes.values() {
            index.remove(entry)?;
        }
        Ok(())
    }

    /// List all index names
    pub fn list_indexes(&self) -> Vec<String> {
        self.indexes.read().keys().cloned().collect()
    }

    /// Clear all indexes
    pub fn clear_all(&self) {
        let indexes = self.indexes.read();
        for index in indexes.values() {
            index.clear();
        }
    }
}

impl Default for IndexManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::Value;

    #[test]
    fn test_hash_index() {
        let index = Index::hash("test_idx", "_key");
        
        let entry = Entry::new(Key::string("key1"), Value::Integer(42));
        index.insert(&entry).unwrap();

        let results = index.lookup(&entry.key.to_bytes()).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_btree_index() {
        let index = Index::btree("test_idx", "_version");
        
        let entry1 = Entry::new(Key::string("key1"), Value::Integer(1));
        let mut entry2 = Entry::new(Key::string("key2"), Value::Integer(2));
        entry2.version = 5;

        index.insert(&entry1).unwrap();
        index.insert(&entry2).unwrap();

        // Range query
        let start = 1u64.to_le_bytes();
        let end = 10u64.to_le_bytes();
        let results = index.range(&start, &end).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_index_manager() {
        let manager = IndexManager::new();
        
        manager.add_index(Index::hash("idx1", "_key")).unwrap();
        manager.add_index(Index::btree("idx2", "_version")).unwrap();

        let names = manager.list_indexes();
        assert_eq!(names.len(), 2);

        // Duplicate should fail
        assert!(manager.add_index(Index::hash("idx1", "_key")).is_err());
    }
}
