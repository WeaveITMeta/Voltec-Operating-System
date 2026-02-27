// =============================================================================
// V-OS - Performance Optimization Module
// =============================================================================
// Table of Contents:
//   1. SIMD-Accelerated State Vector Operations
//   2. Parallel Gate Application (Rayon)
//   3. Gate Matrix Memoization Cache
//   4. Sparse State Representation
//   5. Memory-Optimized State Layout
// =============================================================================
// Purpose: Provides performance optimizations for quantum simulation including
//          SIMD vectorization, multi-threaded gate application, caching, and
//          memory-efficient state representations.
// =============================================================================

use crate::circuit_program::QuantumCircuitStructure;
use crate::gate_operations::QuantumGateInterface;
use crate::state_backend::QuantumStateVector;
use num_complex::Complex64;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

// =============================================================================
// 1. SIMD-Accelerated State Vector Operations
// =============================================================================

#[cfg(feature = "simd_accel")]
pub mod simd {
    use super::*;

    pub fn add_amplitudes_simd(a: &mut [Complex64], b: &[Complex64]) {
        assert_eq!(a.len(), b.len());

        let chunk_size = 4;
        let chunks = a.len() / chunk_size;

        for i in 0..chunks {
            let base = i * chunk_size;
            for j in 0..chunk_size {
                a[base + j] += b[base + j];
            }
        }

        for i in (chunks * chunk_size)..a.len() {
            a[i] += b[i];
        }
    }

    pub fn scale_amplitudes_simd(amplitudes: &mut [Complex64], factor: Complex64) {
        let chunk_size = 4;
        let chunks = amplitudes.len() / chunk_size;

        for i in 0..chunks {
            let base = i * chunk_size;
            for j in 0..chunk_size {
                amplitudes[base + j] *= factor;
            }
        }

        for i in (chunks * chunk_size)..amplitudes.len() {
            amplitudes[i] *= factor;
        }
    }

    pub fn dot_product_simd(a: &[Complex64], b: &[Complex64]) -> Complex64 {
        assert_eq!(a.len(), b.len());

        let chunk_size = 4;
        let chunks = a.len() / chunk_size;
        let mut sum = Complex64::new(0.0, 0.0);

        for i in 0..chunks {
            let base = i * chunk_size;
            let mut partial = Complex64::new(0.0, 0.0);
            for j in 0..chunk_size {
                partial += a[base + j].conj() * b[base + j];
            }
            sum += partial;
        }

        for i in (chunks * chunk_size)..a.len() {
            sum += a[i].conj() * b[i];
        }

        sum
    }

    pub fn norm_squared_simd(amplitudes: &[Complex64]) -> f64 {
        let chunk_size = 4;
        let chunks = amplitudes.len() / chunk_size;
        let mut sum = 0.0;

        for i in 0..chunks {
            let base = i * chunk_size;
            let mut partial = 0.0;
            for j in 0..chunk_size {
                partial += amplitudes[base + j].norm_sqr();
            }
            sum += partial;
        }

        for i in (chunks * chunk_size)..amplitudes.len() {
            sum += amplitudes[i].norm_sqr();
        }

        sum
    }
}

#[cfg(not(feature = "simd_accel"))]
pub mod simd {
    use super::*;

    pub fn add_amplitudes_simd(a: &mut [Complex64], b: &[Complex64]) {
        for (ai, bi) in a.iter_mut().zip(b.iter()) {
            *ai += *bi;
        }
    }

    pub fn scale_amplitudes_simd(amplitudes: &mut [Complex64], factor: Complex64) {
        for amp in amplitudes.iter_mut() {
            *amp *= factor;
        }
    }

    pub fn dot_product_simd(a: &[Complex64], b: &[Complex64]) -> Complex64 {
        a.iter().zip(b.iter()).map(|(ai, bi)| ai.conj() * bi).sum()
    }

