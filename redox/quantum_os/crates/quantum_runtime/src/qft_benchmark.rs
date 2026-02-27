// =============================================================================
// V-OS - QFT Format Benchmarking Suite
// =============================================================================
// Table of Contents:
//   1. Reproducible State Generators - Deterministic quantum state creation
//   2. Fidelity Benchmarks - Bond dimension vs fidelity analysis
//   3. Storage Benchmarks - Compression and I/O performance
//   4. Error Correction Benchmarks - Golay performance under noise
//   5. Entanglement Analysis - Entropy and correlation metrics
//   6. Benchmark Runner - Automated benchmark execution
// =============================================================================
// Purpose: Provides reproducible benchmarking utilities for validating the
//          .qft file format. Addresses peer review concerns about benchmark
//          validation and reproducibility.
// =============================================================================
// Usage: Run benchmarks with `cargo test --release -- --nocapture benchmark_`
//        or use the BenchmarkRunner for comprehensive analysis.
// =============================================================================

use crate::bond_optimization::{
    compute_fidelity, compute_infidelity, EntanglementAnalyzer, EntanglementClass,
};
use crate::golay::{decode_golay, encode_golay, GolayErrorStats};
use crate::golay_quantum::{GolayNoiseModel, QuantumGolayCode};
use crate::qft_format::{
    convert_dense_to_mps, convert_mps_to_dense, QftStorageConfig, SerializableMpsTensor,
};
use crate::state_backend::{MatrixProductStateConfiguration, QuantumStateVector};
use num_complex::Complex64;
use std::collections::HashMap;
use std::f64::consts::PI;

// =============================================================================
// 1. Reproducible State Generators
// =============================================================================

/// Deterministic random number generator for reproducible benchmarks
/// Uses xorshift128+ algorithm
#[derive(Debug, Clone)]
pub struct DeterministicRng {
    state: [u64; 2],
}

impl DeterministicRng {
    /// Create RNG with given seed
    pub fn new(seed: u64) -> Self {
        Self {
            state: [seed, seed.wrapping_mul(0x9E3779B97F4A7C15)],
        }
    }

    /// Generate next u64
    pub fn next_u64(&mut self) -> u64 {
        let mut s1 = self.state[0];
        let s0 = self.state[1];
        let result = s0.wrapping_add(s1);
        self.state[0] = s0;
        s1 ^= s1 << 23;
        self.state[1] = s1 ^ s0 ^ (s1 >> 18) ^ (s0 >> 5);
        result
    }

    /// Generate f64 in [0, 1)
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Generate complex number with unit norm
    pub fn next_unit_complex(&mut self) -> Complex64 {
        let theta = self.next_f64() * 2.0 * PI;
        Complex64::new(theta.cos(), theta.sin())
    }
}

/// State generator for reproducible benchmark states
#[derive(Debug, Clone)]
pub struct StateGenerator {
    rng: DeterministicRng,
}

impl StateGenerator {
    /// Create generator with seed
    pub fn new(seed: u64) -> Self {
        Self {
            rng: DeterministicRng::new(seed),
        }
    }

    /// Generate |0...0⟩ product state
    pub fn product_zero_state(&self, n_qubits: usize) -> QuantumStateVector {
        QuantumStateVector::zero_state(n_qubits)
    }

    /// Generate |+...+⟩ product state (equal superposition)
    pub fn product_plus_state(&self, n_qubits: usize) -> QuantumStateVector {
        let dim = 1 << n_qubits;
        let norm = 1.0 / (dim as f64).sqrt();
        let amplitudes: Vec<Complex64> = (0..dim)
            .map(|_| Complex64::new(norm, 0.0))
            .collect();
        QuantumStateVector::from_amplitudes(amplitudes)
    }

