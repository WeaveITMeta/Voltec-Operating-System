// =============================================================================
// V-OS - State Backend
// =============================================================================
// Table of Contents:
//   1. QuantumStateBackendInterface - Trait for state representations
//   2. QuantumStateVector (FullStateVectorRepresentation)
//   3. MatrixProductStateRepresentation (MPS backend)
//   4. MatrixProductStateConfiguration
// =============================================================================
// Purpose: Provides quantum state representations with different trade-offs.
//          Dense state vector for small systems, MPS for larger systems.
// =============================================================================

use num_complex::Complex64;
use serde::{Deserialize, Serialize};

// =============================================================================
// 1. QuantumStateBackendInterface - Common interface for all backends
// =============================================================================

pub trait QuantumStateBackendInterface: Send + Sync {
    fn number_of_quantum_bits(&self) -> usize;
    fn reset_to_zero_state(&mut self);
    fn probability_of_bitstring(&self, bitstring: &[u8]) -> f64;
    fn measure_all(&self) -> Vec<u8>;
    fn clone_state(&self) -> Box<dyn QuantumStateBackendInterface>;
}

// =============================================================================
// 2. QuantumStateVector - Full state vector representation
// =============================================================================

#[derive(Debug, Clone)]
pub struct QuantumStateVector {
    amplitudes: Vec<Complex64>,
    number_of_quantum_bits: usize,
}

impl QuantumStateVector {
    pub fn zero_state(number_of_quantum_bits: usize) -> Self {
        let dimension = 1usize << number_of_quantum_bits;
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dimension];
        amplitudes[0] = Complex64::new(1.0, 0.0);
        Self {
            amplitudes,
            number_of_quantum_bits,
        }
    }

    pub fn one_state(number_of_quantum_bits: usize) -> Self {
        let dimension = 1usize << number_of_quantum_bits;
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dimension];
        amplitudes[dimension - 1] = Complex64::new(1.0, 0.0);
        Self {
            amplitudes,
            number_of_quantum_bits,
        }
    }

    pub fn from_amplitudes(amplitudes: Vec<Complex64>) -> Self {
        let dimension = amplitudes.len();
        assert!(
            dimension.is_power_of_two(),
            "Amplitude vector length must be a power of 2"
        );
        let number_of_quantum_bits = dimension.trailing_zeros() as usize;
        Self {
            amplitudes,
            number_of_quantum_bits,
        }
    }

    pub fn number_of_quantum_bits(&self) -> usize {
        self.number_of_quantum_bits
    }

    pub fn dimension(&self) -> usize {
        self.amplitudes.len()
    }

    pub fn amplitude(&self, index: usize) -> Complex64 {
        self.amplitudes[index]
    }

    pub fn set_amplitude(&mut self, index: usize, value: Complex64) {
        self.amplitudes[index] = value;
    }

    pub fn swap_amplitudes(&mut self, i: usize, j: usize) {
        self.amplitudes.swap(i, j);
    }

    pub fn amplitudes(&self) -> &[Complex64] {
        &self.amplitudes
    }

    pub fn amplitudes_mut(&mut self) -> &mut [Complex64] {
        &mut self.amplitudes
    }

    pub fn normalize(&mut self) {
        let norm: f64 = self.amplitudes.iter().map(|a| a.norm_sqr()).sum::<f64>().sqrt();
        if norm > 1e-15 {
            for amp in &mut self.amplitudes {
                *amp /= norm;
            }
        }
    }

    pub fn probability_distribution(&self) -> Vec<f64> {
        self.amplitudes.iter().map(|a| a.norm_sqr()).collect()
    }

    pub fn measure_all_deterministic(&self, random_value: f64) -> Vec<u8> {
        let probs = self.probability_distribution();
        let mut cumulative = 0.0;
        for (i, &prob) in probs.iter().enumerate() {
            cumulative += prob;
            if random_value < cumulative {
                return self.index_to_bitstring(i);
            }
        }
        self.index_to_bitstring(self.dimension() - 1)
    }

    fn index_to_bitstring(&self, index: usize) -> Vec<u8> {
        (0..self.number_of_quantum_bits)
            .rev()
            .map(|bit| ((index >> bit) & 1) as u8)
            .collect()
    }

    pub fn inner_product(&self, other: &Self) -> Complex64 {
        assert_eq!(self.dimension(), other.dimension());
        self.amplitudes
            .iter()
            .zip(other.amplitudes.iter())
            .map(|(a, b)| a.conj() * b)
            .sum()
    }

    pub fn expectation_value_pauli_z(&self, qubit: usize) -> f64 {
        let n = self.number_of_quantum_bits;
        let bit = n - 1 - qubit;
        let mask = 1usize << bit;

        let mut expectation = 0.0;
        for (i, amp) in self.amplitudes.iter().enumerate() {
            let prob = amp.norm_sqr();
            if (i & mask) == 0 {
                expectation += prob;
            } else {
                expectation -= prob;
            }
        }
        expectation
    }
}

