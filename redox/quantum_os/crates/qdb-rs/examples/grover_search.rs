//! Grover Search Examples - O(√N) Quantum Search
//!
//! Demonstrates Grover-inspired search for unstructured data.
//! Run with: cargo run --example grover_search

use qdb_rs::prelude::*;
use qdb_rs::query::{GroverSearch, Query, Filter, OrderBy, SortDirection};
use qdb_rs::storage::MemoryBackend;
use std::time::Instant;

fn main() -> Result<()> {
    println!("=== Grover Search Examples ===\n");

    // =========================================================================
    // 1. Basic Grover Search
    // =========================================================================
    println!("1. Basic Grover Search");
    println!("   Finding index 42 in a space of 1024 elements\n");

    let target = 42;
    let search_space = 1024;

    let search = GroverSearch::new(search_space, move |i| i == target);
    
    let start = Instant::now();
    let candidates = search.search()?;
    let elapsed = start.elapsed();

    println!("   Target: {}", target);
    println!("   Search space: {}", search_space);
    println!("   Top candidates: {:?}", &candidates[..candidates.len().min(5)]);
    println!("   Found target: {}", candidates.contains(&target));
    println!("   Time: {:?}\n", elapsed);

    // =========================================================================
    // 2. Grover Search with State Output
    // =========================================================================
    println!("2. Grover Search with Quantum State Output");

    let (candidates, final_state) = GroverSearch::new(64, |i| i == 7)
        .search_with_state()?;

    println!("   Search space: 64 (6 qubits)");
    println!("   Target: 7");
    println!("   Final state qubits: {}", final_state.num_qubits());
    println!("   Candidates: {:?}", candidates);
    
    // Check amplitude at target
    let target_amp = final_state.get_amplitude(7)?;
    println!("   Amplitude at target |7⟩: {:.4} + {:.4}i", 
        target_amp.re, target_amp.im);
    println!("   Probability: {:.4}\n", target_amp.norm_sqr());

    // =========================================================================
    // 3. Multiple Target Search
    // =========================================================================
    println!("3. Multiple Target Search (Divisible by 7)");

    let search = GroverSearch::new(128, |i| i % 7 == 0 && i > 0);
    let candidates = search.search()?;

    println!("   Search space: 128");
    println!("   Targets: multiples of 7");
    println!("   Found: {:?}", candidates);
    
    let valid_count = candidates.iter().filter(|&&x| x % 7 == 0 && x > 0).count();
    println!("   Valid multiples found: {}\n", valid_count);

    // =========================================================================
    // 4. Comparison: Grover vs Linear Search
    // =========================================================================
    println!("4. Performance Comparison: Grover vs Linear");

    for size in [256, 1024, 4096] {
        let target = size / 2;

        // Grover search
        let start = Instant::now();
        let grover = GroverSearch::new(size, move |i| i == target);
        let grover_result = grover.search()?;
        let grover_time = start.elapsed();

        // Linear search (classical)
        let start = Instant::now();
        let mut linear_result = None;
        for i in 0..size {
            if i == target {
                linear_result = Some(i);
                break;
            }
        }
        let linear_time = start.elapsed();

        println!("   N={}: Grover {:?}, Linear {:?}", 
            size, grover_time, linear_time);
        println!("      Grover found: {}, Linear found: {:?}",
            grover_result.contains(&target), linear_result);
    }
    println!();

    // =========================================================================
    // 5. Query API with Filters
    // =========================================================================
    println!("5. Query API with Filters");

    let db = QDB::new(MemoryBackend::new());

    // Populate database
    for i in 0..100 {
        let key = format!("item:{:03}", i);
        db.put(&key, Value::Integer(i as i64))?;
    }

    // Add some quantum states
    db.put_quantum("quantum:bell", bell_state()?)?;
    db.put_quantum("quantum:ghz", ghz_state(4)?)?;

    // Query with filters
    let results = Query::new()
        .key_prefix("item:0".to_string())  // Keys starting with "item:0"
        .order_by(OrderBy::Key(SortDirection::Ascending))
        .limit(5)
        .execute(db.backend())?;

    println!("   Query: key_prefix='item:0', limit=5");
    println!("   Results: {} entries", results.len());
    for entry in results.iter() {
        println!("      {} = {:?}", entry.key, entry.value);
    }
    println!();

    // Query quantum states only
    let quantum_results = Query::new()
        .entry_type(EntryType::QuantumState)
        .execute(db.backend())?;

    println!("   Query: entry_type=QuantumState");
    println!("   Found {} quantum states", quantum_results.len());
    for entry in quantum_results.iter() {
        if let Some(qft) = entry.value.as_quantum() {
            println!("      {}: {} qubits", entry.key, qft.num_qubits());
        }
    }

    println!("\n=== Grover Search Examples Complete ===");
    Ok(())
}