    /// Generate GHZ state (|0...0⟩ + |1...1⟩) / √2
    pub fn ghz_state(&self, n_qubits: usize) -> QuantumStateVector {
        let dim = 1 << n_qubits;
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dim];
        let norm = 1.0 / 2.0_f64.sqrt();
        amplitudes[0] = Complex64::new(norm, 0.0);
        amplitudes[dim - 1] = Complex64::new(norm, 0.0);
        QuantumStateVector::from_amplitudes(amplitudes)
    }

    /// Generate W state (|100...0⟩ + |010...0⟩ + ... + |000...1⟩) / √n
    pub fn w_state(&self, n_qubits: usize) -> QuantumStateVector {
        let dim = 1 << n_qubits;
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dim];
        let norm = 1.0 / (n_qubits as f64).sqrt();

        for i in 0..n_qubits {
            let idx = 1 << (n_qubits - 1 - i);
            amplitudes[idx] = Complex64::new(norm, 0.0);
        }

        QuantumStateVector::from_amplitudes(amplitudes)
    }

    /// Generate random Haar-distributed state (reproducible with seed)
    pub fn random_haar_state(&mut self, n_qubits: usize) -> QuantumStateVector {
        let dim = 1 << n_qubits;
        let mut amplitudes: Vec<Complex64> = (0..dim)
            .map(|_| {
                // Box-Muller transform for Gaussian
                let u1 = self.rng.next_f64().max(1e-15);
                let u2 = self.rng.next_f64();
                let r = (-2.0 * u1.ln()).sqrt();
                let theta = 2.0 * PI * u2;
                Complex64::new(r * theta.cos(), r * theta.sin())
            })
            .collect();

        // Normalize
        let norm: f64 = amplitudes.iter().map(|a| a.norm_sqr()).sum::<f64>().sqrt();
        for a in &mut amplitudes {
            *a /= norm;
        }

        QuantumStateVector::from_amplitudes(amplitudes)
    }

    /// Generate cluster state (1D linear cluster)
    pub fn cluster_state_1d(&self, n_qubits: usize) -> QuantumStateVector {
        // Start with |+⟩^⊗n
        let dim = 1 << n_qubits;
        let mut amplitudes = vec![Complex64::new(1.0 / (dim as f64).sqrt(), 0.0); dim];

        // Apply CZ gates between neighbors
        for i in 0..(n_qubits - 1) {
            for idx in 0..dim {
                let bit_i = (idx >> (n_qubits - 1 - i)) & 1;
                let bit_j = (idx >> (n_qubits - 2 - i)) & 1;
                if bit_i == 1 && bit_j == 1 {
                    amplitudes[idx] *= Complex64::new(-1.0, 0.0);
                }
            }
        }

        QuantumStateVector::from_amplitudes(amplitudes)
    }

    /// Generate state with specified entanglement entropy at middle cut
    pub fn state_with_entropy(&mut self, n_qubits: usize, target_entropy: f64) -> QuantumStateVector {
        // Use Schmidt decomposition to create state with target entropy
        // For simplicity, create superposition of product states
        let dim = 1 << n_qubits;
        let half = n_qubits / 2;

        // Number of Schmidt coefficients based on entropy
        // S = -Σ λ² log(λ²), for uniform: S = log(k) where k = number of terms
        let k = (2.0_f64.powf(target_entropy)).ceil() as usize;
        let k = k.min(1 << half).max(1);

        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dim];
        let norm = 1.0 / (k as f64).sqrt();

        for i in 0..k {
            // |i⟩_L ⊗ |i⟩_R
            let left_idx = i;
            let right_idx = i;
            let full_idx = (left_idx << (n_qubits - half)) | right_idx;
            amplitudes[full_idx] = Complex64::new(norm, 0.0);
        }

        QuantumStateVector::from_amplitudes(amplitudes)
    }
}

// =============================================================================
// 2. Fidelity Benchmarks
// =============================================================================

/// Single benchmark data point
#[derive(Debug, Clone)]
pub struct FidelityBenchmarkPoint {
    pub bond_dim: usize,
    pub fidelity: f64,
    pub infidelity: f64,
    pub truncation_error: f64,
    pub max_singular_value_discarded: f64,
}