impl QuantumStateBackendInterface for QuantumStateVector {
    fn number_of_quantum_bits(&self) -> usize {
        self.number_of_quantum_bits
    }

    fn reset_to_zero_state(&mut self) {
        for amp in &mut self.amplitudes {
            *amp = Complex64::new(0.0, 0.0);
        }
        self.amplitudes[0] = Complex64::new(1.0, 0.0);
    }

    fn probability_of_bitstring(&self, bitstring: &[u8]) -> f64 {
        let index: usize = bitstring
            .iter()
            .fold(0, |acc, &bit| (acc << 1) | (bit as usize));
        self.amplitudes[index].norm_sqr()
    }

    fn measure_all(&self) -> Vec<u8> {
        let random_value: f64 = rand_simple();
        self.measure_all_deterministic(random_value)
    }

    fn clone_state(&self) -> Box<dyn QuantumStateBackendInterface> {
        Box::new(self.clone())
    }
}

fn rand_simple() -> f64 {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    ((seed as u64).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) % 1000000)
        as f64
        / 1000000.0
}

// =============================================================================
// 3. MatrixProductStateRepresentation - Tensor network backend
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixProductStateConfiguration {
    pub maximum_bond_dimension: usize,
    pub truncation_threshold: f64,
}

impl Default for MatrixProductStateConfiguration {
    fn default() -> Self {
        Self {
            maximum_bond_dimension: 64,
            truncation_threshold: 1e-8,
        }
    }
}

impl MatrixProductStateConfiguration {
    pub fn new(maximum_bond_dimension: usize, truncation_threshold: f64) -> Self {
        Self {
            maximum_bond_dimension,
            truncation_threshold,
        }
    }

    pub fn high_accuracy() -> Self {
        Self {
            maximum_bond_dimension: 256,
            truncation_threshold: 1e-12,
        }
    }

