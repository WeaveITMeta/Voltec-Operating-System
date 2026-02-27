// =============================================================================
// V-OS - Bond Dimension Optimization
// =============================================================================
// Table of Contents:
//   1. BondDimensionOptimizer - Automatic χ Selection
//   2. EntanglementAnalyzer - Entanglement Entropy Estimation
//   3. FidelityEstimator - MPS Approximation Fidelity
//   4. OptimizationStrategy - Different Optimization Approaches
//   5. BondDimensionBenchmark - Benchmarking Utilities
// =============================================================================
// Purpose: Provides utilities for determining optimal bond dimension (χ) for
//          MPS approximations based on target fidelity, entanglement structure,
//          and storage constraints. Implements binary search and adaptive
//          algorithms for efficient χ selection.
// =============================================================================
// Complexity Analysis:
//   - Binary search optimization: O(log(χ_max) × n × χ²) 
//   - Entanglement entropy: O(n × χ²)
//   - Fidelity estimation: O(2^n) for exact, O(n × χ²) for sampled
// =============================================================================

use crate::qft_format::{convert_dense_to_mps, convert_mps_to_dense, SerializableMpsTensor};
use crate::state_backend::{MatrixProductStateConfiguration, QuantumStateVector};
use num_complex::Complex64;
use std::collections::HashMap;

// =============================================================================
// Error Types
// =============================================================================

#[derive(Debug, Clone, thiserror::Error)]
pub enum OptimizationError {
    #[error("Optimization failed: could not achieve target fidelity {target} (best: {achieved})")]
    TargetNotAchieved { target: f64, achieved: f64 },

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("MPS conversion error: {0}")]
    ConversionError(String),

    #[error("Numerical error: {0}")]
    NumericalError(String),
}

// =============================================================================
// 1. BondDimensionOptimizer - Automatic χ Selection
// =============================================================================

/// Configuration for bond dimension optimization
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    /// Target fidelity to achieve (default: 1 - 1e-10)
    pub target_fidelity: f64,
    /// Minimum bond dimension to consider
    pub min_bond_dim: usize,
    /// Maximum bond dimension to consider
    pub max_bond_dim: usize,
    /// SVD truncation threshold
    pub truncation_threshold: f64,
    /// Strategy for optimization
    pub strategy: OptimizationStrategy,
    /// Enable caching of intermediate results
    pub enable_caching: bool,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            target_fidelity: 1.0 - 1e-10,
            min_bond_dim: 1,
            max_bond_dim: 256,
            truncation_threshold: 1e-12,
            strategy: OptimizationStrategy::BinarySearch,
            enable_caching: true,
        }
    }
}

impl OptimizationConfig {
    /// Create config for high-fidelity optimization
    pub fn high_fidelity() -> Self {
        Self {
            target_fidelity: 1.0 - 1e-12,
            max_bond_dim: 512,
            truncation_threshold: 1e-14,
            ..Default::default()
        }
    }

    /// Create config for fast optimization (lower fidelity)
    pub fn fast() -> Self {
        Self {
            target_fidelity: 0.999,
            max_bond_dim: 64,
            truncation_threshold: 1e-8,
            strategy: OptimizationStrategy::LinearSearch,
            ..Default::default()
        }
    }

    /// Create config for memory-constrained optimization
    pub fn memory_constrained(max_chi: usize) -> Self {
        Self {
            max_bond_dim: max_chi,
            strategy: OptimizationStrategy::AdaptiveGradient,
            ..Default::default()
        }
    }
}

/// Strategy for finding optimal bond dimension
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationStrategy {
    /// Binary search for optimal χ (fastest for most cases)
    BinarySearch,
    /// Linear search from min to max (guaranteed to find minimum)
    LinearSearch,
    /// Adaptive gradient-based search
    AdaptiveGradient,
    /// Entanglement-guided estimation (fastest but approximate)
    EntanglementGuided,
}

/// Result of bond dimension optimization
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// Optimal bond dimension found
    pub optimal_bond_dim: usize,
    /// Achieved fidelity at optimal χ
    pub achieved_fidelity: f64,
    /// Number of iterations performed
    pub iterations: usize,
    /// Fidelity values at each tested χ (if caching enabled)
    pub fidelity_history: HashMap<usize, f64>,
    /// Estimated storage size at optimal χ
    pub estimated_storage_bytes: usize,
    /// Compression ratio vs dense storage
    pub compression_ratio: f64,
}

