//! Transaction support for QDB - Quantum-aware ACID semantics

use crate::entry::{Entry, Key, Value};
use crate::error::{QdbError, Result};
use crate::storage::Backend;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// =============================================================================
// Transaction Types
// =============================================================================

/// Transaction isolation level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    /// Read uncommitted - no isolation
    ReadUncommitted = 0,
    /// Read committed - only see committed data
    ReadCommitted = 1,
    /// Repeatable read - consistent snapshot
    RepeatableRead = 2,
    /// Serializable - full isolation
    Serializable = 3,
}

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    /// Transaction is active
    Active,
    /// Transaction is preparing to commit
    Preparing,
    /// Transaction is committed
    Committed,
    /// Transaction is rolled back
    RolledBack,
}

/// A database transaction
pub struct Transaction<B: Backend> {
    /// Transaction ID
    pub id: u64,
    /// Backend reference
    backend: Arc<B>,
    /// Isolation level
    isolation: IsolationLevel,
    /// Current state
    state: RwLock<TransactionState>,
    /// Write set (pending changes)
    write_set: Mutex<HashMap<Vec<u8>, WriteOp>>,
    /// Read set (for conflict detection)
    read_set: Mutex<HashMap<Vec<u8>, u64>>, // key -> version
    /// Start time
    start_time: Instant,
    /// Timeout
    timeout: Duration,
}

/// Write operation type
#[derive(Debug, Clone)]
enum WriteOp {
    Put(Entry),
    Delete,
}

impl<B: Backend> Transaction<B> {
    /// Create a new transaction
    pub fn new(id: u64, backend: Arc<B>, isolation: IsolationLevel) -> Self {
        Self {
            id,
            backend,
            isolation,
            state: RwLock::new(TransactionState::Active),
            write_set: Mutex::new(HashMap::new()),
            read_set: Mutex::new(HashMap::new()),
            start_time: Instant::now(),
            timeout: Duration::from_secs(30),
        }
    }

    /// Set transaction timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Check if transaction is still valid
    fn check_valid(&self) -> Result<()> {
        let state = *self.state.read();
        if state != TransactionState::Active {
            return Err(QdbError::TransactionError(format!(
                "Transaction {} is not active (state: {:?})",
                self.id, state
            )));
        }

        if self.start_time.elapsed() > self.timeout {
            return Err(QdbError::TransactionError(format!(
                "Transaction {} timed out",
                self.id
            )));
        }

        Ok(())
    }

    /// Get a value within the transaction
    pub fn get(&self, key: impl Into<Key>) -> Result<Option<Value>> {
        self.check_valid()?;
        let key = key.into();
        let key_bytes = key.to_bytes();

        // Check write set first
        {
            let write_set = self.write_set.lock();
            if let Some(op) = write_set.get(&key_bytes) {
                return match op {
                    WriteOp::Put(entry) => Ok(Some(entry.value.clone())),
                    WriteOp::Delete => Ok(None),
                };
            }
        }

        // Read from backend
        let entry = self.backend.get(&key)?;

        // Record in read set for conflict detection
        if let Some(ref e) = entry {
            let mut read_set = self.read_set.lock();
            read_set.insert(key_bytes, e.version);
        }

        Ok(entry.map(|e| e.value))
    }

    /// Put a value within the transaction
    pub fn put(&self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        self.check_valid()?;
        let entry = Entry::new(key.into(), value.into());
        let key_bytes = entry.key.to_bytes();

        let mut write_set = self.write_set.lock();
        write_set.insert(key_bytes, WriteOp::Put(entry));
        Ok(())
    }

    /// Delete a key within the transaction
    pub fn delete(&self, key: impl Into<Key>) -> Result<()> {
        self.check_valid()?;
        let key = key.into();
        let key_bytes = key.to_bytes();

        let mut write_set = self.write_set.lock();
        write_set.insert(key_bytes, WriteOp::Delete);
        Ok(())
    }

