//! Vector Store for QDB - Quantum-Optimized Embedding Storage
//!
//! # Table of Contents
//! 1. Vector Types - Embedding, VectorEntry, QuantumEmbedding
//! 2. Distance Metrics - Cosine, Euclidean, Dot Product, Manhattan, Fidelity
//! 3. Vector Index - HNSW-inspired approximate nearest neighbor
//! 4. Vector Store API - put, search, upsert, batch operations
//! 5. Quantum Extensions - Amplitude encoding, fidelity distance
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Vector Store API                         │
//! │  put() │ upsert() │ search() │ batch_put() │ delete()      │
//! ├─────────────────────────────────────────────────────────────┤
//! │                    Vector Index (HNSW)                      │
//! │  - Multi-layer graph for O(log N) search                   │
//! │  - Configurable M (connections) and ef (search width)      │
//! │  - Product quantization for memory efficiency              │
//! ├─────────────────────────────────────────────────────────────┤
//! │                    Distance Metrics                         │
//! │  Cosine │ Euclidean │ DotProduct │ Manhattan │ Fidelity    │
//! ├─────────────────────────────────────────────────────────────┤
//! │                    Quantum Extensions                       │
//! │  - Amplitude encoding (classical → quantum)                │
//! │  - Fidelity-based similarity (quantum states)              │
//! │  - QFT integration for quantum state embeddings            │
//! ├─────────────────────────────────────────────────────────────┤
//! │                    QDB Storage Backend                      │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Quantum-Specific Features
//!
//! Unlike classical vector stores, QDB's vector store supports:
//! - **Amplitude Encoding**: Convert classical vectors to quantum state amplitudes
//! - **Fidelity Distance**: Use quantum fidelity |⟨ψ|φ⟩|² as similarity metric
//! - **QFT Integration**: Store quantum states directly as embeddings
//! - **Entanglement-Aware Search**: Group related embeddings by entanglement

use crate::entry::{Entry, EntryType, Key, Value};
use crate::error::{QdbError, Result};
use num_complex::Complex64;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

// =============================================================================
// 1. Vector Types
// =============================================================================

/// A dense vector embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// The vector data
    pub data: Vec<f32>,
    /// Dimensionality
    pub dim: usize,
    /// L2 norm (cached for cosine similarity)
    #[serde(skip)]
    norm: Option<f32>,
}

impl Embedding {
    /// Create a new embedding from a vector
    pub fn new(data: Vec<f32>) -> Self {
        let dim = data.len();
        Self {
            data,
            dim,
            norm: None,
        }
    }

    /// Create a zero embedding of given dimension
    pub fn zeros(dim: usize) -> Self {
        Self::new(vec![0.0; dim])
    }

    /// Create a random embedding (for testing)
    pub fn random(dim: usize) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::{SystemTime, UNIX_EPOCH};

        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let mut hasher = DefaultHasher::new();
        let data: Vec<f32> = (0..dim)
            .map(|i| {
                (seed + i as u64).hash(&mut hasher);
                let h = hasher.finish();
                ((h as f64) / (u64::MAX as f64) * 2.0 - 1.0) as f32
            })
            .collect();

        Self::new(data)
    }

    /// Get the L2 norm of the embedding
    pub fn norm(&mut self) -> f32 {
        if let Some(n) = self.norm {
            return n;
        }
        let n = self.data.iter().map(|x| x * x).sum::<f32>().sqrt();
        self.norm = Some(n);
        n
    }

    /// Normalize the embedding to unit length
    pub fn normalize(&mut self) {
        let n = self.norm();
        if n > 1e-10 {
            for x in &mut self.data {
                *x /= n;
            }
            self.norm = Some(1.0);
        }
    }

    /// Get a normalized copy
    pub fn normalized(&self) -> Self {
        let mut copy = self.clone();
        copy.normalize();
        copy
    }

    /// Dot product with another embedding
    pub fn dot(&self, other: &Embedding) -> f32 {
        self.data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a * b)
            .sum()
    }

    /// Add another embedding (element-wise)
    pub fn add(&self, other: &Embedding) -> Self {
        let data: Vec<f32> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a + b)
            .collect();
        Self::new(data)
    }

    /// Scale by a scalar
    pub fn scale(&self, s: f32) -> Self {
        let data: Vec<f32> = self.data.iter().map(|x| x * s).collect();
        Self::new(data)
    }
}

impl From<Vec<f32>> for Embedding {
    fn from(data: Vec<f32>) -> Self {
        Self::new(data)
    }
}

impl From<&[f32]> for Embedding {
    fn from(data: &[f32]) -> Self {
        Self::new(data.to_vec())
    }
}

// =============================================================================
// Quantum Embedding Extensions
// =============================================================================

impl Embedding {
    /// Convert to amplitude encoding (normalize to unit vector for quantum state)
    /// This prepares the embedding for use as quantum state amplitudes
    pub fn to_amplitude_encoding(&self) -> Self {
        self.normalized()
    }