/// Optimizer for finding optimal bond dimension
#[derive(Debug)]
pub struct BondDimensionOptimizer {
    config: OptimizationConfig,
    cache: HashMap<usize, f64>,
}

impl BondDimensionOptimizer {
    /// Create a new optimizer with given configuration
    pub fn new(config: OptimizationConfig) -> Self {
        Self {
            config,
            cache: HashMap::new(),
        }
    }

    /// Create optimizer with default configuration
    pub fn default_optimizer() -> Self {
        Self::new(OptimizationConfig::default())
    }

    /// Find optimal bond dimension for a given state
    pub fn optimize(
        &mut self,
        state: &QuantumStateVector,
    ) -> Result<OptimizationResult, OptimizationError> {
        self.cache.clear();

        match self.config.strategy {
            OptimizationStrategy::BinarySearch => self.binary_search_optimize(state),
            OptimizationStrategy::LinearSearch => self.linear_search_optimize(state),
            OptimizationStrategy::AdaptiveGradient => self.adaptive_gradient_optimize(state),
            OptimizationStrategy::EntanglementGuided => self.entanglement_guided_optimize(state),
        }
    }

    /// Binary search optimization (most efficient for typical cases)
    fn binary_search_optimize(
        &mut self,
        state: &QuantumStateVector,
    ) -> Result<OptimizationResult, OptimizationError> {
        let n = state.number_of_quantum_bits();
        let max_possible = (1usize << (n / 2)).min(self.config.max_bond_dim);

        let mut low = self.config.min_bond_dim;
        let mut high = max_possible;
        let mut best_chi = high;
        let mut best_fidelity = 0.0;
        let mut iterations = 0;

        // First check if max bond dim achieves target
        let max_fidelity = self.compute_fidelity(state, high)?;
        if max_fidelity < self.config.target_fidelity {
            return Err(OptimizationError::TargetNotAchieved {
                target: self.config.target_fidelity,
                achieved: max_fidelity,
            });
        }

        while low <= high {
            iterations += 1;
            let mid = (low + high) / 2;

            let fidelity = self.compute_fidelity(state, mid)?;

            if fidelity >= self.config.target_fidelity {
                best_chi = mid;
                best_fidelity = fidelity;
                high = mid.saturating_sub(1);
            } else {
                low = mid + 1;
            }
        }

        self.build_result(state, best_chi, best_fidelity, iterations)
    }

    /// Linear search optimization (guaranteed minimum χ)
    fn linear_search_optimize(
        &mut self,
        state: &QuantumStateVector,
    ) -> Result<OptimizationResult, OptimizationError> {
        let n = state.number_of_quantum_bits();
        let max_possible = (1usize << (n / 2)).min(self.config.max_bond_dim);

        let mut iterations = 0;

        for chi in self.config.min_bond_dim..=max_possible {
            iterations += 1;
            let fidelity = self.compute_fidelity(state, chi)?;

            if fidelity >= self.config.target_fidelity {
                return self.build_result(state, chi, fidelity, iterations);
            }
        }

        // Check final fidelity
        let final_fidelity = self.cache.get(&max_possible).copied().unwrap_or(0.0);
        Err(OptimizationError::TargetNotAchieved {
            target: self.config.target_fidelity,
            achieved: final_fidelity,
        })
    }

    /// Adaptive gradient-based optimization
    fn adaptive_gradient_optimize(
        &mut self,
        state: &QuantumStateVector,
    ) -> Result<OptimizationResult, OptimizationError> {
        let n = state.number_of_quantum_bits();
        let max_possible = (1usize << (n / 2)).min(self.config.max_bond_dim);

        let mut chi = self.config.min_bond_dim;
        let mut step = (max_possible - chi) / 4;
        let mut iterations = 0;
        let mut best_chi = chi;
        let mut best_fidelity = 0.0;

        while step >= 1 && chi <= max_possible {
            iterations += 1;
            let fidelity = self.compute_fidelity(state, chi)?;

            if fidelity >= self.config.target_fidelity {
                best_chi = chi;
                best_fidelity = fidelity;
                // Try to reduce χ
                if chi > self.config.min_bond_dim {
                    chi = chi.saturating_sub(step);
                } else {
                    break;
                }
            } else {
                // Need larger χ
                chi += step;
            }

            step = step / 2;
        }

        if best_fidelity >= self.config.target_fidelity {
            self.build_result(state, best_chi, best_fidelity, iterations)
        } else {
            Err(OptimizationError::TargetNotAchieved {
                target: self.config.target_fidelity,
                achieved: best_fidelity,
            })
        }
    }

