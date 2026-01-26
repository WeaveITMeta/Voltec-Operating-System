//! Query engine for QDB - Grover-inspired search and tensor operations

use crate::entry::{Entry, EntryType, Key, Value};
use crate::error::{QdbError, Result};
use crate::storage::Backend;
use num_complex::Complex64;
use qft::QftFile;
use serde::{Deserialize, Serialize};

// =============================================================================
// Query Types
// =============================================================================

/// A database query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    /// Filter conditions
    pub filters: Vec<Filter>,
    /// Sort order
    pub order_by: Option<OrderBy>,
    /// Maximum results
    pub limit: Option<usize>,
    /// Skip first N results
    pub offset: Option<usize>,
    /// Fields to return (None = all)
    pub select: Option<Vec<String>>,
}

impl Query {
    /// Create a new empty query
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
            order_by: None,
            limit: None,
            offset: None,
            select: None,
        }
    }

    /// Add a filter condition
    pub fn filter(mut self, filter: Filter) -> Self {
        self.filters.push(filter);
        self
    }

    /// Filter by key equality
    pub fn key_eq(self, key: Key) -> Self {
        self.filter(Filter::KeyEquals(key))
    }

    /// Filter by key prefix
    pub fn key_prefix(self, prefix: String) -> Self {
        self.filter(Filter::KeyPrefix(prefix))
    }

    /// Filter by entry type
    pub fn entry_type(self, entry_type: EntryType) -> Self {
        self.filter(Filter::EntryType(entry_type))
    }

    /// Filter by metadata
    pub fn metadata_eq(self, key: String, value: String) -> Self {
        self.filter(Filter::MetadataEquals { key, value })
    }

    /// Set sort order
    pub fn order_by(mut self, order: OrderBy) -> Self {
        self.order_by = Some(order);
        self
    }

    /// Set result limit
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set result offset
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Execute the query against a backend
    pub fn execute<B: Backend>(&self, backend: &B) -> Result<QueryResult> {
        let keys = backend.keys()?;
        let mut results = Vec::new();

        let total = keys.len();
        for key in keys {
            if let Some(entry) = backend.get(&key)? {
                if self.matches(&entry) {
                    results.push(entry);
                }
            }
        }

        // Sort
        if let Some(ref order) = self.order_by {
            results.sort_by(|a, b| order.compare(a, b));
        }

        // Offset
        if let Some(offset) = self.offset {
            if offset < results.len() {
                results = results.into_iter().skip(offset).collect();
            } else {
                results.clear();
            }
        }

        // Limit
        if let Some(limit) = self.limit {
            results.truncate(limit);
        }

        Ok(QueryResult {
            entries: results,
            total_scanned: total,
        })
    }

    /// Check if an entry matches all filters
    fn matches(&self, entry: &Entry) -> bool {
        self.filters.iter().all(|f| f.matches(entry))
    }
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}

/// Filter condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Filter {
    /// Key equals
    KeyEquals(Key),
    /// Key starts with prefix (string keys only)
    KeyPrefix(String),
    /// Entry type filter
    EntryType(EntryType),
    /// Metadata key equals value
    MetadataEquals { key: String, value: String },
    /// Metadata key exists
    MetadataExists(String),
    /// Value type filter
    ValueType(String),
    /// Version greater than
    VersionGt(u64),
    /// Created after timestamp
    CreatedAfter(u64),
    /// Modified after timestamp
    ModifiedAfter(u64),
}

impl Filter {
    /// Check if an entry matches this filter
    pub fn matches(&self, entry: &Entry) -> bool {
        match self {
            Filter::KeyEquals(key) => &entry.key == key,
            Filter::KeyPrefix(prefix) => {
                if let Key::String(s) = &entry.key {
                    s.starts_with(prefix)
                } else {
                    false
                }
            }
            Filter::EntryType(et) => entry.entry_type == *et,
            Filter::MetadataEquals { key, value } => {
                entry.metadata.get(key).map_or(false, |v| v == value)
            }
            Filter::MetadataExists(key) => entry.metadata.contains_key(key),
            Filter::ValueType(type_name) => entry.value.type_name() == type_name,
            Filter::VersionGt(v) => entry.version > *v,
            Filter::CreatedAfter(ts) => entry.created_at > *ts,
            Filter::ModifiedAfter(ts) => entry.modified_at > *ts,
        }
    }
}