    /// Create embedding from quantum state amplitudes (complex → real)
    /// Takes the real parts of complex amplitudes
    pub fn from_complex_amplitudes(amplitudes: &[Complex64]) -> Self {
        let data: Vec<f32> = amplitudes.iter().map(|c| c.re as f32).collect();
        Self::new(data)
    }

    /// Create embedding from quantum state probability distribution
    /// Each element is sqrt(probability) to preserve amplitude structure
    pub fn from_probabilities(probs: &[f64]) -> Self {
        let data: Vec<f32> = probs.iter().map(|p| p.sqrt() as f32).collect();
        Self::new(data)
    }

    /// Compute quantum fidelity with another embedding
    /// Treats both as normalized quantum state amplitudes
    /// Returns |⟨ψ|φ⟩|² ∈ [0, 1]
    pub fn fidelity(&self, other: &Embedding) -> f32 {
        let inner_product: f32 = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a * b)
            .sum();
        inner_product.powi(2)
    }

    /// Compute trace distance (quantum distinguishability)
    /// Returns value in [0, 1], where 0 = identical, 1 = orthogonal
    pub fn trace_distance(&self, other: &Embedding) -> f32 {
        // For pure states: D(ρ,σ) = sqrt(1 - F(ρ,σ))
        (1.0 - self.fidelity(other)).sqrt()
    }

    /// Quantize embedding to binary (for Hamming distance)
    pub fn quantize_binary(&self) -> Self {
        let data: Vec<f32> = self.data.iter().map(|x| if *x > 0.0 { 1.0 } else { -1.0 }).collect();
        Self::new(data)
    }

    /// Product quantization: split into subvectors
    /// Returns (num_subvectors, subvector_dim, subvector_data)
    pub fn product_quantize(&self, num_subvectors: usize) -> Vec<Vec<f32>> {
        let subvec_dim = self.dim / num_subvectors;
        (0..num_subvectors)
            .map(|i| {
                let start = i * subvec_dim;
                let end = ((i + 1) * subvec_dim).min(self.dim);
                self.data[start..end].to_vec()
            })
            .collect()
    }
}

/// A vector entry in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    /// Unique identifier
    pub id: String,
    /// The embedding vector
    pub embedding: Embedding,
    /// Associated metadata
    pub metadata: HashMap<String, Value>,
    /// Optional text content (for RAG applications)
    pub content: Option<String>,
    /// Namespace for multi-tenant isolation
    pub namespace: Option<String>,
}

impl VectorEntry {
    /// Create a new vector entry
    pub fn new(id: impl Into<String>, embedding: Embedding) -> Self {
        Self {
            id: id.into(),
            embedding,
            metadata: HashMap::new(),
            content: None,
            namespace: None,
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set content
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Set namespace
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Convert to a QDB Entry
    pub fn to_entry(&self) -> Entry {
        let key = Key::string(&self.id);
        let value = Value::Json(serde_json::to_value(self).unwrap_or_default());
        let mut entry = Entry::new(key, value);
        entry.entry_type = EntryType::Document;
        entry.metadata.insert("type".to_string(), "vector".to_string());
        entry
            .metadata
            .insert("dim".to_string(), self.embedding.dim.to_string());
        entry
    }
}

// =============================================================================
// 2. Distance Metrics
// =============================================================================

/// Distance metric for similarity search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DistanceMetric {
    /// Cosine similarity (1 - cosine_sim for distance)
    #[default]
    Cosine,
    /// Euclidean (L2) distance
    Euclidean,
    /// Dot product (negative for distance, higher = more similar)
    DotProduct,
    /// Manhattan (L1) distance
    Manhattan,
    /// Quantum Fidelity: |⟨ψ|φ⟩|² - treats vectors as quantum state amplitudes
    /// Best for quantum state embeddings where vectors represent probability amplitudes
    Fidelity,
    /// Hamming distance for binary/quantized vectors
    Hamming,
}

impl DistanceMetric {
    /// Compute distance between two embeddings
    pub fn distance(&self, a: &Embedding, b: &Embedding) -> f32 {
        match self {
            DistanceMetric::Cosine => {
                let dot = a.dot(b);
                let norm_a = a.data.iter().map(|x| x * x).sum::<f32>().sqrt();
                let norm_b = b.data.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm_a < 1e-10 || norm_b < 1e-10 {
                    1.0
                } else {
                    1.0 - (dot / (norm_a * norm_b))
                }
            }
            DistanceMetric::Euclidean => a
                .data
                .iter()
                .zip(b.data.iter())
                .map(|(x, y)| (x - y).powi(2))
                .sum::<f32>()
                .sqrt(),
            DistanceMetric::DotProduct => -a.dot(b), // Negative so lower = more similar
            DistanceMetric::Manhattan => a
                .data
                .iter()
                .zip(b.data.iter())
                .map(|(x, y)| (x - y).abs())
                .sum(),
            DistanceMetric::Fidelity => {
                // Quantum fidelity: 1 - |⟨ψ|φ⟩|²
                // Treat vectors as probability amplitudes (should be normalized)
                let inner_product: f32 = a.data.iter().zip(b.data.iter()).map(|(x, y)| x * y).sum();
                1.0 - inner_product.powi(2)
            }
            DistanceMetric::Hamming => {
                // Hamming distance for binary vectors (threshold at 0)
                a.data
                    .iter()
                    .zip(b.data.iter())
                    .filter(|(x, y)| (**x > 0.0) != (**y > 0.0))
                    .count() as f32
            }
        }
    }