    /// Entanglement-guided optimization (fast estimation)
    fn entanglement_guided_optimize(
        &mut self,
        state: &QuantumStateVector,
    ) -> Result<OptimizationResult, OptimizationError> {
        let n = state.number_of_quantum_bits();
        let analyzer = EntanglementAnalyzer::new();

        // Estimate required χ from entanglement entropy
        let max_entropy = analyzer.estimate_max_entanglement_entropy(state);
        let estimated_chi = (2.0_f64.powf(max_entropy)).ceil() as usize;

        // Add safety margin
        let target_chi = (estimated_chi * 2).min(self.config.max_bond_dim);

        // Verify with actual fidelity computation
        let fidelity = self.compute_fidelity(state, target_chi)?;

        if fidelity >= self.config.target_fidelity {
            // Try to reduce
            let mut chi = target_chi;
            while chi > self.config.min_bond_dim {
                let test_chi = chi / 2;
                let test_fidelity = self.compute_fidelity(state, test_chi)?;
                if test_fidelity >= self.config.target_fidelity {
                    chi = test_chi;
                } else {
                    break;
                }
            }
            let final_fidelity = self.compute_fidelity(state, chi)?;
            self.build_result(state, chi, final_fidelity, 3)
        } else {
            // Need more χ, fall back to binary search
            self.binary_search_optimize(state)
        }
    }

    /// Compute fidelity for a given bond dimension
    fn compute_fidelity(
        &mut self,
        state: &QuantumStateVector,
        chi: usize,
    ) -> Result<f64, OptimizationError> {
        // Check cache
        if self.config.enable_caching {
            if let Some(&fidelity) = self.cache.get(&chi) {
                return Ok(fidelity);
            }
        }

        let config = MatrixProductStateConfiguration {
            maximum_bond_dimension: chi,
            truncation_threshold: self.config.truncation_threshold,
        };

        let tensors = convert_dense_to_mps(state, &config)
            .map_err(|e| OptimizationError::ConversionError(e.to_string()))?;

        let reconstructed = convert_mps_to_dense(&tensors)
            .map_err(|e| OptimizationError::ConversionError(e.to_string()))?;

        let fidelity = compute_fidelity(state, &reconstructed);

        if self.config.enable_caching {
            self.cache.insert(chi, fidelity);
        }

        Ok(fidelity)
    }

    /// Build optimization result
    fn build_result(
        &self,
        state: &QuantumStateVector,
        chi: usize,
        fidelity: f64,
        iterations: usize,
    ) -> Result<OptimizationResult, OptimizationError> {
        let n = state.number_of_quantum_bits();

        // Estimate storage size
        let tensor_data_size = n * chi * 2 * chi * 16; // Complex64 = 16 bytes
        let overhead = 48 + n * 28; // Header + descriptors
        let estimated_storage = tensor_data_size + overhead;

        // Compression ratio
        let dense_size = (1usize << n) * 16;
        let compression_ratio = dense_size as f64 / estimated_storage as f64;

        Ok(OptimizationResult {
            optimal_bond_dim: chi,
            achieved_fidelity: fidelity,
            iterations,
            fidelity_history: self.cache.clone(),
            estimated_storage_bytes: estimated_storage,
            compression_ratio,
        })
    }

    /// Get cached fidelity values
    pub fn get_cache(&self) -> &HashMap<usize, f64> {
        &self.cache
    }

    /// Clear the cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

// =============================================================================
// 2. EntanglementAnalyzer - Entanglement Entropy Estimation
// =============================================================================

/// Analyzer for entanglement properties of quantum states
#[derive(Debug, Clone, Default)]
pub struct EntanglementAnalyzer;

impl EntanglementAnalyzer {
    /// Create a new entanglement analyzer
    pub fn new() -> Self {
        Self
    }