    /// Commit the transaction
    pub fn commit(self) -> Result<()> {
        self.check_valid()?;

        // Set state to preparing
        {
            let mut state = self.state.write();
            *state = TransactionState::Preparing;
        }

        // Validate read set (optimistic concurrency control)
        if (self.isolation as u8) >= (IsolationLevel::RepeatableRead as u8) {
            let read_set = self.read_set.lock();
            for (key_bytes, expected_version) in read_set.iter() {
                // Reconstruct key for lookup
                if let Ok(Some(current)) = self.backend.get(&Key::Binary(key_bytes.clone())) {
                    if current.version != *expected_version {
                        let mut state = self.state.write();
                        *state = TransactionState::RolledBack;
                        return Err(QdbError::ConcurrentModification(
                            "Read set validation failed".to_string(),
                        ));
                    }
                }
            }
        }

        // Apply write set
        let write_set = self.write_set.lock();
        for (key_bytes, op) in write_set.iter() {
            match op {
                WriteOp::Put(entry) => {
                    self.backend.put(entry.clone())?;
                }
                WriteOp::Delete => {
                    self.backend.delete(&Key::Binary(key_bytes.clone()))?;
                }
            }
        }

        // Set state to committed
        {
            let mut state = self.state.write();
            *state = TransactionState::Committed;
        }

        Ok(())
    }

    /// Rollback the transaction
    pub fn rollback(self) -> Result<()> {
        let mut state = self.state.write();
        *state = TransactionState::RolledBack;
        // Write set is discarded automatically
        Ok(())
    }

    /// Get transaction state
    pub fn state(&self) -> TransactionState {
        *self.state.read()
    }

    /// Get number of pending writes
    pub fn pending_writes(&self) -> usize {
        self.write_set.lock().len()
    }
}

// =============================================================================
// Transaction Manager
// =============================================================================

/// Manages transactions for a database
pub struct TransactionManager<B: Backend> {
    backend: Arc<B>,
    next_id: AtomicU64,
    active_transactions: RwLock<HashMap<u64, Arc<Mutex<TransactionState>>>>,
    default_isolation: IsolationLevel,
}

impl<B: Backend> TransactionManager<B> {
    /// Create a new transaction manager
    pub fn new(backend: Arc<B>) -> Self {
        Self {
            backend,
            next_id: AtomicU64::new(1),
            active_transactions: RwLock::new(HashMap::new()),
            default_isolation: IsolationLevel::ReadCommitted,
        }
    }

    /// Set default isolation level
    pub fn with_isolation(mut self, isolation: IsolationLevel) -> Self {
        self.default_isolation = isolation;
        self
    }

    /// Begin a new transaction
    pub fn begin(&self) -> Transaction<B> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let tx = Transaction::new(id, self.backend.clone(), self.default_isolation);

        // Track active transaction
        {
            let mut active = self.active_transactions.write();
            active.insert(id, Arc::new(Mutex::new(TransactionState::Active)));
        }

        tx
    }

    /// Begin a transaction with specific isolation level
    pub fn begin_with_isolation(&self, isolation: IsolationLevel) -> Transaction<B> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let tx = Transaction::new(id, self.backend.clone(), isolation);

        {
            let mut active = self.active_transactions.write();
            active.insert(id, Arc::new(Mutex::new(TransactionState::Active)));
        }

        tx
    }

    /// Get count of active transactions
    pub fn active_count(&self) -> usize {
        self.active_transactions.read().len()
    }

    /// Clean up completed transactions
    pub fn cleanup(&self) {
        let mut active = self.active_transactions.write();
        active.retain(|_, state| {
            let s = *state.lock();
            s == TransactionState::Active || s == TransactionState::Preparing
        });
    }
}

// =============================================================================
// Quantum Transaction Extensions
// =============================================================================

/// Quantum-aware transaction that can handle entangled states
pub struct QuantumTransaction<B: Backend> {
    inner: Transaction<B>,
    /// Entanglement groups - keys that must be committed atomically
    entanglement_groups: Mutex<Vec<Vec<Key>>>,
}

impl<B: Backend> QuantumTransaction<B> {
    /// Create from a regular transaction
    pub fn from_transaction(tx: Transaction<B>) -> Self {
        Self {
            inner: tx,
            entanglement_groups: Mutex::new(Vec::new()),
        }
    }

