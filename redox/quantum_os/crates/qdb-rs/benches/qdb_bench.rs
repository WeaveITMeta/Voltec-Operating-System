//! Benchmarks for QDB operations

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use qdb_rs::prelude::*;
use qdb_rs::storage::MemoryBackend;
use qdb_rs::query::GroverSearch;

fn bench_put_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_get");
    
    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("put", size), size, |b, &size| {
            let db = QDB::new(MemoryBackend::new());
            b.iter(|| {
                for i in 0..size {
                    let _ = db.put(format!("key{}", i), format!("value{}", i));
                }
            });
        });
        
        group.bench_with_input(BenchmarkId::new("get", size), size, |b, &size| {
            let db = QDB::new(MemoryBackend::new());
            for i in 0..size {
                let _ = db.put(format!("key{}", i), format!("value{}", i));
            }
            
            b.iter(|| {
                for i in 0..size {
                    let _ = black_box(db.get(format!("key{}", i)));
                }
            });
        });
    }
    
    group.finish();
}

fn bench_quantum_storage(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_storage");
    
    for qubits in [4, 8, 12, 16].iter() {
        group.bench_with_input(BenchmarkId::new("put_quantum", qubits), qubits, |b, &qubits| {
            let db = QDB::new(MemoryBackend::new());
            b.iter(|| {
                if let Ok(state) = QftFile::new(qubits) {
                    let _ = db.put_quantum("test_state", state);
                }
            });
        });
        
        group.bench_with_input(BenchmarkId::new("get_quantum", qubits), qubits, |b, &qubits| {
            let db = QDB::new(MemoryBackend::new());
            if let Ok(state) = QftFile::new(qubits) {
                let _ = db.put_quantum("test_state", state);
            }
            
            b.iter(|| {
                let _ = black_box(db.get_quantum("test_state"));
            });
        });
    }
    
    group.finish();
}

fn bench_grover_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("grover_search");
    
    for size in [64, 256, 1024, 4096].iter() {
        group.bench_with_input(BenchmarkId::new("search", size), size, |b, &size| {
            let target = size / 2;
            b.iter(|| {
                let search = GroverSearch::new(size, |i| i == target);
                let _ = black_box(search.search());
            });
        });
    }
    
    group.finish();
}

criterion_group!(benches, bench_put_get, bench_quantum_storage, bench_grover_search);
criterion_main!(benches);