    /// Estimate maximum entanglement entropy across all bipartitions
    pub fn estimate_max_entanglement_entropy(&self, state: &QuantumStateVector) -> f64 {
        let n = state.number_of_quantum_bits();
        if n <= 1 {
            return 0.0;
        }

        let mut max_entropy = 0.0;

        // Check entropy at each bipartition
        for cut in 1..n {
            let entropy = self.bipartite_entropy(state, cut);
            max_entropy = max_entropy.max(entropy);
        }

        max_entropy
    }

    /// Compute entanglement entropy for bipartition at given cut position
    pub fn bipartite_entropy(&self, state: &QuantumStateVector, cut: usize) -> f64 {
        let n = state.number_of_quantum_bits();
        if cut == 0 || cut >= n {
            return 0.0;
        }

        // Compute reduced density matrix by partial trace
        let left_dim = 1usize << cut;
        let right_dim = 1usize << (n - cut);

        // Compute ρ_A = Tr_B(|ψ⟩⟨ψ|)
        let mut rho_a = vec![Complex64::new(0.0, 0.0); left_dim * left_dim];

        for i in 0..left_dim {
            for j in 0..left_dim {
                let mut sum = Complex64::new(0.0, 0.0);
                for k in 0..right_dim {
                    let idx_i = i * right_dim + k;
                    let idx_j = j * right_dim + k;
                    sum += state.amplitude(idx_i) * state.amplitude(idx_j).conj();
                }
                rho_a[i * left_dim + j] = sum;
            }
        }

        // Compute eigenvalues of ρ_A (simplified: use diagonal approximation for speed)
        // For exact computation, would need full eigendecomposition
        let eigenvalues = self.approximate_eigenvalues(&rho_a, left_dim);

        // Von Neumann entropy: S = -Σ λᵢ log(λᵢ)
        let mut entropy = 0.0;
        for &lambda in &eigenvalues {
            if lambda > 1e-15 {
                entropy -= lambda * lambda.ln();
            }
        }

        entropy / std::f64::consts::LN_2 // Convert to bits
    }

    /// Approximate eigenvalues using power iteration
    fn approximate_eigenvalues(&self, matrix: &[Complex64], dim: usize) -> Vec<f64> {
        let mut eigenvalues = Vec::new();
        let mut remaining = matrix.to_vec();

        for _ in 0..dim.min(16) {
            // Limit iterations
            // Power iteration
            let mut v: Vec<Complex64> = (0..dim)
                .map(|i| Complex64::new((i as f64 + 1.0).sin(), 0.0))
                .collect();

            // Normalize
            let norm: f64 = v.iter().map(|x| x.norm_sqr()).sum::<f64>().sqrt();
            if norm > 1e-15 {
                for x in &mut v {
                    *x /= norm;
                }
            }

            for _ in 0..20 {
                // Multiply by matrix
                let mut new_v = vec![Complex64::new(0.0, 0.0); dim];
                for i in 0..dim {
                    for j in 0..dim {
                        new_v[i] += remaining[i * dim + j] * v[j];
                    }
                }

                // Normalize
                let norm: f64 = new_v.iter().map(|x| x.norm_sqr()).sum::<f64>().sqrt();
                if norm < 1e-15 {
                    break;
                }
                for i in 0..dim {
                    v[i] = new_v[i] / norm;
                }
            }

            // Compute Rayleigh quotient for eigenvalue
            let mut numerator = Complex64::new(0.0, 0.0);
            for i in 0..dim {
                for j in 0..dim {
                    numerator += v[i].conj() * remaining[i * dim + j] * v[j];
                }
            }
            let eigenvalue = numerator.re.max(0.0);

            if eigenvalue < 1e-12 {
                break;
            }

            eigenvalues.push(eigenvalue);

            // Deflate matrix
            for i in 0..dim {
                for j in 0..dim {
                    remaining[i * dim + j] -= Complex64::new(eigenvalue, 0.0) * v[i] * v[j].conj();
                }
            }
        }

        eigenvalues
    }

    /// Estimate if state is a product state (no entanglement)
    pub fn is_product_state(&self, state: &QuantumStateVector, tolerance: f64) -> bool {
        let max_entropy = self.estimate_max_entanglement_entropy(state);
        max_entropy < tolerance
    }