    /// Compute similarity (inverse of distance, higher = more similar)
    pub fn similarity(&self, a: &Embedding, b: &Embedding) -> f32 {
        match self {
            DistanceMetric::Cosine => {
                let dot = a.dot(b);
                let norm_a = a.data.iter().map(|x| x * x).sum::<f32>().sqrt();
                let norm_b = b.data.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm_a < 1e-10 || norm_b < 1e-10 {
                    0.0
                } else {
                    dot / (norm_a * norm_b)
                }
            }
            DistanceMetric::DotProduct => a.dot(b),
            DistanceMetric::Euclidean => 1.0 / (1.0 + self.distance(a, b)),
            DistanceMetric::Manhattan => 1.0 / (1.0 + self.distance(a, b)),
            DistanceMetric::Fidelity => {
                // Quantum fidelity: |⟨ψ|φ⟩|²
                let inner_product: f32 = a.data.iter().zip(b.data.iter()).map(|(x, y)| x * y).sum();
                inner_product.powi(2)
            }
            DistanceMetric::Hamming => {
                // Similarity = 1 - normalized hamming distance
                let matches = a
                    .data
                    .iter()
                    .zip(b.data.iter())
                    .filter(|(x, y)| (**x > 0.0) == (**y > 0.0))
                    .count();
                matches as f32 / a.dim.max(1) as f32
            }
        }
    }
}

// =============================================================================
// SIMD-Optimized Distance Functions
// =============================================================================

/// Optimized cosine distance using loop unrolling
#[inline]
pub fn cosine_distance_fast(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    // Process 4 elements at a time
    let chunks = a.len() / 4;
    for i in 0..chunks {
        let idx = i * 4;
        dot += a[idx] * b[idx] + a[idx + 1] * b[idx + 1] + a[idx + 2] * b[idx + 2] + a[idx + 3] * b[idx + 3];
        norm_a += a[idx] * a[idx] + a[idx + 1] * a[idx + 1] + a[idx + 2] * a[idx + 2] + a[idx + 3] * a[idx + 3];
        norm_b += b[idx] * b[idx] + b[idx + 1] * b[idx + 1] + b[idx + 2] * b[idx + 2] + b[idx + 3] * b[idx + 3];
    }

    // Handle remainder
    for i in (chunks * 4)..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denom = (norm_a * norm_b).sqrt();
    if denom < 1e-10 {
        1.0
    } else {
        1.0 - (dot / denom)
    }
}

/// Optimized euclidean distance squared (avoids sqrt for comparisons)
#[inline]
pub fn euclidean_distance_squared(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    let chunks = a.len() / 4;
    
    for i in 0..chunks {
        let idx = i * 4;
        let d0 = a[idx] - b[idx];
        let d1 = a[idx + 1] - b[idx + 1];
        let d2 = a[idx + 2] - b[idx + 2];
        let d3 = a[idx + 3] - b[idx + 3];
        sum += d0 * d0 + d1 * d1 + d2 * d2 + d3 * d3;
    }

    for i in (chunks * 4)..a.len() {
        let d = a[i] - b[i];
        sum += d * d;
    }

    sum
}

// =============================================================================
// 3. Vector Index (HNSW-inspired)
// =============================================================================

/// Configuration for the vector index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndexConfig {
    /// Maximum number of connections per node per layer
    pub m: usize,
    /// Maximum number of connections for layer 0
    pub m0: usize,
    /// Size of dynamic candidate list during construction
    pub ef_construction: usize,
    /// Size of dynamic candidate list during search
    pub ef_search: usize,
    /// Distance metric to use
    pub metric: DistanceMetric,
    /// Maximum number of layers
    pub max_layers: usize,
}

impl Default for VectorIndexConfig {
    fn default() -> Self {
        Self {
            m: 16,
            m0: 32,
            ef_construction: 200,
            ef_search: 50,
            metric: DistanceMetric::Cosine,
            max_layers: 16,
        }
    }
}

/// A node in the HNSW graph
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HnswNode {
    id: String,
    embedding: Embedding,
    /// Connections per layer: layer -> list of neighbor IDs
    connections: Vec<Vec<String>>,
}