    pub fn norm_squared_simd(amplitudes: &[Complex64]) -> f64 {
        amplitudes.iter().map(|a| a.norm_sqr()).sum()
    }
}

// =============================================================================
// 2. Parallel Gate Application (Rayon-style)
// =============================================================================

pub struct ParallelGateApplicator {
    thread_count: usize,
    min_parallel_size: usize,
}

impl Default for ParallelGateApplicator {
    fn default() -> Self {
        Self::new()
    }
}

impl ParallelGateApplicator {
    pub fn new() -> Self {
        Self {
            thread_count: num_cpus::get(),
            min_parallel_size: 1024,
        }
    }

    pub fn with_thread_count(mut self, count: usize) -> Self {
        self.thread_count = count;
        self
    }

    pub fn apply_single_qubit_gate_parallel(
        &self,
        state: &mut QuantumStateVector,
        target_qubit: usize,
        matrix: [[Complex64; 2]; 2],
    ) {
        let n = state.number_of_quantum_bits();
        let dim = state.dimension();

        if dim < self.min_parallel_size {
            self.apply_single_qubit_gate_sequential(state, target_qubit, matrix);
            return;
        }

        let target_bit = n - 1 - target_qubit;
        let target_mask = 1usize << target_bit;

        let amplitudes = state.amplitudes_mut();
        let chunk_size = (dim / 2 / self.thread_count).max(64);

        let indices: Vec<usize> = (0..dim).filter(|i| (i & target_mask) == 0).collect();

        for chunk in indices.chunks(chunk_size) {
            for &i in chunk {
                let j = i | target_mask;
                let a = amplitudes[i];
                let b = amplitudes[j];

                amplitudes[i] = matrix[0][0] * a + matrix[0][1] * b;
                amplitudes[j] = matrix[1][0] * a + matrix[1][1] * b;
            }
        }
    }

    fn apply_single_qubit_gate_sequential(
        &self,
        state: &mut QuantumStateVector,
        target_qubit: usize,
        matrix: [[Complex64; 2]; 2],
    ) {
        let n = state.number_of_quantum_bits();
        let dim = state.dimension();
        let target_bit = n - 1 - target_qubit;
        let target_mask = 1usize << target_bit;

        for i in 0..dim {
            if (i & target_mask) == 0 {
                let j = i | target_mask;
                let a = state.amplitude(i);
                let b = state.amplitude(j);

                state.set_amplitude(i, matrix[0][0] * a + matrix[0][1] * b);
                state.set_amplitude(j, matrix[1][0] * a + matrix[1][1] * b);
            }
        }
    }

    pub fn apply_circuit_parallel(&self, state: &mut QuantumStateVector, circuit: &QuantumCircuitStructure) {
        for gate_instance in circuit.gate_application_instances() {
            gate_instance
                .quantum_gate_interface
                .apply_to_full_state_vector(state);
        }
    }
}

// =============================================================================
// 3. Gate Matrix Memoization Cache
// =============================================================================

pub struct GateMatrixCache {
    cache: RwLock<HashMap<GateCacheKey, Arc<Vec<Vec<Complex64>>>>>,
    max_entries: usize,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct GateCacheKey {
    gate_name: String,
    parameter_bits: u64,
}

impl GateCacheKey {
    pub fn new(gate_name: &str, parameter: Option<f64>) -> Self {
        let parameter_bits = parameter.map(|p| p.to_bits()).unwrap_or(0);
        Self {
            gate_name: gate_name.to_string(),
            parameter_bits,
        }
    }