    /// Estimate entanglement class
    pub fn classify_entanglement(&self, state: &QuantumStateVector) -> EntanglementClass {
        let n = state.number_of_quantum_bits();
        let max_entropy = self.estimate_max_entanglement_entropy(state);

        if max_entropy < 0.01 {
            EntanglementClass::Product
        } else if max_entropy < 1.0 {
            EntanglementClass::LowEntanglement
        } else if max_entropy < (n as f64) / 2.0 {
            EntanglementClass::ModerateEntanglement
        } else {
            EntanglementClass::HighEntanglement
        }
    }
}

/// Classification of entanglement level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntanglementClass {
    /// No entanglement (product state)
    Product,
    /// Low entanglement (area-law states)
    LowEntanglement,
    /// Moderate entanglement
    ModerateEntanglement,
    /// High entanglement (volume-law states)
    HighEntanglement,
}

impl EntanglementClass {
    /// Suggested bond dimension range for this entanglement class
    pub fn suggested_bond_dim_range(&self, n_qubits: usize) -> (usize, usize) {
        match self {
            EntanglementClass::Product => (1, 2),
            EntanglementClass::LowEntanglement => (2, 16),
            EntanglementClass::ModerateEntanglement => (16, 64),
            EntanglementClass::HighEntanglement => (64, 1 << (n_qubits / 2)),
        }
    }
}

// =============================================================================
// 3. FidelityEstimator - MPS Approximation Fidelity
// =============================================================================

/// Compute fidelity |⟨ψ|φ⟩|² between two states
pub fn compute_fidelity(psi: &QuantumStateVector, phi: &QuantumStateVector) -> f64 {
    let inner = psi.inner_product(phi);
    inner.norm_sqr()
}

/// Compute infidelity 1 - |⟨ψ|φ⟩|²
pub fn compute_infidelity(psi: &QuantumStateVector, phi: &QuantumStateVector) -> f64 {
    1.0 - compute_fidelity(psi, phi)
}

/// Estimate fidelity using sampling (for large states)
pub fn estimate_fidelity_sampled(
    psi: &QuantumStateVector,
    phi: &QuantumStateVector,
    num_samples: usize,
) -> f64 {
    let dim = psi.dimension();
    if dim <= num_samples {
        return compute_fidelity(psi, phi);
    }

    // Sample random basis states and estimate overlap
    let mut sum = Complex64::new(0.0, 0.0);
    let step = dim / num_samples;

    for i in 0..num_samples {
        let idx = (i * step) % dim;
        sum += psi.amplitude(idx).conj() * phi.amplitude(idx);
    }

    // Scale by dimension ratio
    let scale = dim as f64 / num_samples as f64;
    (sum * scale).norm_sqr()
}

// =============================================================================
// 4. BondDimensionBenchmark - Benchmarking Utilities
// =============================================================================

/// Result of a single benchmark point
#[derive(Debug, Clone)]
pub struct BenchmarkPoint {
    pub bond_dim: usize,
    pub fidelity: f64,
    pub infidelity: f64,
    pub storage_bytes: usize,
    pub compression_ratio: f64,
}

/// Benchmark results across multiple bond dimensions
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub num_qubits: usize,
    pub state_type: String,
    pub points: Vec<BenchmarkPoint>,
    pub optimal_chi_for_fidelity: HashMap<String, usize>,
}