    pub fn low_memory() -> Self {
        Self {
            maximum_bond_dimension: 16,
            truncation_threshold: 1e-6,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MatrixProductStateRepresentation {
    number_of_quantum_bits: usize,
    configuration: MatrixProductStateConfiguration,
    tensors: Vec<MpsTensor>,
}

#[derive(Debug, Clone)]
struct MpsTensor {
    data: Vec<Complex64>,
    left_bond_dim: usize,
    physical_dim: usize,
    right_bond_dim: usize,
}

impl MpsTensor {
    fn new(left_bond: usize, physical: usize, right_bond: usize) -> Self {
        let size = left_bond * physical * right_bond;
        Self {
            data: vec![Complex64::new(0.0, 0.0); size],
            left_bond_dim: left_bond,
            physical_dim: physical,
            right_bond_dim: right_bond,
        }
    }

    fn get(&self, left: usize, phys: usize, right: usize) -> Complex64 {
        let idx = left * self.physical_dim * self.right_bond_dim + phys * self.right_bond_dim + right;
        self.data[idx]
    }

    fn set(&mut self, left: usize, phys: usize, right: usize, value: Complex64) {
        let idx = left * self.physical_dim * self.right_bond_dim + phys * self.right_bond_dim + right;
        self.data[idx] = value;
    }
}

impl MatrixProductStateRepresentation {
    pub fn zero_state(number_of_quantum_bits: usize, config: MatrixProductStateConfiguration) -> Self {
        let mut tensors = Vec::with_capacity(number_of_quantum_bits);
        for i in 0..number_of_quantum_bits {
            let left_bond = if i == 0 { 1 } else { 1 };
            let right_bond = if i == number_of_quantum_bits - 1 { 1 } else { 1 };
            let mut tensor = MpsTensor::new(left_bond, 2, right_bond);
            tensor.set(0, 0, 0, Complex64::new(1.0, 0.0));
            tensors.push(tensor);
        }
        Self {
            number_of_quantum_bits,
            configuration: config,
            tensors,
        }
    }

    pub fn number_of_quantum_bits(&self) -> usize {
        self.number_of_quantum_bits
    }

    pub fn configuration(&self) -> &MatrixProductStateConfiguration {
        &self.configuration
    }

    pub fn maximum_bond_dimension(&self) -> usize {
        self.tensors
            .iter()
            .map(|t| t.right_bond_dim.max(t.left_bond_dim))
            .max()
            .unwrap_or(1)
    }

    pub fn to_full_state_vector(&self) -> QuantumStateVector {
        let dim = 1usize << self.number_of_quantum_bits;
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dim];

        for basis_state in 0..dim {
            let mut result = Complex64::new(1.0, 0.0);
            let mut left_vec = vec![Complex64::new(1.0, 0.0)];

            for (site, tensor) in self.tensors.iter().enumerate() {
                let bit = (basis_state >> (self.number_of_quantum_bits - 1 - site)) & 1;
                let mut new_left = vec![Complex64::new(0.0, 0.0); tensor.right_bond_dim];

                for left_idx in 0..tensor.left_bond_dim {
                    for right_idx in 0..tensor.right_bond_dim {
                        new_left[right_idx] += left_vec[left_idx] * tensor.get(left_idx, bit, right_idx);
                    }
                }
                left_vec = new_left;
            }
            amplitudes[basis_state] = left_vec[0];
        }

        QuantumStateVector::from_amplitudes(amplitudes)
    }
}

impl QuantumStateBackendInterface for MatrixProductStateRepresentation {
    fn number_of_quantum_bits(&self) -> usize {
        self.number_of_quantum_bits
    }

    fn reset_to_zero_state(&mut self) {
        *self = Self::zero_state(self.number_of_quantum_bits, self.configuration.clone());
    }

    fn probability_of_bitstring(&self, bitstring: &[u8]) -> f64 {
        let full_state = self.to_full_state_vector();
        full_state.probability_of_bitstring(bitstring)
    }

    fn measure_all(&self) -> Vec<u8> {
        let full_state = self.to_full_state_vector();
        full_state.measure_all()
    }

    fn clone_state(&self) -> Box<dyn QuantumStateBackendInterface> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_state_initialization() {
        let state = QuantumStateVector::zero_state(2);
        assert_eq!(state.number_of_quantum_bits(), 2);
        assert_eq!(state.dimension(), 4);
        assert!((state.amplitude(0).re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_probability_distribution() {
        let state = QuantumStateVector::zero_state(1);
        let probs = state.probability_distribution();
        assert!((probs[0] - 1.0).abs() < 1e-10);
        assert!((probs[1] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_mps_zero_state() {
        let config = MatrixProductStateConfiguration::default();
        let mps = MatrixProductStateRepresentation::zero_state(2, config);
        let full = mps.to_full_state_vector();
        assert!((full.amplitude(0).re - 1.0).abs() < 1e-10);
    }
}