/// Search result with distance
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Vector entry ID
    pub id: String,
    /// Distance from query
    pub distance: f32,
    /// Similarity score (1 - distance for cosine)
    pub score: f32,
    /// The vector entry (if requested)
    pub entry: Option<VectorEntry>,
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for SearchResult {}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchResult {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for min-heap (lower distance = higher priority)
        other
            .distance
            .partial_cmp(&self.distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// HNSW-inspired vector index for approximate nearest neighbor search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndex {
    /// Configuration
    config: VectorIndexConfig,
    /// All nodes in the index
    nodes: HashMap<String, HnswNode>,
    /// Entry point (top layer node)
    entry_point: Option<String>,
    /// Current maximum layer
    max_layer: usize,
    /// Dimensionality of vectors
    dim: usize,
}

impl VectorIndex {
    /// Create a new vector index
    pub fn new(dim: usize) -> Self {
        Self::with_config(dim, VectorIndexConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(dim: usize, config: VectorIndexConfig) -> Self {
        Self {
            config,
            nodes: HashMap::new(),
            entry_point: None,
            max_layer: 0,
            dim,
        }
    }

    /// Get the number of vectors in the index
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Insert a vector into the index
    pub fn insert(&mut self, id: impl Into<String>, embedding: Embedding) -> Result<()> {
        let id = id.into();

        if embedding.dim != self.dim {
            return Err(QdbError::InvalidInput(format!(
                "Embedding dimension {} does not match index dimension {}",
                embedding.dim, self.dim
            )));
        }

        // Determine layer for this node
        let layer = self.random_layer();

        // Create node with empty connections
        let mut node = HnswNode {
            id: id.clone(),
            embedding,
            connections: vec![Vec::new(); layer + 1],
        };

        if self.entry_point.is_none() {
            // First node
            self.entry_point = Some(id.clone());
            self.max_layer = layer;
            self.nodes.insert(id, node);
            return Ok(());
        }

        // Find entry point and search down
        let mut current = self.entry_point.clone().unwrap();

        // Search from top layer to layer + 1
        for l in (layer + 1..=self.max_layer).rev() {
            current = self.search_layer_greedy(&node.embedding, &current, l);
        }

        // Insert at each layer from layer down to 0
        for l in (0..=layer.min(self.max_layer)).rev() {
            let neighbors = self.search_layer(&node.embedding, &current, self.config.ef_construction, l);

            // Select M best neighbors
            let m = if l == 0 { self.config.m0 } else { self.config.m };
            let selected: Vec<String> = neighbors.into_iter().take(m).map(|(id, _)| id).collect();

            // Add bidirectional connections
            for neighbor_id in &selected {
                if let Some(neighbor) = self.nodes.get_mut(neighbor_id) {
                    if neighbor.connections.len() > l {
                        neighbor.connections[l].push(id.clone());
                        // Prune if too many connections
                        if neighbor.connections[l].len() > m {
                            self.prune_connections(neighbor_id, l, m);
                        }
                    }
                }
            }

            node.connections[l] = selected;

            if !node.connections[l].is_empty() {
                current = node.connections[l][0].clone();
            }
        }

        // Update entry point if new node is at higher layer
        if layer > self.max_layer {
            self.entry_point = Some(id.clone());
            self.max_layer = layer;
        }

        self.nodes.insert(id, node);
        Ok(())
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &Embedding, k: usize) -> Vec<SearchResult> {
        if self.entry_point.is_none() || k == 0 {
            return Vec::new();
        }

        let mut current = self.entry_point.clone().unwrap();

        // Greedy search from top layer to layer 1
        for l in (1..=self.max_layer).rev() {
            current = self.search_layer_greedy(query, &current, l);
        }

        // Search layer 0 with ef_search candidates
        let candidates = self.search_layer(query, &current, self.config.ef_search, 0);

        // Return top k
        candidates
            .into_iter()
            .take(k)
            .map(|(id, dist)| SearchResult {
                id: id.clone(),
                distance: dist,
                score: 1.0 - dist.min(1.0),
                entry: None,
            })
            .collect()
    }

    /// Search with filter function
    pub fn search_with_filter<F>(&self, query: &Embedding, k: usize, filter: F) -> Vec<SearchResult>
    where
        F: Fn(&str) -> bool,
    {
        // Get more candidates to account for filtering
        let candidates = self.search(query, k * 10);

        candidates
            .into_iter()
            .filter(|r| filter(&r.id))
            .take(k)
            .collect()
    }

    /// Delete a vector from the index
    pub fn delete(&mut self, id: &str) -> bool {
        if let Some(node) = self.nodes.remove(id) {
            // Remove connections from neighbors
            for (layer, neighbors) in node.connections.iter().enumerate() {
                for neighbor_id in neighbors {
                    if let Some(neighbor) = self.nodes.get_mut(neighbor_id) {
                        if neighbor.connections.len() > layer {
                            neighbor.connections[layer].retain(|x| x != id);
                        }
                    }
                }
            }

            // Update entry point if needed
            if self.entry_point.as_ref() == Some(&id.to_string()) {
                self.entry_point = self.nodes.keys().next().cloned();
                if let Some(ep) = &self.entry_point {
                    if let Some(node) = self.nodes.get(ep) {
                        self.max_layer = node.connections.len().saturating_sub(1);
                    }
                }
            }

            true
        } else {
            false
        }
    }

    // Helper: Generate random layer using exponential distribution
    fn random_layer(&self) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::{SystemTime, UNIX_EPOCH};

        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        let r = (hasher.finish() as f64) / (u64::MAX as f64);

        let ml = 1.0 / (self.config.m as f64).ln();
        ((-r.ln() * ml) as usize).min(self.config.max_layers - 1)
    }

    // Helper: Greedy search in a single layer
    fn search_layer_greedy(&self, query: &Embedding, start: &str, layer: usize) -> String {
        let mut current = start.to_string();
        let mut current_dist = self.distance_to(query, &current);

        loop {
            let mut changed = false;

            if let Some(node) = self.nodes.get(&current) {
                if node.connections.len() > layer {
                    for neighbor_id in &node.connections[layer] {
                        let dist = self.distance_to(query, neighbor_id);
                        if dist < current_dist {
                            current = neighbor_id.clone();
                            current_dist = dist;
                            changed = true;
                        }
                    }
                }
            }

            if !changed {
                break;
            }
        }

        current
    }

    // Helper: Search layer with ef candidates
    fn search_layer(
        &self,
        query: &Embedding,
        start: &str,
        ef: usize,
        layer: usize,
    ) -> Vec<(String, f32)> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut candidates: BinaryHeap<(OrderedFloat, String)> = BinaryHeap::new();
        let mut results: BinaryHeap<(OrderedFloat, String)> = BinaryHeap::new();

        let start_dist = self.distance_to(query, start);
        candidates.push((OrderedFloat(-start_dist), start.to_string()));
        results.push((OrderedFloat(start_dist), start.to_string()));
        visited.insert(start.to_string());

        while let Some((OrderedFloat(neg_dist), current)) = candidates.pop() {
            let current_dist = -neg_dist;

            // Check if we can stop
            if let Some((OrderedFloat(worst_dist), _)) = results.peek() {
                if current_dist > *worst_dist && results.len() >= ef {
                    break;
                }
            }

            if let Some(node) = self.nodes.get(&current) {
                if node.connections.len() > layer {
                    for neighbor_id in &node.connections[layer] {
                        if visited.contains(neighbor_id) {
                            continue;
                        }
                        visited.insert(neighbor_id.clone());

                        let dist = self.distance_to(query, neighbor_id);

                        let should_add = if results.len() < ef {
                            true
                        } else if let Some((OrderedFloat(worst_dist), _)) = results.peek() {
                            dist < *worst_dist
                        } else {
                            true
                        };

                        if should_add {
                            candidates.push((OrderedFloat(-dist), neighbor_id.clone()));
                            results.push((OrderedFloat(dist), neighbor_id.clone()));

                            if results.len() > ef {
                                results.pop();
                            }
                        }
                    }
                }
            }
        }

        // Convert to sorted vec
        let mut result_vec: Vec<(String, f32)> = results
            .into_iter()
            .map(|(OrderedFloat(d), id)| (id, d))
            .collect();
        result_vec.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        result_vec
    }

    // Helper: Compute distance to a node
    fn distance_to(&self, query: &Embedding, id: &str) -> f32 {
        if let Some(node) = self.nodes.get(id) {
            self.config.metric.distance(query, &node.embedding)
        } else {
            f32::MAX
        }
    }

    // Helper: Prune connections to keep only M best
    fn prune_connections(&mut self, id: &str, layer: usize, m: usize) {
        if let Some(node) = self.nodes.get(id) {
            let embedding = node.embedding.clone();
            let connections = node.connections[layer].clone();

            // Sort by distance
            let mut scored: Vec<(String, f32)> = connections
                .into_iter()
                .filter_map(|neighbor_id| {
                    self.nodes
                        .get(&neighbor_id)
                        .map(|n| (neighbor_id, self.config.metric.distance(&embedding, &n.embedding)))
                })
                .collect();

            scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

            if let Some(node) = self.nodes.get_mut(id) {
                node.connections[layer] = scored.into_iter().take(m).map(|(id, _)| id).collect();
            }
        }
    }
}

// Helper for ordered floats in BinaryHeap
#[derive(Debug, Clone, Copy, PartialEq)]
struct OrderedFloat(f32);

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}