/// Complete fidelity benchmark result
#[derive(Debug, Clone)]
pub struct FidelityBenchmarkResult {
    pub n_qubits: usize,
    pub state_type: String,
    pub seed: u64,
    pub entanglement_entropy: f64,
    pub entanglement_class: EntanglementClass,
    pub points: Vec<FidelityBenchmarkPoint>,
    pub optimal_chi_99: Option<usize>,
    pub optimal_chi_999: Option<usize>,
    pub optimal_chi_9999: Option<usize>,
    pub optimal_chi_exact: Option<usize>,
}

/// Run fidelity benchmark for a state across bond dimensions
pub fn benchmark_fidelity(
    state: &QuantumStateVector,
    state_type: &str,
    seed: u64,
    chi_values: &[usize],
) -> FidelityBenchmarkResult {
    let n = state.number_of_quantum_bits();
    let analyzer = EntanglementAnalyzer::new();
    let entropy = analyzer.estimate_max_entanglement_entropy(state);
    let class = analyzer.classify_entanglement(state);

    let mut points = Vec::with_capacity(chi_values.len());

    for &chi in chi_values {
        let config = MatrixProductStateConfiguration {
            maximum_bond_dimension: chi,
            truncation_threshold: 1e-15, // Very small to isolate χ effect
        };

        match convert_dense_to_mps(state, &config) {
            Ok(tensors) => {
                match convert_mps_to_dense(&tensors) {
                    Ok(reconstructed) => {
                        let fidelity = compute_fidelity(state, &reconstructed);
                        let infidelity = 1.0 - fidelity;

                        points.push(FidelityBenchmarkPoint {
                            bond_dim: chi,
                            fidelity,
                            infidelity,
                            truncation_error: infidelity.sqrt(),
                            max_singular_value_discarded: 0.0, // Would need SVD info
                        });
                    }
                    Err(_) => continue,
                }
            }
            Err(_) => continue,
        }
    }

    // Find optimal χ for various fidelity targets
    let optimal_chi_99 = points.iter().find(|p| p.fidelity >= 0.99).map(|p| p.bond_dim);
    let optimal_chi_999 = points.iter().find(|p| p.fidelity >= 0.999).map(|p| p.bond_dim);
    let optimal_chi_9999 = points.iter().find(|p| p.fidelity >= 0.9999).map(|p| p.bond_dim);
    let optimal_chi_exact = points
        .iter()
        .find(|p| p.fidelity >= 1.0 - 1e-10)
        .map(|p| p.bond_dim);

    FidelityBenchmarkResult {
        n_qubits: n,
        state_type: state_type.to_string(),
        seed,
        entanglement_entropy: entropy,
        entanglement_class: class,
        points,
        optimal_chi_99,
        optimal_chi_999,
        optimal_chi_9999,
        optimal_chi_exact,
    }
}

// =============================================================================
// 3. Storage Benchmarks
// =============================================================================

/// Storage benchmark result
#[derive(Debug, Clone)]
pub struct StorageBenchmarkResult {
    pub n_qubits: usize,
    pub bond_dim: usize,
    pub dense_size_bytes: usize,
    pub mps_size_bytes: usize,
    pub golay_encoded_size_bytes: usize,
    pub compression_ratio_mps: f64,
    pub compression_ratio_golay: f64,
    pub golay_overhead: f64,
}

/// Benchmark storage sizes
pub fn benchmark_storage(n_qubits: usize, bond_dim: usize) -> StorageBenchmarkResult {
    let dense_size = (1usize << n_qubits) * 16; // Complex64 = 16 bytes

    // MPS size: n tensors of shape [χ, 2, χ] (boundary tensors smaller)
    // Simplified: assume all tensors are [χ, 2, χ]
    let mps_size = n_qubits * bond_dim * 2 * bond_dim * 16;

    // Golay overhead: 23/12 ≈ 1.917
    let golay_encoded_size = (mps_size * 23 + 11) / 12;

    StorageBenchmarkResult {
        n_qubits,
        bond_dim,
        dense_size_bytes: dense_size,
        mps_size_bytes: mps_size,
        golay_encoded_size_bytes: golay_encoded_size,
        compression_ratio_mps: dense_size as f64 / mps_size as f64,
        compression_ratio_golay: dense_size as f64 / golay_encoded_size as f64,
        golay_overhead: golay_encoded_size as f64 / mps_size as f64,
    }
}