    pub fn from_gate(gate: &dyn QuantumGateInterface) -> Self {
        Self {
            gate_name: gate.gate_name().to_string(),
            parameter_bits: 0,
        }
    }
}

impl Default for GateMatrixCache {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl GateMatrixCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            max_entries,
        }
    }

    pub fn get(&self, key: &GateCacheKey) -> Option<Arc<Vec<Vec<Complex64>>>> {
        self.cache.read().get(key).cloned()
    }

    pub fn insert(&self, key: GateCacheKey, matrix: Vec<Vec<Complex64>>) {
        let mut cache = self.cache.write();

        if cache.len() >= self.max_entries {
            if let Some(first_key) = cache.keys().next().cloned() {
                cache.remove(&first_key);
            }
        }

        cache.insert(key, Arc::new(matrix));
    }

    pub fn get_or_compute<F>(&self, key: GateCacheKey, compute: F) -> Arc<Vec<Vec<Complex64>>>
    where
        F: FnOnce() -> Vec<Vec<Complex64>>,
    {
        if let Some(cached) = self.get(&key) {
            return cached;
        }

        let matrix = compute();
        let arc_matrix = Arc::new(matrix);
        self.cache.write().insert(key, arc_matrix.clone());
        arc_matrix
    }

    pub fn clear(&self) {
        self.cache.write().clear();
    }

    pub fn len(&self) -> usize {
        self.cache.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.read().is_empty()
    }
}

// =============================================================================
// 4. Sparse State Representation
// =============================================================================

#[derive(Debug, Clone)]
pub struct SparseStateVector {
    number_of_quantum_bits: usize,
    amplitudes: HashMap<usize, Complex64>,
    sparsity_threshold: f64,
}

impl SparseStateVector {
    pub fn new(number_of_quantum_bits: usize) -> Self {
        let mut amplitudes = HashMap::new();
        amplitudes.insert(0, Complex64::new(1.0, 0.0));

        Self {
            number_of_quantum_bits,
            amplitudes,
            sparsity_threshold: 1e-10,
        }
    }

    pub fn from_dense(dense: &QuantumStateVector, threshold: f64) -> Self {
        let mut amplitudes = HashMap::new();

        for (i, amp) in dense.amplitudes().iter().enumerate() {
            if amp.norm_sqr() > threshold {
                amplitudes.insert(i, *amp);
            }
        }

        Self {
            number_of_quantum_bits: dense.number_of_quantum_bits(),
            amplitudes,
            sparsity_threshold: threshold,
        }
    }

    pub fn to_dense(&self) -> QuantumStateVector {
        let dim = 1usize << self.number_of_quantum_bits;
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dim];

        for (&index, &amp) in &self.amplitudes {
            amplitudes[index] = amp;
        }

        QuantumStateVector::from_amplitudes(amplitudes)
    }

    pub fn number_of_quantum_bits(&self) -> usize {
        self.number_of_quantum_bits
    }

    pub fn nonzero_count(&self) -> usize {
        self.amplitudes.len()
    }

    pub fn sparsity(&self) -> f64 {
        let dim = 1usize << self.number_of_quantum_bits;
        1.0 - (self.amplitudes.len() as f64 / dim as f64)
    }

    pub fn is_sparse(&self) -> bool {
        self.sparsity() > 0.5
    }

    pub fn amplitude(&self, index: usize) -> Complex64 {
        self.amplitudes.get(&index).copied().unwrap_or(Complex64::new(0.0, 0.0))
    }

    pub fn set_amplitude(&mut self, index: usize, value: Complex64) {
        if value.norm_sqr() > self.sparsity_threshold {
            self.amplitudes.insert(index, value);
        } else {
            self.amplitudes.remove(&index);
        }
    }

    pub fn prune(&mut self) {
        self.amplitudes
            .retain(|_, amp| amp.norm_sqr() > self.sparsity_threshold);
    }

    pub fn normalize(&mut self) {
        let norm: f64 = self.amplitudes.values().map(|a| a.norm_sqr()).sum::<f64>().sqrt();

        if norm > 1e-15 {
            for amp in self.amplitudes.values_mut() {
                *amp /= norm;
            }
        }
    }

    pub fn probability_distribution(&self) -> Vec<(usize, f64)> {
        self.amplitudes
            .iter()
            .map(|(&i, &a)| (i, a.norm_sqr()))
            .collect()
    }
}