// =============================================================================
// 4. Vector Store API
// =============================================================================

/// Vector store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStoreConfig {
    /// Embedding dimension
    pub dim: usize,
    /// Distance metric
    pub metric: DistanceMetric,
    /// Index configuration
    pub index_config: VectorIndexConfig,
}

impl VectorStoreConfig {
    /// Create config for OpenAI embeddings (1536 dim)
    pub fn openai() -> Self {
        Self {
            dim: 1536,
            metric: DistanceMetric::Cosine,
            index_config: VectorIndexConfig::default(),
        }
    }

    /// Create config for sentence-transformers (384 dim)
    pub fn sentence_transformers() -> Self {
        Self {
            dim: 384,
            metric: DistanceMetric::Cosine,
            index_config: VectorIndexConfig::default(),
        }
    }

    /// Create config for custom dimension
    pub fn custom(dim: usize, metric: DistanceMetric) -> Self {
        Self {
            dim,
            metric,
            index_config: VectorIndexConfig {
                metric,
                ..Default::default()
            },
        }
    }
}

/// Vector store for managing embeddings
#[derive(Debug)]
pub struct VectorStore {
    /// Configuration
    config: VectorStoreConfig,
    /// The vector index
    index: VectorIndex,
    /// Stored entries (for retrieval)
    entries: HashMap<String, VectorEntry>,
}