// =============================================================================
// 4. Error Correction Benchmarks
// =============================================================================

/// Golay error correction benchmark result
#[derive(Debug, Clone)]
pub struct GolayBenchmarkResult {
    pub bit_error_rate: f64,
    pub num_blocks: usize,
    pub blocks_with_errors: usize,
    pub total_errors_injected: usize,
    pub total_errors_corrected: usize,
    pub uncorrectable_blocks: usize,
    pub recovery_rate: f64,
    pub data_integrity: bool,
}

/// Benchmark Golay error correction under various error rates
pub fn benchmark_golay_correction(
    data_size: usize,
    bit_error_rate: f64,
    seed: u64,
) -> GolayBenchmarkResult {
    let mut rng = DeterministicRng::new(seed);

    // Generate random data
    let original: Vec<u8> = (0..data_size).map(|_| rng.next_u64() as u8).collect();

    // Encode
    let encoded = match encode_golay(&original) {
        Ok(e) => e,
        Err(_) => {
            return GolayBenchmarkResult {
                bit_error_rate,
                num_blocks: 0,
                blocks_with_errors: 0,
                total_errors_injected: 0,
                total_errors_corrected: 0,
                uncorrectable_blocks: 0,
                recovery_rate: 0.0,
                data_integrity: false,
            }
        }
    };

    // Inject errors
    let mut corrupted = encoded.clone();
    let mut total_errors = 0;
    let mut bytes_with_errors = 0;

    for byte in &mut corrupted {
        let mut byte_errors = 0;
        for bit in 0..8 {
            if rng.next_f64() < bit_error_rate {
                *byte ^= 1 << bit;
                byte_errors += 1;
            }
        }
        if byte_errors > 0 {
            bytes_with_errors += 1;
            total_errors += byte_errors;
        }
    }

    // Decode
    let (decoded, stats) = match decode_golay(&corrupted) {
        Ok(result) => result,
        Err(_) => {
            return GolayBenchmarkResult {
                bit_error_rate,
                num_blocks: encoded.len() * 8 / 23,
                blocks_with_errors: bytes_with_errors,
                total_errors_injected: total_errors,
                total_errors_corrected: 0,
                uncorrectable_blocks: 1,
                recovery_rate: 0.0,
                data_integrity: false,
            }
        }
    };

    // Check integrity
    let data_integrity = decoded.len() >= original.len()
        && decoded[..original.len()] == original[..];

    GolayBenchmarkResult {
        bit_error_rate,
        num_blocks: stats.blocks_processed as usize,
        blocks_with_errors: (stats.single_bit_errors
            + stats.double_bit_errors
            + stats.triple_bit_errors) as usize,
        total_errors_injected: total_errors,
        total_errors_corrected: stats.total_errors_corrected as usize,
        uncorrectable_blocks: stats.uncorrectable_errors as usize,
        recovery_rate: stats.recovery_rate(),
        data_integrity,
    }
}