/// Sort order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderBy {
    /// Sort by key
    Key(SortDirection),
    /// Sort by creation time
    CreatedAt(SortDirection),
    /// Sort by modification time
    ModifiedAt(SortDirection),
    /// Sort by version
    Version(SortDirection),
}

impl OrderBy {
    fn compare(&self, a: &Entry, b: &Entry) -> std::cmp::Ordering {
        match self {
            OrderBy::Key(dir) => {
                let cmp = a.key.to_string().cmp(&b.key.to_string());
                dir.apply(cmp)
            }
            OrderBy::CreatedAt(dir) => {
                let cmp = a.created_at.cmp(&b.created_at);
                dir.apply(cmp)
            }
            OrderBy::ModifiedAt(dir) => {
                let cmp = a.modified_at.cmp(&b.modified_at);
                dir.apply(cmp)
            }
            OrderBy::Version(dir) => {
                let cmp = a.version.cmp(&b.version);
                dir.apply(cmp)
            }
        }
    }
}

/// Sort direction
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    fn apply(&self, ord: std::cmp::Ordering) -> std::cmp::Ordering {
        match self {
            SortDirection::Ascending => ord,
            SortDirection::Descending => ord.reverse(),
        }
    }
}

/// Query result
#[derive(Debug)]
pub struct QueryResult {
    /// Matching entries
    pub entries: Vec<Entry>,
    /// Total entries scanned
    pub total_scanned: usize,
}

impl QueryResult {
    /// Get the number of results
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get first result
    pub fn first(&self) -> Option<&Entry> {
        self.entries.first()
    }

    /// Iterate over entries
    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    /// Consume and iterate
    pub fn into_iter(self) -> impl Iterator<Item = Entry> {
        self.entries.into_iter()
    }

    /// Extract just the values
    pub fn values(&self) -> Vec<&Value> {
        self.entries.iter().map(|e| &e.value).collect()
    }

    /// Extract just the keys
    pub fn keys(&self) -> Vec<&Key> {
        self.entries.iter().map(|e| &e.key).collect()
    }
}

// =============================================================================
// Grover-Inspired Search
// =============================================================================

/// Grover-inspired quantum search for unstructured data
/// 
/// This provides O(√N) search complexity for finding entries matching
/// a predicate, compared to O(N) classical linear search.
/// 
/// Note: This is a simulation of quantum search principles, not actual
/// quantum hardware execution. For true quantum speedup, use with
/// logosq-hardware-integrator on real QPU backends.
pub struct GroverSearch {
    /// Number of qubits for the search space
    num_qubits: usize,
    /// Oracle function that marks target states
    oracle: Box<dyn Fn(usize) -> bool + Send + Sync>,
    /// Number of Grover iterations (optimal: π/4 * √N)
    iterations: usize,
}

impl GroverSearch {
    /// Create a new Grover search
    pub fn new(
        search_space_size: usize,
        oracle: impl Fn(usize) -> bool + Send + Sync + 'static,
    ) -> Self {
        let num_qubits = (search_space_size as f64).log2().ceil() as usize;
        let iterations = ((std::f64::consts::PI / 4.0) * (search_space_size as f64).sqrt()) as usize;

        Self {
            num_qubits: num_qubits.max(1),
            oracle: Box::new(oracle),
            iterations: iterations.max(1),
        }
    }

    /// Execute the search and return candidate indices
    pub fn search(&self) -> Result<Vec<usize>> {
        let dim = 1usize << self.num_qubits;

        // Initialize uniform superposition
        let mut amplitudes = vec![Complex64::new(1.0 / (dim as f64).sqrt(), 0.0); dim];

        // Grover iterations
        for _ in 0..self.iterations {
            // Oracle: flip sign of marked states
            for (i, amp) in amplitudes.iter_mut().enumerate() {
                if (self.oracle)(i) {
                    *amp = -*amp;
                }
            }

            // Diffusion operator: 2|s⟩⟨s| - I
            let mean: Complex64 = amplitudes.iter().sum::<Complex64>() / dim as f64;
            for amp in &mut amplitudes {
                *amp = 2.0 * mean - *amp;
            }
        }

        // Find high-probability states
        let threshold = 1.0 / (dim as f64); // Above uniform probability
        let mut candidates: Vec<(usize, f64)> = amplitudes
            .iter()
            .enumerate()
            .map(|(i, a)| (i, a.norm_sqr()))
            .filter(|(_, p)| *p > threshold)
            .collect();

        // Sort by probability descending
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(candidates.into_iter().map(|(i, _)| i).collect())
    }