impl VectorStore {
    /// Create a new vector store
    pub fn new(config: VectorStoreConfig) -> Self {
        let index = VectorIndex::with_config(config.dim, config.index_config.clone());
        Self {
            config,
            index,
            entries: HashMap::new(),
        }
    }

    /// Create with default config for given dimension
    pub fn with_dim(dim: usize) -> Self {
        Self::new(VectorStoreConfig::custom(dim, DistanceMetric::Cosine))
    }

    /// Get the number of vectors
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insert a vector
    pub fn put(&mut self, entry: VectorEntry) -> Result<()> {
        let id = entry.id.clone();
        self.index.insert(&id, entry.embedding.clone())?;
        self.entries.insert(id, entry);
        Ok(())
    }

    /// Insert a vector with just ID and embedding
    pub fn put_embedding(&mut self, id: impl Into<String>, embedding: impl Into<Embedding>) -> Result<()> {
        let entry = VectorEntry::new(id, embedding.into());
        self.put(entry)
    }

    /// Batch insert
    pub fn put_batch(&mut self, entries: Vec<VectorEntry>) -> Result<usize> {
        let mut count = 0;
        for entry in entries {
            self.put(entry)?;
            count += 1;
        }
        Ok(count)
    }

    /// Search for similar vectors
    pub fn search(&self, query: &Embedding, k: usize) -> Vec<SearchResult> {
        let mut results = self.index.search(query, k);

        // Attach entries
        for result in &mut results {
            result.entry = self.entries.get(&result.id).cloned();
        }

        results
    }

    /// Search with a filter on metadata
    pub fn search_with_filter<F>(&self, query: &Embedding, k: usize, filter: F) -> Vec<SearchResult>
    where
        F: Fn(&VectorEntry) -> bool,
    {
        let mut results = self.index.search(query, k * 10);

        // Filter and attach entries
        results
            .into_iter()
            .filter_map(|mut r| {
                if let Some(entry) = self.entries.get(&r.id) {
                    if filter(entry) {
                        r.entry = Some(entry.clone());
                        return Some(r);
                    }
                }
                None
            })
            .take(k)
            .collect()
    }

    /// Search by namespace
    pub fn search_namespace(&self, query: &Embedding, k: usize, namespace: &str) -> Vec<SearchResult> {
        self.search_with_filter(query, k, |e| {
            e.namespace.as_ref().map(|n| n == namespace).unwrap_or(false)
        })
    }

    /// Get a vector by ID
    pub fn get(&self, id: &str) -> Option<&VectorEntry> {
        self.entries.get(id)
    }

    /// Delete a vector
    pub fn delete(&mut self, id: &str) -> bool {
        self.index.delete(id);
        self.entries.remove(id).is_some()
    }

    /// Get all IDs
    pub fn ids(&self) -> Vec<&String> {
        self.entries.keys().collect()
    }

    /// Compute similarity between two vectors by ID
    pub fn similarity(&self, id1: &str, id2: &str) -> Option<f32> {
        let e1 = self.entries.get(id1)?;
        let e2 = self.entries.get(id2)?;
        Some(self.config.metric.similarity(&e1.embedding, &e2.embedding))
    }

    /// Upsert: insert or update a vector
    pub fn upsert(&mut self, entry: VectorEntry) -> Result<bool> {
        let id = entry.id.clone();
        let existed = self.entries.contains_key(&id);
        
        if existed {
            self.index.delete(&id);
        }
        
        self.index.insert(&id, entry.embedding.clone())?;
        self.entries.insert(id, entry);
        Ok(existed)
    }

    /// Clear all vectors from the store
    pub fn clear(&mut self) {
        self.entries.clear();
        self.index = VectorIndex::with_config(self.config.dim, self.config.index_config.clone());
    }

    /// Get store statistics
    pub fn stats(&self) -> VectorStoreStats {
        let total_vectors = self.entries.len();
        let total_dimensions = self.config.dim;
        let memory_estimate = total_vectors * total_dimensions * 4; // 4 bytes per f32
        
        let namespaces: HashSet<_> = self.entries
            .values()
            .filter_map(|e| e.namespace.as_ref())
            .collect();

        VectorStoreStats {
            total_vectors,
            dimensions: total_dimensions,
            memory_bytes: memory_estimate,
            index_layers: self.index.max_layer,
            namespaces: namespaces.len(),
            metric: self.config.metric,
        }
    }

    /// Find vectors similar to a given ID
    pub fn find_similar(&self, id: &str, k: usize) -> Vec<SearchResult> {
        if let Some(entry) = self.entries.get(id) {
            let mut results = self.search(&entry.embedding, k + 1);
            results.retain(|r| r.id != id);
            results.truncate(k);
            results
        } else {
            Vec::new()
        }
    }

