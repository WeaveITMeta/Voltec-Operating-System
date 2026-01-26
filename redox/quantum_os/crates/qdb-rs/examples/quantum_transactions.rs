//! Quantum Transaction Examples - Entanglement Groups
//!
//! Demonstrates quantum-aware ACID transactions with entanglement semantics.
//! Run with: cargo run --example quantum_transactions

use qdb_rs::prelude::*;
use qdb_rs::storage::MemoryBackend;
use qdb_rs::transaction::{TransactionManager, QuantumTransaction, IsolationLevel};
use std::sync::Arc;

fn main() -> Result<()> {
    println!("=== Quantum Transaction Examples ===\n");

    let backend = Arc::new(MemoryBackend::new());

    // =========================================================================
    // 1. Basic Transaction
    // =========================================================================
    println!("1. Basic Transaction (Commit)");
    
    {
        let manager = TransactionManager::new(backend.clone());
        let tx = manager.begin();

        tx.put("tx:key1", "value1")?;
        tx.put("tx:key2", "value2")?;
        
        println!("   Pending writes: {}", tx.pending_writes());
        tx.commit()?;
        println!("   Transaction committed successfully");
    }

    // Verify committed data
    if let Some(entry) = backend.get(&Key::string("tx:key1"))? {
        println!("   Verified: tx:key1 = {:?}\n", entry.value);
    }

    // =========================================================================
    // 2. Transaction Rollback
    // =========================================================================
    println!("2. Transaction Rollback");
    
    {
        let manager = TransactionManager::new(backend.clone());
        let tx = manager.begin();

        tx.put("rollback:key", "should_not_exist")?;
        println!("   Pending writes before rollback: {}", tx.pending_writes());
        
        tx.rollback()?;
        println!("   Transaction rolled back");
    }

    // Verify data was not persisted
    let exists = backend.exists(&Key::string("rollback:key"))?;
    println!("   Key exists after rollback: {}\n", exists);

    // =========================================================================
    // 3. Quantum Transaction with Entanglement Groups
    // =========================================================================
    println!("3. Quantum Transaction with Entanglement Groups");
    println!("   (Entangled keys must be committed together)");
    
    {
        let manager = TransactionManager::new(backend.clone());
        let tx = manager.begin();
        let qtx = QuantumTransaction::from_transaction(tx);

        // Create two entangled quantum states
        let state_a = bell_state()?;
        let state_b = bell_state()?;

        // Mark these keys as entangled - they MUST be written together
        qtx.entangle(vec![
            Key::string("entangled:alice"),
            Key::string("entangled:bob"),
        ])?;

        // Write both entangled states
        qtx.put("entangled:alice", Value::quantum(state_a))?;
        qtx.put("entangled:bob", Value::quantum(state_b))?;

        qtx.commit()?;
        println!("   Entangled transaction committed successfully");
    }

    // Verify both states exist
    let alice_exists = backend.exists(&Key::string("entangled:alice"))?;
    let bob_exists = backend.exists(&Key::string("entangled:bob"))?;
    println!("   Alice exists: {}, Bob exists: {}\n", alice_exists, bob_exists);

    // =========================================================================
    // 4. Entanglement Violation (Should Fail)
    // =========================================================================
    println!("4. Entanglement Violation Detection");
    
    {
        let manager = TransactionManager::new(backend.clone());
        let tx = manager.begin();
        let qtx = QuantumTransaction::from_transaction(tx);

        // Mark keys as entangled
        qtx.entangle(vec![
            Key::string("violation:q1"),
            Key::string("violation:q2"),
        ])?;

        // Only write ONE of the entangled keys (violation!)
        qtx.put("violation:q1", "partial_write")?;
        // Missing: qtx.put("violation:q2", ...)?;

        match qtx.commit() {
            Ok(_) => println!("   ERROR: Should have failed!"),
            Err(e) => println!("   Correctly detected violation: {}", e),
        }
    }
    println!();

    // =========================================================================
    // 5. Isolation Levels
    // =========================================================================
    println!("5. Transaction Isolation Levels");
    
    {
        let manager = TransactionManager::new(backend.clone());
        
        // Serializable isolation (strictest)
        let tx = manager.begin_with_isolation(IsolationLevel::Serializable);
        tx.put("isolated:key", "serializable_value")?;
        tx.commit()?;
        println!("   Serializable transaction committed");

        // Read Committed isolation
        let tx = manager.begin_with_isolation(IsolationLevel::ReadCommitted);
        let _ = tx.get("isolated:key")?;
        tx.commit()?;
        println!("   ReadCommitted transaction committed");
    }

    println!("\n=== Quantum Transactions Complete ===");
    Ok(())
}