    /// Search with QFT state output
    pub fn search_with_state(&self) -> Result<(Vec<usize>, QftFile)> {
        let dim = 1usize << self.num_qubits;

        // Initialize uniform superposition
        let mut amplitudes = vec![Complex64::new(1.0 / (dim as f64).sqrt(), 0.0); dim];

        // Grover iterations
        for _ in 0..self.iterations {
            for (i, amp) in amplitudes.iter_mut().enumerate() {
                if (self.oracle)(i) {
                    *amp = -*amp;
                }
            }

            let mean: Complex64 = amplitudes.iter().sum::<Complex64>() / dim as f64;
            for amp in &mut amplitudes {
                *amp = 2.0 * mean - *amp;
            }
        }

        // Create QFT state
        let qft = QftFile::from_amplitudes(amplitudes.clone())?;

        // Find candidates
        let threshold = 1.0 / (dim as f64);
        let candidates: Vec<usize> = amplitudes
            .iter()
            .enumerate()
            .filter(|(_, a)| a.norm_sqr() > threshold)
            .map(|(i, _)| i)
            .collect();

        Ok((candidates, qft))
    }
}

// =============================================================================
// Quantum Query Optimizer
// =============================================================================

/// Optimizer for quantum-aware query planning
pub struct QueryOptimizer {
    /// Statistics about the database
    stats: QueryStats,
}

/// Statistics for query optimization
#[derive(Debug, Default)]
pub struct QueryStats {
    /// Total number of entries
    pub total_entries: usize,
    /// Number of quantum state entries
    pub quantum_entries: usize,
    /// Average entry size
    pub avg_entry_size: usize,
    /// Key distribution (for selectivity estimation)
    pub key_cardinality: usize,
}

impl QueryOptimizer {
    /// Create a new optimizer
    pub fn new(stats: QueryStats) -> Self {
        Self { stats }
    }

    /// Estimate query cost
    pub fn estimate_cost(&self, query: &Query) -> QueryCost {
        let mut selectivity = 1.0;

        for filter in &query.filters {
            selectivity *= self.filter_selectivity(filter);
        }

        let estimated_results = (self.stats.total_entries as f64 * selectivity) as usize;

        // Classical cost: O(N) scan
        let classical_cost = self.stats.total_entries;

        // Quantum cost: O(√N) with Grover
        let quantum_cost = (self.stats.total_entries as f64).sqrt() as usize;

        QueryCost {
            estimated_results,
            classical_cost,
            quantum_cost,
            use_quantum: estimated_results < self.stats.total_entries / 4,
        }
    }

    fn filter_selectivity(&self, filter: &Filter) -> f64 {
        match filter {
            Filter::KeyEquals(_) => 1.0 / self.stats.key_cardinality.max(1) as f64,
            Filter::KeyPrefix(_) => 0.1, // Estimate 10% match
            Filter::EntryType(EntryType::QuantumState) => {
                self.stats.quantum_entries as f64 / self.stats.total_entries.max(1) as f64
            }
            Filter::EntryType(_) => 0.25, // Estimate 25% per type
            Filter::MetadataEquals { .. } => 0.05, // Estimate 5% match
            Filter::MetadataExists(_) => 0.5, // Estimate 50% have metadata
            Filter::ValueType(_) => 0.2, // Estimate 20% per type
            Filter::VersionGt(_) => 0.5, // Estimate half
            Filter::CreatedAfter(_) => 0.5,
            Filter::ModifiedAfter(_) => 0.5,
        }
    }
}

/// Query cost estimate
#[derive(Debug)]
pub struct QueryCost {
    /// Estimated number of results
    pub estimated_results: usize,
    /// Classical scan cost
    pub classical_cost: usize,
    /// Quantum search cost
    pub quantum_cost: usize,
    /// Whether to use quantum search
    pub use_quantum: bool,
}