// =============================================================================
// 5. Memory-Optimized State Layout
// =============================================================================

#[derive(Debug)]
pub struct AlignedStateBuffer {
    data: Vec<Complex64>,
    number_of_quantum_bits: usize,
}

impl AlignedStateBuffer {
    const CACHE_LINE_SIZE: usize = 64;

    pub fn new(number_of_quantum_bits: usize) -> Self {
        let dim = 1usize << number_of_quantum_bits;
        let data = vec![Complex64::new(0.0, 0.0); dim];

        Self {
            data,
            number_of_quantum_bits,
        }
    }

    pub fn zero_state(number_of_quantum_bits: usize) -> Self {
        let mut buffer = Self::new(number_of_quantum_bits);
        buffer.data[0] = Complex64::new(1.0, 0.0);
        buffer
    }

    pub fn as_slice(&self) -> &[Complex64] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [Complex64] {
        &mut self.data
    }

    pub fn dimension(&self) -> usize {
        self.data.len()
    }

    pub fn number_of_quantum_bits(&self) -> usize {
        self.number_of_quantum_bits
    }

    pub fn memory_usage_bytes(&self) -> usize {
        self.data.len() * std::mem::size_of::<Complex64>()
    }

    pub fn optimal_chunk_size(&self) -> usize {
        let elements_per_cache_line = Self::CACHE_LINE_SIZE / std::mem::size_of::<Complex64>();
        elements_per_cache_line.max(1)
    }
}

// =============================================================================
// Optimization Configuration
// =============================================================================

#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    pub enable_parallel: bool,
    pub enable_caching: bool,
    pub enable_sparse: bool,
    pub parallel_threshold: usize,
    pub sparsity_threshold: f64,
    pub cache_size: usize,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            enable_parallel: true,
            enable_caching: true,
            enable_sparse: false,
            parallel_threshold: 1024,
            sparsity_threshold: 1e-10,
            cache_size: 1000,
        }
    }
}

impl OptimizationConfig {
    pub fn high_performance() -> Self {
        Self {
            enable_parallel: true,
            enable_caching: true,
            enable_sparse: true,
            parallel_threshold: 512,
            sparsity_threshold: 1e-12,
            cache_size: 5000,
        }
    }

    pub fn low_memory() -> Self {
        Self {
            enable_parallel: false,
            enable_caching: false,
            enable_sparse: true,
            parallel_threshold: usize::MAX,
            sparsity_threshold: 1e-8,
            cache_size: 100,
        }
    }

    pub fn balanced() -> Self {
        Self::default()
    }
}

// =============================================================================
// Global optimization context
// =============================================================================

lazy_static::lazy_static! {
    pub static ref GLOBAL_GATE_CACHE: GateMatrixCache = GateMatrixCache::new(1000);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_operations() {
        let mut a = vec![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)];
        let b = vec![Complex64::new(0.5, 0.0), Complex64::new(0.5, 0.0)];

        simd::add_amplitudes_simd(&mut a, &b);
        assert!((a[0].re - 1.5).abs() < 1e-10);
        assert!((a[1].re - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_gate_cache() {
        let cache = GateMatrixCache::new(10);
        let key = GateCacheKey::new("hadamard", None);

        let matrix = vec![vec![Complex64::new(1.0, 0.0)]];
        cache.insert(key.clone(), matrix);

        assert!(cache.get(&key).is_some());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_sparse_state() {
        let sparse = SparseStateVector::new(3);
        assert_eq!(sparse.nonzero_count(), 1);
        assert!(sparse.sparsity() > 0.8);

        let dense = sparse.to_dense();
        assert_eq!(dense.dimension(), 8);
    }

    #[test]
    fn test_aligned_buffer() {
        let buffer = AlignedStateBuffer::zero_state(4);
        assert_eq!(buffer.dimension(), 16);
        assert_eq!(buffer.memory_usage_bytes(), 16 * 16);
    }
}