    /// Check if a vector exists
    pub fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    /// Get the configuration
    pub fn config(&self) -> &VectorStoreConfig {
        &self.config
    }

    /// Update metadata for an existing vector
    pub fn update_metadata(&mut self, id: &str, key: impl Into<String>, value: impl Into<Value>) -> bool {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.metadata.insert(key.into(), value.into());
            true
        } else {
            false
        }
    }

    /// Get all vectors in a namespace
    pub fn get_namespace(&self, namespace: &str) -> Vec<&VectorEntry> {
        self.entries
            .values()
            .filter(|e| e.namespace.as_ref().map(|n| n == namespace).unwrap_or(false))
            .collect()
    }

    /// Count vectors in a namespace
    pub fn count_namespace(&self, namespace: &str) -> usize {
        self.entries
            .values()
            .filter(|e| e.namespace.as_ref().map(|n| n == namespace).unwrap_or(false))
            .count()
    }

    /// Delete all vectors in a namespace
    pub fn delete_namespace(&mut self, namespace: &str) -> usize {
        let ids_to_delete: Vec<String> = self.entries
            .iter()
            .filter(|(_, e)| e.namespace.as_ref().map(|n| n == namespace).unwrap_or(false))
            .map(|(id, _)| id.clone())
            .collect();
        
        let count = ids_to_delete.len();
        for id in ids_to_delete {
            self.delete(&id);
        }
        count
    }

    /// Compute centroid of all vectors (or a subset by namespace)
    pub fn centroid(&self, namespace: Option<&str>) -> Option<Embedding> {
        let vectors: Vec<&Embedding> = self.entries
            .values()
            .filter(|e| {
                namespace.map(|ns| e.namespace.as_ref().map(|n| n == ns).unwrap_or(false)).unwrap_or(true)
            })
            .map(|e| &e.embedding)
            .collect();

        if vectors.is_empty() {
            return None;
        }

        let dim = vectors[0].dim;
        let mut centroid = vec![0.0f32; dim];
        
        for v in &vectors {
            for (i, val) in v.data.iter().enumerate() {
                centroid[i] += val;
            }
        }

        let n = vectors.len() as f32;
        for val in &mut centroid {
            *val /= n;
        }

        Some(Embedding::new(centroid))
    }
}

/// Statistics about the vector store
#[derive(Debug, Clone)]
pub struct VectorStoreStats {
    /// Total number of vectors
    pub total_vectors: usize,
    /// Embedding dimensions
    pub dimensions: usize,
    /// Estimated memory usage in bytes
    pub memory_bytes: usize,
    /// Number of HNSW index layers
    pub index_layers: usize,
    /// Number of distinct namespaces
    pub namespaces: usize,
    /// Distance metric in use
    pub metric: DistanceMetric,
}

