//! Basic QDB usage examples
//!
//! Run with: cargo run --example basic_usage

use qdb_rs::prelude::*;
use qdb_rs::storage::MemoryBackend;

fn main() -> Result<()> {
    println!("=== QDB Basic Usage Examples ===\n");

    // Create an in-memory database
    let db = QDB::new(MemoryBackend::new());

    // =========================================================================
    // 1. Classical Key-Value Storage
    // =========================================================================
    println!("1. Classical Key-Value Storage");
    
    db.put("user:1", "Alice")?;
    db.put("user:2", "Bob")?;
    db.put("counter", Value::Integer(42))?;

    if let Some(Value::String(name)) = db.get("user:1")? {
        println!("   User 1: {}", name);
    }

    if let Some(Value::Integer(count)) = db.get("counter")? {
        println!("   Counter: {}", count);
    }

    println!("   Total entries: {}\n", db.len()?);

    // =========================================================================
    // 2. Quantum State Storage
    // =========================================================================
    println!("2. Quantum State Storage");

    // Create a Bell state |00⟩ + |11⟩
    let bell = bell_state()?;
    println!("   Created Bell state: {} qubits, norm={:.6}", 
        bell.num_qubits(), bell.norm());

    // Store in database
    db.put_quantum("entangled_pair", bell)?;

    // Retrieve and verify
    if let Some(loaded) = db.get_quantum("entangled_pair")? {
        println!("   Retrieved: {} qubits", loaded.num_qubits());
        println!("   Fidelity with original: {:.10}", 
            bell_state()?.fidelity(&loaded)?);
    }

    // Create a GHZ state
    let ghz = ghz_state(5)?;
    db.put_quantum("ghz_5qubit", ghz)?;
    println!("   Stored 5-qubit GHZ state\n");

    // =========================================================================
    // 3. Custom Quantum States with Metadata
    // =========================================================================
    println!("3. Custom Quantum States with Metadata");

    let vqe_state = QftBuilder::new(4)
        .bond_dimension(128)
        .golay(true)
        .truncation_threshold(1e-12)
        .metadata("algorithm", "VQE")
        .metadata("molecule", "H2")
        .metadata("energy", "-1.137")
        .build()?;

    db.put_quantum("vqe_h2_ground", vqe_state)?;

    if let Some(entry) = db.get_entry("vqe_h2_ground")? {
        println!("   Entry type: {:?}", entry.entry_type);
        println!("   Version: {}", entry.version);
        if let Some(qft) = entry.value.as_quantum() {
            println!("   Bond dimension: {}", qft.config().bond_dimension);
            println!("   Golay enabled: {}", qft.config().golay_enabled);
        }
    }
    println!();

    // =========================================================================
    // 4. Batch Operations
    // =========================================================================
    println!("4. Batch Operations");

    let entries = vec![
        Entry::new(Key::string("batch:1"), Value::Integer(100)),
        Entry::new(Key::string("batch:2"), Value::Integer(200)),
        Entry::new(Key::string("batch:3"), Value::Integer(300)),
    ];

    db.put_batch(entries)?;

    let keys = vec![
        Key::string("batch:1"),
        Key::string("batch:2"),
        Key::string("batch:3"),
    ];
    let values = db.get_batch(keys)?;
    
    println!("   Batch retrieved: {} values", values.len());
    for (i, v) in values.iter().enumerate() {
        if let Some(Value::Integer(n)) = v {
            println!("   batch:{} = {}", i + 1, n);
        }
    }
    println!();

    // =========================================================================
    // 5. Database Statistics
    // =========================================================================
    println!("5. Database Statistics");
    
    let stats = db.stats();
    println!("   Reads: {}", stats.reads.load(std::sync::atomic::Ordering::Relaxed));
    println!("   Writes: {}", stats.writes.load(std::sync::atomic::Ordering::Relaxed));
    println!("   Total entries: {}", db.len()?);

    println!("\n=== Examples Complete ===");
    Ok(())
}