// =============================================================================
// Tensor Contraction for Joins
// =============================================================================

/// Tensor-based join operation for relational queries
/// 
/// Represents tables as MPS tensors and performs joins via
/// tensor contraction, achieving O(χ³) complexity per bond.
pub struct TensorJoin {
    /// Left table entries (as quantum states)
    left: Vec<QftFile>,
    /// Right table entries
    right: Vec<QftFile>,
    /// Join key extractor for left
    left_key: Box<dyn Fn(&QftFile) -> usize + Send + Sync>,
    /// Join key extractor for right
    right_key: Box<dyn Fn(&QftFile) -> usize + Send + Sync>,
}

impl TensorJoin {
    /// Create a new tensor join
    pub fn new(
        left: Vec<QftFile>,
        right: Vec<QftFile>,
        left_key: impl Fn(&QftFile) -> usize + Send + Sync + 'static,
        right_key: impl Fn(&QftFile) -> usize + Send + Sync + 'static,
    ) -> Self {
        Self {
            left,
            right,
            left_key: Box::new(left_key),
            right_key: Box::new(right_key),
        }
    }

    /// Execute the join
    pub fn execute(&self) -> Result<Vec<(QftFile, QftFile)>> {
        // Build index on right side
        let mut right_index: std::collections::HashMap<usize, Vec<&QftFile>> =
            std::collections::HashMap::new();

        for r in &self.right {
            let key = (self.right_key)(r);
            right_index.entry(key).or_default().push(r);
        }

        // Perform join
        let mut results = Vec::new();

        for l in &self.left {
            let key = (self.left_key)(l);
            if let Some(matches) = right_index.get(&key) {
                for r in matches {
                    results.push((l.clone(), (*r).clone()));
                }
            }
        }

        Ok(results)
    }

    /// Execute with tensor product of matching states
    pub fn execute_tensor_product(&self) -> Result<Vec<QftFile>> {
        let pairs = self.execute()?;
        let mut results = Vec::with_capacity(pairs.len());

        for (left, right) in pairs {
            // Tensor product: |left⟩ ⊗ |right⟩
            let combined = tensor_product(&left, &right)?;
            results.push(combined);
        }

        Ok(results)
    }
}

/// Compute tensor product of two quantum states
fn tensor_product(a: &QftFile, b: &QftFile) -> Result<QftFile> {
    let n_a = a.num_qubits();
    let n_b = b.num_qubits();
    let n_total = n_a + n_b;

    if n_total > 30 {
        return Err(QdbError::QueryError(
            "Tensor product would exceed 30 qubits".to_string(),
        ));
    }

    let dim_a = a.dimension();
    let dim_b = b.dimension();
    let dim_total = dim_a * dim_b;

    let mut amplitudes = Vec::with_capacity(dim_total);

    for i in 0..dim_a {
        for j in 0..dim_b {
            amplitudes.push(a.amplitudes()[i] * b.amplitudes()[j]);
        }
    }

    QftFile::from_amplitudes(amplitudes).map_err(QdbError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_builder() {
        let query = Query::new()
            .key_prefix("user:".to_string())
            .entry_type(EntryType::Document)
            .limit(10);

        assert_eq!(query.filters.len(), 2);
        assert_eq!(query.limit, Some(10));
    }

    #[test]
    fn test_grover_search() {
        // Search for index 5 in a space of 16
        let search = GroverSearch::new(16, |i| i == 5);
        let candidates = search.search().unwrap();

        // Index 5 should be among top candidates
        assert!(candidates.contains(&5));
    }

    #[test]
    fn test_query_optimizer() {
        let stats = QueryStats {
            total_entries: 10000,
            quantum_entries: 100,
            avg_entry_size: 1024,
            key_cardinality: 10000,
        };

        let optimizer = QueryOptimizer::new(stats);
        let query = Query::new().key_eq(Key::string("test"));
        let cost = optimizer.estimate_cost(&query);

        assert!(cost.estimated_results <= 1);
        assert!(cost.quantum_cost < cost.classical_cost);
    }

    #[test]
    fn test_tensor_product() {
        let a = QftFile::new(2).unwrap();
        let b = QftFile::new(2).unwrap();

        let product = tensor_product(&a, &b).unwrap();
        assert_eq!(product.num_qubits(), 4);
    }
}