    /// Mark keys as entangled (must be committed together)
    pub fn entangle(&self, keys: Vec<Key>) -> Result<()> {
        self.inner.check_valid()?;
        let mut groups = self.entanglement_groups.lock();
        groups.push(keys);
        Ok(())
    }

    /// Get a value
    pub fn get(&self, key: impl Into<Key>) -> Result<Option<Value>> {
        self.inner.get(key)
    }

    /// Put a value
    pub fn put(&self, key: impl Into<Key>, value: impl Into<Value>) -> Result<()> {
        self.inner.put(key, value)
    }

    /// Delete a key
    pub fn delete(&self, key: impl Into<Key>) -> Result<()> {
        self.inner.delete(key)
    }

    /// Commit with entanglement verification
    pub fn commit(self) -> Result<()> {
        // Verify all entangled keys are in write set
        {
            let groups = self.entanglement_groups.lock();
            let write_set = self.inner.write_set.lock();

            for group in groups.iter() {
                let all_present = group.iter().all(|k| write_set.contains_key(&k.to_bytes()));
                if !all_present {
                    return Err(QdbError::TransactionError(
                        "Entangled keys must all be written together".to_string(),
                    ));
                }
            }
        }

        self.inner.commit()
    }

    /// Rollback
    pub fn rollback(self) -> Result<()> {
        self.inner.rollback()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryBackend;

    #[test]
    fn test_transaction_basic() {
        let backend = Arc::new(MemoryBackend::new());
        let manager = TransactionManager::new(backend.clone());

        let tx = manager.begin();
        tx.put("key1", "value1").unwrap();
        tx.put("key2", "value2").unwrap();
        tx.commit().unwrap();

        // Verify committed
        let entry = backend.get(&Key::string("key1")).unwrap();
        assert!(entry.is_some());
    }

    #[test]
    fn test_transaction_rollback() {
        let backend = Arc::new(MemoryBackend::new());
        let manager = TransactionManager::new(backend.clone());

        let tx = manager.begin();
        tx.put("key1", "value1").unwrap();
        tx.rollback().unwrap();

        // Verify not committed
        let entry = backend.get(&Key::string("key1")).unwrap();
        assert!(entry.is_none());
    }

    #[test]
    fn test_transaction_isolation() {
        let backend = Arc::new(MemoryBackend::new());
        let manager = TransactionManager::new(backend.clone());

        // Pre-populate
        backend.put(Entry::new(Key::string("key1"), Value::Integer(1))).unwrap();

        let tx = manager.begin();
        
        // Read within transaction
        let value = tx.get("key1").unwrap();
        assert!(matches!(value, Some(Value::Integer(1))));

        // Write within transaction
        tx.put("key1", Value::Integer(2)).unwrap();

        // Read again - should see uncommitted write
        let value = tx.get("key1").unwrap();
        assert!(matches!(value, Some(Value::Integer(2))));

        tx.commit().unwrap();
    }

    #[test]
    fn test_quantum_transaction() {
        let backend = Arc::new(MemoryBackend::new());
        let manager = TransactionManager::new(backend.clone());

        let tx = manager.begin();
        let qtx = QuantumTransaction::from_transaction(tx);

        // Mark keys as entangled
        qtx.entangle(vec![Key::string("q1"), Key::string("q2")]).unwrap();

        // Must write both
        qtx.put("q1", "state1").unwrap();
        qtx.put("q2", "state2").unwrap();

        qtx.commit().unwrap();
    }

    #[test]
    fn test_quantum_transaction_entanglement_violation() {
        let backend = Arc::new(MemoryBackend::new());
        let manager = TransactionManager::new(backend.clone());

        let tx = manager.begin();
        let qtx = QuantumTransaction::from_transaction(tx);

        // Mark keys as entangled
        qtx.entangle(vec![Key::string("q1"), Key::string("q2")]).unwrap();

        // Only write one - should fail
        qtx.put("q1", "state1").unwrap();

        let result = qtx.commit();
        assert!(result.is_err());
    }
}