/// Benchmark quantum Golay code under noise
pub fn benchmark_quantum_golay(
    physical_error_rate: f64,
    num_samples: usize,
    seed: u64,
) -> QuantumGolayBenchmarkResult {
    let code = QuantumGolayCode::new();
    let noise = GolayNoiseModel::with_error_rate(physical_error_rate);

    let mut logical_errors = 0;
    let mut decoding_failures = 0;
    let mut total_x_weight = 0usize;
    let mut total_z_weight = 0usize;

    for i in 0..num_samples {
        let (x_err, z_err) = noise.sample_errors(seed + i as u64);
        total_x_weight += x_err.count_ones() as usize;
        total_z_weight += z_err.count_ones() as usize;

        let x_syndrome = code.compute_z_syndrome(x_err);
        let z_syndrome = code.compute_x_syndrome(z_err);

        match code.decode_errors(x_syndrome, z_syndrome) {
            Ok((decoded_x, decoded_z)) => {
                let residual_x = x_err ^ decoded_x;
                let residual_z = z_err ^ decoded_z;

                let logical_x_error = (residual_x & code.logical_z).count_ones() % 2 == 1;
                let logical_z_error = (residual_z & code.logical_x).count_ones() % 2 == 1;

                if logical_x_error || logical_z_error {
                    logical_errors += 1;
                }
            }
            Err(_) => {
                decoding_failures += 1;
                logical_errors += 1;
            }
        }
    }

    QuantumGolayBenchmarkResult {
        physical_error_rate,
        num_samples,
        logical_error_rate: logical_errors as f64 / num_samples as f64,
        decoding_failure_rate: decoding_failures as f64 / num_samples as f64,
        avg_x_error_weight: total_x_weight as f64 / num_samples as f64,
        avg_z_error_weight: total_z_weight as f64 / num_samples as f64,
        suppression_factor: physical_error_rate / (logical_errors as f64 / num_samples as f64).max(1e-10),
    }
}

/// Quantum Golay benchmark result
#[derive(Debug, Clone)]
pub struct QuantumGolayBenchmarkResult {
    pub physical_error_rate: f64,
    pub num_samples: usize,
    pub logical_error_rate: f64,
    pub decoding_failure_rate: f64,
    pub avg_x_error_weight: f64,
    pub avg_z_error_weight: f64,
    pub suppression_factor: f64,
}

// =============================================================================
// 5. Comprehensive Benchmark Runner
// =============================================================================

/// Complete benchmark suite results
#[derive(Debug, Clone)]
pub struct BenchmarkSuiteResults {
    pub fidelity_results: Vec<FidelityBenchmarkResult>,
    pub storage_results: Vec<StorageBenchmarkResult>,
    pub golay_results: Vec<GolayBenchmarkResult>,
    pub quantum_golay_results: Vec<QuantumGolayBenchmarkResult>,
    pub summary: BenchmarkSummary,
}

/// Summary statistics
#[derive(Debug, Clone)]
pub struct BenchmarkSummary {
    pub total_states_tested: usize,
    pub qubit_range: (usize, usize),
    pub chi_range: (usize, usize),
    pub avg_compression_ratio: f64,
    pub golay_recovery_rate: f64,
    pub quantum_threshold_estimate: f64,
}