impl std::fmt::Display for VectorStoreStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VectorStore: {} vectors, {} dims, {:.2} MB, {} layers, {} namespaces, {:?}",
            self.total_vectors,
            self.dimensions,
            self.memory_bytes as f64 / (1024.0 * 1024.0),
            self.index_layers,
            self.namespaces,
            self.metric
        )
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_creation() {
        let emb = Embedding::new(vec![1.0, 2.0, 3.0]);
        assert_eq!(emb.dim, 3);
    }

    #[test]
    fn test_embedding_dot_product() {
        let a = Embedding::new(vec![1.0, 0.0, 0.0]);
        let b = Embedding::new(vec![0.0, 1.0, 0.0]);
        let c = Embedding::new(vec![1.0, 0.0, 0.0]);

        assert!((a.dot(&b) - 0.0).abs() < 1e-6);
        assert!((a.dot(&c) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_distance_metrics() {
        let a = Embedding::new(vec![1.0, 0.0]);
        let b = Embedding::new(vec![0.0, 1.0]);

        let cosine_dist = DistanceMetric::Cosine.distance(&a, &b);
        assert!((cosine_dist - 1.0).abs() < 1e-6); // Orthogonal = 1.0 distance

        let euclidean_dist = DistanceMetric::Euclidean.distance(&a, &b);
        assert!((euclidean_dist - 2.0_f32.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn test_vector_index_insert_search() {
        let mut index = VectorIndex::new(3);

        index.insert("a", Embedding::new(vec![1.0, 0.0, 0.0])).unwrap();
        index.insert("b", Embedding::new(vec![0.9, 0.1, 0.0])).unwrap();
        index.insert("c", Embedding::new(vec![0.0, 1.0, 0.0])).unwrap();

        let query = Embedding::new(vec![1.0, 0.0, 0.0]);
        let results = index.search(&query, 2);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "a"); // Exact match should be first
    }

    #[test]
    fn test_vector_store() {
        let mut store = VectorStore::with_dim(3);

        store
            .put(VectorEntry::new("doc1", Embedding::new(vec![1.0, 0.0, 0.0])).with_content("Hello"))
            .unwrap();
        store
            .put(VectorEntry::new("doc2", Embedding::new(vec![0.0, 1.0, 0.0])).with_content("World"))
            .unwrap();

        let query = Embedding::new(vec![0.9, 0.1, 0.0]);
        let results = store.search(&query, 1);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
        assert!(results[0].entry.is_some());
    }

    #[test]
    fn test_vector_store_namespace() {
        let mut store = VectorStore::with_dim(2);

        store
            .put(
                VectorEntry::new("a", Embedding::new(vec![1.0, 0.0]))
                    .with_namespace("ns1"),
            )
            .unwrap();
        store
            .put(
                VectorEntry::new("b", Embedding::new(vec![0.9, 0.1]))
                    .with_namespace("ns2"),
            )
            .unwrap();

        let query = Embedding::new(vec![1.0, 0.0]);
        let results = store.search_namespace(&query, 10, "ns1");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "a");
    }

    #[test]
    fn test_quantum_fidelity() {
        // Bell state amplitudes: |00⟩ + |11⟩ normalized
        let bell = Embedding::new(vec![0.707, 0.0, 0.0, 0.707]);
        let same = Embedding::new(vec![0.707, 0.0, 0.0, 0.707]);
        let orthogonal = Embedding::new(vec![0.0, 0.707, 0.707, 0.0]);

        // Same state should have fidelity ~1
        let fid_same = bell.fidelity(&same);
        assert!((fid_same - 1.0).abs() < 0.01);

        // Orthogonal states should have fidelity ~0
        let fid_orth = bell.fidelity(&orthogonal);
        assert!(fid_orth < 0.01);
    }

    #[test]
    fn test_fidelity_distance_metric() {
        let a = Embedding::new(vec![1.0, 0.0]).normalized();
        let b = Embedding::new(vec![0.707, 0.707]).normalized();

        let fid_dist = DistanceMetric::Fidelity.distance(&a, &b);
        let fid_sim = DistanceMetric::Fidelity.similarity(&a, &b);

        // Fidelity should be |⟨a|b⟩|² = 0.5
        assert!((fid_sim - 0.5).abs() < 0.1);
        assert!((fid_dist - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_amplitude_encoding() {
        let emb = Embedding::new(vec![3.0, 4.0]);
        let amp = emb.to_amplitude_encoding();

        // Should be normalized to unit vector
        let norm: f32 = amp.data.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_upsert() {
        let mut store = VectorStore::with_dim(2);

        // First insert
        let existed = store.upsert(VectorEntry::new("a", Embedding::new(vec![1.0, 0.0]))).unwrap();
        assert!(!existed);

        // Update
        let existed = store.upsert(VectorEntry::new("a", Embedding::new(vec![0.0, 1.0]))).unwrap();
        assert!(existed);

        // Verify update
        let entry = store.get("a").unwrap();
        assert!((entry.embedding.data[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_find_similar() {
        let mut store = VectorStore::with_dim(2);

        store.put(VectorEntry::new("a", Embedding::new(vec![1.0, 0.0]))).unwrap();
        store.put(VectorEntry::new("b", Embedding::new(vec![0.9, 0.1]))).unwrap();
        store.put(VectorEntry::new("c", Embedding::new(vec![0.0, 1.0]))).unwrap();

        let similar = store.find_similar("a", 2);
        assert_eq!(similar.len(), 2);
        assert_eq!(similar[0].id, "b"); // Most similar to a
    }

    #[test]
    fn test_stats() {
        let mut store = VectorStore::with_dim(128);

        for i in 0..10 {
            store.put(
                VectorEntry::new(format!("v{}", i), Embedding::zeros(128))
                    .with_namespace(if i < 5 { "ns1" } else { "ns2" })
            ).unwrap();
        }

        let stats = store.stats();
        assert_eq!(stats.total_vectors, 10);
        assert_eq!(stats.dimensions, 128);
        assert_eq!(stats.namespaces, 2);
    }

    #[test]
    fn test_centroid() {
        let mut store = VectorStore::with_dim(2);

        store.put(VectorEntry::new("a", Embedding::new(vec![1.0, 0.0]))).unwrap();
        store.put(VectorEntry::new("b", Embedding::new(vec![0.0, 1.0]))).unwrap();

        let centroid = store.centroid(None).unwrap();
        assert!((centroid.data[0] - 0.5).abs() < 1e-6);
        assert!((centroid.data[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_hamming_distance() {
        let a = Embedding::new(vec![1.0, -1.0, 1.0, -1.0]);
        let b = Embedding::new(vec![1.0, 1.0, 1.0, -1.0]);

        let dist = DistanceMetric::Hamming.distance(&a, &b);
        assert!((dist - 1.0).abs() < 1e-6); // One bit different
    }

    #[test]
    fn test_simd_cosine() {
        let a: Vec<f32> = (0..128).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..128).map(|i| (i * 2) as f32).collect();

        let fast = cosine_distance_fast(&a, &b);
        let slow = DistanceMetric::Cosine.distance(
            &Embedding::new(a.clone()),
            &Embedding::new(b.clone())
        );

        assert!((fast - slow).abs() < 1e-4);
    }
}
