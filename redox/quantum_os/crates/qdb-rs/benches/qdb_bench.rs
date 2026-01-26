//! Comprehensive Benchmarks for QDB Operations
//!
//! Run with: cargo bench -p qdb-rs
//!
//! Benchmark Categories:
//! 1. Classical Key-Value Operations (put/get throughput)
//! 2. Quantum State Storage (varying qubit counts)
//! 3. Grover Search vs Linear Scan Comparison
//! 4. Mixed Workload (classical + quantum)
//! 5. Compression Ratio Analysis
//! 6. Transaction Overhead

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use qdb_rs::prelude::*;
use qdb_rs::storage::MemoryBackend;
use qdb_rs::query::GroverSearch;
use qdb_rs::transaction::TransactionManager;
use std::sync::Arc;

// =============================================================================
// 1. Classical Key-Value Operations
// =============================================================================

fn bench_classical_put(c: &mut Criterion) {
    let mut group = c.benchmark_group("classical_put");
    
    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("entries", size), size, |b, &size| {
            b.iter(|| {
                let db = QDB::new(MemoryBackend::new());
                for i in 0..size {
                    let _ = db.put(format!("key{}", i), format!("value{}", i));
                }
            });
        });
    }
    
    group.finish();
}

fn bench_classical_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("classical_get");
    
    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        
        // Pre-populate database
        let db = QDB::new(MemoryBackend::new());
        for i in 0..*size {
            let _ = db.put(format!("key{}", i), format!("value{}", i));
        }
        
        group.bench_with_input(BenchmarkId::new("entries", size), size, |b, &size| {
            b.iter(|| {
                for i in 0..size {
                    let _ = black_box(db.get(format!("key{}", i)));
                }
            });
        });
    }
    
    group.finish();
}

// =============================================================================
// 2. Quantum State Storage
// =============================================================================

fn bench_quantum_put(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_put");
    
    for qubits in [4, 8, 12, 16, 20].iter() {
        group.bench_with_input(BenchmarkId::new("qubits", qubits), qubits, |b, &qubits| {
            let db = QDB::new(MemoryBackend::new());
            b.iter(|| {
                if let Ok(state) = QftFile::new(qubits) {
                    let _ = db.put_quantum("test_state", state);
                }
            });
        });
    }
    
    group.finish();
}

fn bench_quantum_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_get");
    
    for qubits in [4, 8, 12, 16, 20].iter() {
        let db = QDB::new(MemoryBackend::new());
        if let Ok(state) = QftFile::new(*qubits) {
            let _ = db.put_quantum("test_state", state);
        }
        
        group.bench_with_input(BenchmarkId::new("qubits", qubits), qubits, |b, _| {
            b.iter(|| {
                let _ = black_box(db.get_quantum("test_state"));
            });
        });
    }
    
    group.finish();
}

fn bench_quantum_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_serialization");
    
    for qubits in [4, 8, 12, 16].iter() {
        let state = QftFile::new(*qubits).unwrap();
        
        group.bench_with_input(BenchmarkId::new("to_bytes", qubits), &state, |b, state| {
            b.iter(|| {
                let _ = black_box(state.to_bytes());
            });
        });
        
        let bytes = state.to_bytes().unwrap();
        group.bench_with_input(BenchmarkId::new("from_bytes", qubits), &bytes, |b, bytes| {
            b.iter(|| {
                let _ = black_box(QftFile::from_bytes(bytes));
            });
        });
    }
    
    group.finish();
}

// =============================================================================
// 3. Grover Search vs Linear Scan
// =============================================================================

fn bench_grover_vs_linear(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_comparison");
    
    for size in [256, 1024, 4096, 16384, 65536].iter() {
        let target = size / 2;
        
        // Grover search
        group.bench_with_input(BenchmarkId::new("grover", size), size, |b, &size| {
            b.iter(|| {
                let search = GroverSearch::new(size, |i| i == target);
                let _ = black_box(search.search());
            });
        });
        
        // Linear scan (classical baseline)
        group.bench_with_input(BenchmarkId::new("linear", size), size, |b, &size| {
            b.iter(|| {
                let mut result = None;
                for i in 0..size {
                    if i == target {
                        result = Some(i);
                        break;
                    }
                }
                black_box(result)
            });
        });
    }
    
    group.finish();
}