/// Run comprehensive benchmark suite
pub fn run_benchmark_suite(config: BenchmarkConfig) -> BenchmarkSuiteResults {
    let mut fidelity_results = Vec::new();
    let mut storage_results = Vec::new();
    let mut golay_results = Vec::new();
    let mut quantum_golay_results = Vec::new();

    let chi_values: Vec<usize> = (0..=8).map(|i| 1 << i).collect(); // 1, 2, 4, ..., 256

    // Fidelity benchmarks for each qubit count
    for n in config.min_qubits..=config.max_qubits {
        let mut gen = StateGenerator::new(config.seed);

        // Product state
        let product = gen.product_zero_state(n);
        fidelity_results.push(benchmark_fidelity(&product, "product", config.seed, &chi_values));

        // GHZ state
        let ghz = gen.ghz_state(n);
        fidelity_results.push(benchmark_fidelity(&ghz, "ghz", config.seed, &chi_values));

        // W state
        if n <= 16 {
            let w = gen.w_state(n);
            fidelity_results.push(benchmark_fidelity(&w, "w", config.seed, &chi_values));
        }

        // Random states (multiple seeds for statistics)
        for seed_offset in 0..config.random_state_samples {
            let mut gen2 = StateGenerator::new(config.seed + seed_offset as u64);
            let random = gen2.random_haar_state(n);
            fidelity_results.push(benchmark_fidelity(
                &random,
                "random",
                config.seed + seed_offset as u64,
                &chi_values,
            ));
        }

        // Storage benchmarks
        for &chi in &chi_values {
            if chi <= (1 << (n / 2)) {
                storage_results.push(benchmark_storage(n, chi));
            }
        }
    }

    // Golay error correction benchmarks
    for &ber in &[0.001, 0.005, 0.01, 0.05, 0.1] {
        golay_results.push(benchmark_golay_correction(1000, ber, config.seed));
    }

    // Quantum Golay benchmarks
    for &per in &[1e-4, 1e-3, 5e-3, 1e-2, 2e-2] {
        quantum_golay_results.push(benchmark_quantum_golay(per, config.quantum_samples, config.seed));
    }

    // Compute summary
    let avg_compression = storage_results
        .iter()
        .map(|r| r.compression_ratio_mps)
        .sum::<f64>()
        / storage_results.len().max(1) as f64;

    let avg_golay_recovery = golay_results
        .iter()
        .map(|r| r.recovery_rate)
        .sum::<f64>()
        / golay_results.len().max(1) as f64;

    // Estimate threshold from quantum Golay results
    let threshold_estimate = quantum_golay_results
        .iter()
        .filter(|r| r.logical_error_rate < r.physical_error_rate)
        .map(|r| r.physical_error_rate)
        .fold(0.0, f64::max);

    let summary = BenchmarkSummary {
        total_states_tested: fidelity_results.len(),
        qubit_range: (config.min_qubits, config.max_qubits),
        chi_range: (*chi_values.first().unwrap_or(&1), *chi_values.last().unwrap_or(&256)),
        avg_compression_ratio: avg_compression,
        golay_recovery_rate: avg_golay_recovery,
        quantum_threshold_estimate: threshold_estimate,
    };

    BenchmarkSuiteResults {
        fidelity_results,
        storage_results,
        golay_results,
        quantum_golay_results,
        summary,
    }
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub min_qubits: usize,
    pub max_qubits: usize,
    pub seed: u64,
    pub random_state_samples: usize,
    pub quantum_samples: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            min_qubits: 4,
            max_qubits: 12,
            seed: 42,
            random_state_samples: 5,
            quantum_samples: 1000,
        }
    }
}

// =============================================================================
// 6. Report Generation
// =============================================================================

