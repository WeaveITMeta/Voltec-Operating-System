//! Vector Store Example
//!
//! Demonstrates embedding storage and similarity search with QDB.
//! Run with: cargo run --example vector_store

use qdb_rs::vector::{DistanceMetric, Embedding, VectorEntry, VectorStore, VectorStoreConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== QDB Vector Store Example ===\n");

    // =========================================================================
    // 1. Create Vector Store
    // =========================================================================
    println!("1. Creating Vector Store");

    // Create a vector store for 384-dimensional embeddings (sentence-transformers style)
    let config = VectorStoreConfig::custom(384, DistanceMetric::Cosine);
    let mut store = VectorStore::new(config);

    println!("   Created store with 384 dimensions, cosine similarity\n");

    // =========================================================================
    // 2. Add Documents with Embeddings
    // =========================================================================
    println!("2. Adding Documents");

    // Simulate embeddings for documents (in practice, use an embedding model)
    let documents = vec![
        ("doc1", "Quantum computing uses qubits for computation", create_embedding(384, 1)),
        ("doc2", "Machine learning models learn from data", create_embedding(384, 2)),
        ("doc3", "Quantum machine learning combines both fields", create_embedding(384, 3)),
        ("doc4", "Classical computers use binary bits", create_embedding(384, 4)),
        ("doc5", "Neural networks are inspired by the brain", create_embedding(384, 5)),
        ("doc6", "Quantum entanglement enables teleportation", create_embedding(384, 6)),
        ("doc7", "Deep learning requires large datasets", create_embedding(384, 7)),
        ("doc8", "Superposition allows quantum parallelism", create_embedding(384, 8)),
    ];

    for (id, content, embedding) in documents {
        let entry = VectorEntry::new(id, embedding)
            .with_content(content)
            .with_metadata("source", "example");
        store.put(entry)?;
        println!("   Added: {} - \"{}\"", id, content);
    }

    println!("\n   Total vectors: {}\n", store.len());

    // =========================================================================
    // 3. Similarity Search
    // =========================================================================
    println!("3. Similarity Search");

    // Create a query embedding (simulating "quantum computing" query)
    let query = create_embedding(384, 1); // Similar to doc1

    let results = store.search(&query, 3);

    println!("   Query: 'quantum computing' (simulated)");
    println!("   Top 3 results:");
    for (i, result) in results.iter().enumerate() {
        let content = result
            .entry
            .as_ref()
            .and_then(|e| e.content.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("N/A");
        println!(
            "   {}. {} (score: {:.4}) - \"{}\"",
            i + 1,
            result.id,
            result.score,
            content
        );
    }

    // =========================================================================
    // 4. Namespace Filtering
    // =========================================================================
    println!("\n4. Namespace Filtering");

    // Add documents with different namespaces
    let mut namespaced_store = VectorStore::with_dim(128);

    namespaced_store.put(
        VectorEntry::new("physics-1", create_embedding(128, 10))
            .with_namespace("physics")
            .with_content("E=mc² is Einstein's famous equation"),
    )?;

    namespaced_store.put(
        VectorEntry::new("physics-2", create_embedding(128, 11))
            .with_namespace("physics")
            .with_content("Quantum mechanics describes atomic behavior"),
    )?;

    namespaced_store.put(
        VectorEntry::new("biology-1", create_embedding(128, 20))
            .with_namespace("biology")
            .with_content("DNA carries genetic information"),
    )?;

    namespaced_store.put(
        VectorEntry::new("biology-2", create_embedding(128, 21))
            .with_namespace("biology")
            .with_content("Cells are the basic unit of life"),
    )?;

    // Search only in physics namespace
    let query = create_embedding(128, 10);
    let physics_results = namespaced_store.search_namespace(&query, 5, "physics");

    println!("   Search in 'physics' namespace:");
    for result in &physics_results {
        let content = result
            .entry
            .as_ref()
            .and_then(|e| e.content.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("N/A");
        println!("   - {} (score: {:.4}) - \"{}\"", result.id, result.score, content);
    }

    // =========================================================================
    // 5. Distance Metrics Comparison
    // =========================================================================
    println!("\n5. Distance Metrics Comparison");

    let a = Embedding::new(vec![1.0, 0.0, 0.0]);
    let b = Embedding::new(vec![0.707, 0.707, 0.0]);
    let c = Embedding::new(vec![0.0, 1.0, 0.0]);

    println!("   Vectors:");
    println!("   a = [1, 0, 0]");
    println!("   b = [0.707, 0.707, 0]");
    println!("   c = [0, 1, 0]");
    println!();

    for metric in [
        DistanceMetric::Cosine,
        DistanceMetric::Euclidean,
        DistanceMetric::DotProduct,
        DistanceMetric::Manhattan,
    ] {
        println!("   {:?}:", metric);
        println!("     a↔b: dist={:.4}, sim={:.4}", metric.distance(&a, &b), metric.similarity(&a, &b));
        println!("     a↔c: dist={:.4}, sim={:.4}", metric.distance(&a, &c), metric.similarity(&a, &c));
    }

    // =========================================================================
    // 6. Metadata Filtering
    // =========================================================================
    println!("\n6. Metadata Filtering");

    let mut tagged_store = VectorStore::with_dim(64);

    tagged_store.put(
        VectorEntry::new("article-1", create_embedding(64, 1))
            .with_metadata("category", "tech")
            .with_metadata("year", "2024"),
    )?;

    tagged_store.put(
        VectorEntry::new("article-2", create_embedding(64, 2))
            .with_metadata("category", "science")
            .with_metadata("year", "2024"),
    )?;

    tagged_store.put(
        VectorEntry::new("article-3", create_embedding(64, 3))
            .with_metadata("category", "tech")
            .with_metadata("year", "2023"),
    )?;

    // Filter by category = "tech"
    let query = create_embedding(64, 1);
    let tech_results = tagged_store.search_with_filter(&query, 10, |entry| {
        entry
            .metadata
            .get("category")
            .map(|v| matches!(v, qdb_rs::Value::String(s) if s == "tech"))
            .unwrap_or(false)
    });

    println!("   Filter: category = 'tech'");
    for result in &tech_results {
        println!("   - {}", result.id);
    }

    // =========================================================================
    // 7. Batch Operations
    // =========================================================================
    println!("\n7. Batch Operations");

    let mut batch_store = VectorStore::with_dim(32);

    let batch: Vec<VectorEntry> = (0..100)
        .map(|i| VectorEntry::new(format!("vec-{}", i), create_embedding(32, i)))
        .collect();

    let count = batch_store.put_batch(batch)?;
    println!("   Inserted {} vectors in batch", count);
    println!("   Store size: {}", batch_store.len());

    // Search in large store
    let query = create_embedding(32, 50);
    let results = batch_store.search(&query, 5);
    println!("   Top 5 nearest to vec-50:");
    for result in &results {
        println!("   - {} (score: {:.4})", result.id, result.score);
    }

    println!("\n=== Vector Store Example Complete ===");
    Ok(())
}

/// Create a deterministic pseudo-random embedding for testing
fn create_embedding(dim: usize, seed: u32) -> Embedding {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let data: Vec<f32> = (0..dim)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            (seed, i).hash(&mut hasher);
            let h = hasher.finish();
            ((h as f64) / (u64::MAX as f64) * 2.0 - 1.0) as f32
        })
        .collect();

    Embedding::new(data)
}