fn bench_grover_multiple_targets(c: &mut Criterion) {
    let mut group = c.benchmark_group("grover_multiple_targets");
    
    let size = 4096;
    for num_targets in [1, 4, 16, 64].iter() {
        let divisor = size / num_targets;
        
        group.bench_with_input(
            BenchmarkId::new("targets", num_targets),
            num_targets,
            |b, _| {
                b.iter(|| {
                    let search = GroverSearch::new(size, |i| i % divisor == 0);
                    let _ = black_box(search.search());
                });
            },
        );
    }
    
    group.finish();
}

// =============================================================================
// 4. Mixed Workload
// =============================================================================

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");
    
    // 80% classical, 20% quantum
    group.bench_function("80_classical_20_quantum", |b| {
        let db = QDB::new(MemoryBackend::new());
        let mut counter = 0u64;
        
        b.iter(|| {
            counter += 1;
            if counter % 5 == 0 {
                // Quantum operation (20%)
                if let Ok(state) = QftFile::new(4) {
                    let _ = db.put_quantum(format!("q{}", counter), state);
                }
            } else {
                // Classical operation (80%)
                let _ = db.put(format!("c{}", counter), format!("value{}", counter));
            }
        });
    });
    
    // 50% classical, 50% quantum
    group.bench_function("50_classical_50_quantum", |b| {
        let db = QDB::new(MemoryBackend::new());
        let mut counter = 0u64;
        
        b.iter(|| {
            counter += 1;
            if counter % 2 == 0 {
                if let Ok(state) = QftFile::new(4) {
                    let _ = db.put_quantum(format!("q{}", counter), state);
                }
            } else {
                let _ = db.put(format!("c{}", counter), format!("value{}", counter));
            }
        });
    });
    
    group.finish();
}

// =============================================================================
// 5. Transaction Overhead
// =============================================================================

fn bench_transaction_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("transaction_overhead");
    
    // Direct writes (no transaction)
    group.bench_function("direct_writes_100", |b| {
        let db = QDB::new(MemoryBackend::new());
        let mut counter = 0u64;
        
        b.iter(|| {
            for i in 0..100 {
                counter += 1;
                let _ = db.put(format!("key{}", counter), format!("value{}", i));
            }
        });
    });
    
    // Transactional writes
    group.bench_function("transactional_writes_100", |b| {
        let backend = Arc::new(MemoryBackend::new());
        let manager = TransactionManager::new(backend);
        let mut counter = 0u64;
        
        b.iter(|| {
            let tx = manager.begin();
            for i in 0..100 {
                counter += 1;
                let _ = tx.put(format!("key{}", counter), format!("value{}", i));
            }
            let _ = tx.commit();
        });
    });
    
    group.finish();
}

// =============================================================================
// 6. Compression Ratio (informational)
// =============================================================================

fn bench_compression_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");
    
    for qubits in [4, 8, 12, 16].iter() {
        // GHZ state (low entanglement - good compression)
        group.bench_with_input(BenchmarkId::new("ghz_serialize", qubits), qubits, |b, &qubits| {
            let state = ghz_state(qubits).unwrap();
            b.iter(|| {
                let _ = black_box(state.to_bytes());
            });
        });
        
        // Uniform superposition (high entanglement - less compression)
        group.bench_with_input(BenchmarkId::new("uniform_serialize", qubits), qubits, |b, &qubits| {
            let state = uniform_state(qubits).unwrap();
            b.iter(|| {
                let _ = black_box(state.to_bytes());
            });
        });
    }
    
    group.finish();
}

// =============================================================================
// Criterion Groups
// =============================================================================

criterion_group!(
    classical_benches,
    bench_classical_put,
    bench_classical_get
);

criterion_group!(
    quantum_benches,
    bench_quantum_put,
    bench_quantum_get,
    bench_quantum_serialization
);

criterion_group!(
    search_benches,
    bench_grover_vs_linear,
    bench_grover_multiple_targets
);

criterion_group!(
    workload_benches,
    bench_mixed_workload,
    bench_transaction_overhead,
    bench_compression_overhead
);

criterion_main!(classical_benches, quantum_benches, search_benches, workload_benches);