/// Generate markdown report from benchmark results
pub fn generate_benchmark_report(results: &BenchmarkSuiteResults) -> String {
    let mut report = String::new();

    report.push_str("# QFT Format Benchmark Report\n\n");
    report.push_str(&format!(
        "**Generated**: {}\n",
        chrono_lite_timestamp()
    ));
    report.push_str(&format!(
        "**Qubit Range**: {}-{}\n",
        results.summary.qubit_range.0, results.summary.qubit_range.1
    ));
    report.push_str(&format!(
        "**States Tested**: {}\n\n",
        results.summary.total_states_tested
    ));

    // Fidelity table
    report.push_str("## Bond Dimension vs Fidelity\n\n");
    report.push_str("| n | State Type | Entropy | χ=4 | χ=16 | χ=32 | χ=64 | χ_exact |\n");
    report.push_str("|---|------------|---------|-----|------|------|------|--------|\n");

    for result in &results.fidelity_results {
        let f4 = result.points.iter().find(|p| p.bond_dim == 4).map(|p| p.fidelity);
        let f16 = result.points.iter().find(|p| p.bond_dim == 16).map(|p| p.fidelity);
        let f32 = result.points.iter().find(|p| p.bond_dim == 32).map(|p| p.fidelity);
        let f64 = result.points.iter().find(|p| p.bond_dim == 64).map(|p| p.fidelity);

        report.push_str(&format!(
            "| {} | {} | {:.3} | {:.4} | {:.4} | {:.4} | {:.4} | {} |\n",
            result.n_qubits,
            result.state_type,
            result.entanglement_entropy,
            f4.map(|f| format!("{:.4}", f)).unwrap_or("-".to_string()),
            f16.map(|f| format!("{:.4}", f)).unwrap_or("-".to_string()),
            f32.map(|f| format!("{:.4}", f)).unwrap_or("-".to_string()),
            f64.map(|f| format!("{:.4}", f)).unwrap_or("-".to_string()),
            result.optimal_chi_exact.map(|c| c.to_string()).unwrap_or("-".to_string()),
        ));
    }

    // Golay table
    report.push_str("\n## Golay Error Correction Performance\n\n");
    report.push_str("| BER | Blocks | Errors | Corrected | Uncorrectable | Recovery |\n");
    report.push_str("|-----|--------|--------|-----------|---------------|----------|\n");

    for result in &results.golay_results {
        report.push_str(&format!(
            "| {:.1}% | {} | {} | {} | {} | {:.2}% |\n",
            result.bit_error_rate * 100.0,
            result.num_blocks,
            result.total_errors_injected,
            result.total_errors_corrected,
            result.uncorrectable_blocks,
            result.recovery_rate * 100.0,
        ));
    }

    // Quantum Golay table
    report.push_str("\n## Quantum Golay [[23,1,7]] Performance\n\n");
    report.push_str("| p_phys | p_logical | Suppression | Avg X wt | Avg Z wt |\n");
    report.push_str("|--------|-----------|-------------|----------|----------|\n");

    for result in &results.quantum_golay_results {
        report.push_str(&format!(
            "| {:.4} | {:.6} | {:.1}x | {:.2} | {:.2} |\n",
            result.physical_error_rate,
            result.logical_error_rate,
            result.suppression_factor,
            result.avg_x_error_weight,
            result.avg_z_error_weight,
        ));
    }

    // Summary
    report.push_str("\n## Summary\n\n");
    report.push_str(&format!(
        "- **Average MPS Compression**: {:.1}x\n",
        results.summary.avg_compression_ratio
    ));
    report.push_str(&format!(
        "- **Golay Recovery Rate**: {:.2}%\n",
        results.summary.golay_recovery_rate * 100.0
    ));
    report.push_str(&format!(
        "- **Quantum Threshold Estimate**: p ≈ {:.3}\n",
        results.summary.quantum_threshold_estimate
    ));

    report
}

/// Simple timestamp without chrono dependency
fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("Unix timestamp: {}", duration.as_secs())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_rng() {
        let mut rng1 = DeterministicRng::new(42);
        let mut rng2 = DeterministicRng::new(42);

        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn test_state_generator_reproducibility() {
        let mut gen1 = StateGenerator::new(123);
        let mut gen2 = StateGenerator::new(123);

        let state1 = gen1.random_haar_state(4);
        let state2 = gen2.random_haar_state(4);

        let fidelity = compute_fidelity(&state1, &state2);
        assert!((fidelity - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_ghz_state() {
        let gen = StateGenerator::new(0);
        let ghz = gen.ghz_state(3);

        // Should have only |000⟩ and |111⟩ amplitudes
        assert!((ghz.amplitude(0).norm() - 1.0 / 2.0_f64.sqrt()).abs() < 1e-10);
        assert!((ghz.amplitude(7).norm() - 1.0 / 2.0_f64.sqrt()).abs() < 1e-10);
        assert!(ghz.amplitude(1).norm() < 1e-10);
    }

    #[test]
    fn test_fidelity_benchmark() {
        let gen = StateGenerator::new(42);
        let state = gen.product_zero_state(4);

        let result = benchmark_fidelity(&state, "product", 42, &[1, 2, 4]);

        assert_eq!(result.n_qubits, 4);
        assert!(result.points[0].fidelity >= 0.99);
    }

    #[test]
    fn test_golay_benchmark() {
        let result = benchmark_golay_correction(100, 0.01, 42);

        assert!(result.recovery_rate >= 0.9);
    }

    #[test]
    fn test_benchmark_suite() {
        let config = BenchmarkConfig {
            min_qubits: 3,
            max_qubits: 4,
            seed: 42,
            random_state_samples: 1,
            quantum_samples: 100,
        };

        let results = run_benchmark_suite(config);

        assert!(!results.fidelity_results.is_empty());
        assert!(!results.golay_results.is_empty());
    }
}