/// Run benchmark across range of bond dimensions
pub fn benchmark_bond_dimensions(
    state: &QuantumStateVector,
    state_type: &str,
    chi_values: &[usize],
) -> Result<BenchmarkResult, OptimizationError> {
    let n = state.number_of_quantum_bits();
    let dense_size = (1usize << n) * 16;
    let mut points = Vec::new();

    for &chi in chi_values {
        let config = MatrixProductStateConfiguration {
            maximum_bond_dimension: chi,
            truncation_threshold: 1e-12,
        };

        let tensors = convert_dense_to_mps(state, &config)
            .map_err(|e| OptimizationError::ConversionError(e.to_string()))?;

        let reconstructed = convert_mps_to_dense(&tensors)
            .map_err(|e| OptimizationError::ConversionError(e.to_string()))?;

        let fidelity = compute_fidelity(state, &reconstructed);
        let storage = n * chi * 2 * chi * 16 + 48 + n * 28;

        points.push(BenchmarkPoint {
            bond_dim: chi,
            fidelity,
            infidelity: 1.0 - fidelity,
            storage_bytes: storage,
            compression_ratio: dense_size as f64 / storage as f64,
        });
    }

    // Find optimal χ for various fidelity targets
    let mut optimal_chi = HashMap::new();
    for target in ["0.99", "0.999", "0.9999", "0.99999", "1.0-1e-10"] {
        let threshold = match target {
            "0.99" => 0.99,
            "0.999" => 0.999,
            "0.9999" => 0.9999,
            "0.99999" => 0.99999,
            "1.0-1e-10" => 1.0 - 1e-10,
            _ => 0.99,
        };

        if let Some(point) = points.iter().find(|p| p.fidelity >= threshold) {
            optimal_chi.insert(target.to_string(), point.bond_dim);
        }
    }

    Ok(BenchmarkResult {
        num_qubits: n,
        state_type: state_type.to_string(),
        points,
        optimal_chi_for_fidelity: optimal_chi,
    })
}

/// Generate standard benchmark χ values for a given number of qubits
pub fn standard_chi_values(n_qubits: usize) -> Vec<usize> {
    let max_chi = 1usize << (n_qubits / 2);
    let mut values = vec![1, 2, 4, 8, 16, 32, 64, 128, 256];
    values.retain(|&x| x <= max_chi);
    values
}

// =============================================================================
// Convenience Functions
// =============================================================================

/// Find optimal bond dimension for a state with default settings
pub fn find_optimal_bond_dim(
    state: &QuantumStateVector,
    target_fidelity: f64,
) -> Result<usize, OptimizationError> {
    let config = OptimizationConfig {
        target_fidelity,
        ..Default::default()
    };
    let mut optimizer = BondDimensionOptimizer::new(config);
    let result = optimizer.optimize(state)?;
    Ok(result.optimal_bond_dim)
}

/// Quick estimate of required bond dimension based on entanglement
pub fn estimate_required_bond_dim(state: &QuantumStateVector) -> usize {
    let analyzer = EntanglementAnalyzer::new();
    let max_entropy = analyzer.estimate_max_entanglement_entropy(state);
    let estimated = (2.0_f64.powf(max_entropy)).ceil() as usize;
    estimated.max(1)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_product_state_optimization() {
        // Product state should need χ=1
        let state = QuantumStateVector::zero_state(4);

        let config = OptimizationConfig {
            target_fidelity: 0.9999,
            ..Default::default()
        };
        let mut optimizer = BondDimensionOptimizer::new(config);
        let result = optimizer.optimize(&state).unwrap();

        assert_eq!(result.optimal_bond_dim, 1);
        assert!(result.achieved_fidelity >= 0.9999);
    }

    #[test]
    fn test_entanglement_analyzer() {
        let analyzer = EntanglementAnalyzer::new();

        // Product state should have zero entropy
        let product = QuantumStateVector::zero_state(3);
        let entropy = analyzer.estimate_max_entanglement_entropy(&product);
        assert!(entropy < 0.01);

        assert!(analyzer.is_product_state(&product, 0.1));
    }

    #[test]
    fn test_fidelity_computation() {
        let state1 = QuantumStateVector::zero_state(2);
        let state2 = QuantumStateVector::zero_state(2);

        let fidelity = compute_fidelity(&state1, &state2);
        assert!((fidelity - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_entanglement_classification() {
        let analyzer = EntanglementAnalyzer::new();

        let product = QuantumStateVector::zero_state(4);
        assert_eq!(
            analyzer.classify_entanglement(&product),
            EntanglementClass::Product
        );
    }

    #[test]
    fn test_benchmark() {
        let state = QuantumStateVector::zero_state(3);
        let chi_values = vec![1, 2, 4];

        let result = benchmark_bond_dimensions(&state, "zero_state", &chi_values).unwrap();

        assert_eq!(result.num_qubits, 3);
        assert_eq!(result.points.len(), 3);
        assert!(result.points[0].fidelity >= 0.99);
    }

    #[test]
    fn test_standard_chi_values() {
        let values = standard_chi_values(10);
        assert!(values.contains(&1));
        assert!(values.contains(&32));
        assert!(!values.contains(&256)); // 2^5 = 32 is max for 10 qubits
    }
}
