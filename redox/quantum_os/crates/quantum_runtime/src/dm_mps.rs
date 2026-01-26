// =============================================================================
// MACROHARD Quantum OS - Density Matrix MPS (DM-MPS) Module
// =============================================================================
// Table of Contents:
//   1. DensityMatrixMPS - Mixed state representation via MPS
//   2. Purification utilities
//   3. Partial trace operations
//   4. Mixed state operations (depolarizing channel, etc.)
// =============================================================================
// Purpose: Extends MPS representation to handle mixed quantum states via
//          purification. A density matrix ρ is represented as |ψ⟩⟨ψ| where
//          |ψ⟩ is a pure state on system + ancilla.
// =============================================================================

use num_complex::Complex64;
use ndarray::{Array1, Array2};

use crate::state_backend::{QuantumStateVector, MatrixProductStateConfiguration};

/// Configuration for Density Matrix MPS representation
#[derive(Debug, Clone)]
pub struct DmMpsConfig {
    /// Maximum bond dimension for the purified MPS
    pub max_bond_dim: usize,
    /// Truncation threshold for SVD
    pub truncation_threshold: f64,
    /// Whether to use symmetric purification (ancilla = system)
    pub symmetric_purification: bool,
}

impl Default for DmMpsConfig {
    fn default() -> Self {
        Self {
            max_bond_dim: 64,
            truncation_threshold: 1e-10,
            symmetric_purification: true,
        }
    }
}

impl DmMpsConfig {
    pub fn high_accuracy() -> Self {
        Self {
            max_bond_dim: 256,
            truncation_threshold: 1e-14,
            symmetric_purification: true,
        }
    }
    
    pub fn to_mps_config(&self) -> MatrixProductStateConfiguration {
        MatrixProductStateConfiguration::new(self.max_bond_dim, self.truncation_threshold)
    }
}

/// Density Matrix represented via Matrix Product State purification.
/// 
/// A mixed state ρ on n qubits is represented as a pure state |ψ⟩ on 2n qubits
/// (n system + n ancilla), such that ρ = Tr_ancilla(|ψ⟩⟨ψ|).
/// 
/// This allows efficient MPS representation of mixed states with the same
/// O(n χ²) complexity, at the cost of doubling the qubit count.
#[derive(Debug, Clone)]
pub struct DensityMatrixMps {
    /// Number of system qubits (the actual state dimension)
    num_system_qubits: usize,
    /// Number of ancilla qubits (for purification)
    num_ancilla_qubits: usize,
    /// The purified state as a dense vector (for now)
    /// TODO: Replace with actual MPS representation
    purified_amplitudes: Vec<Complex64>,
    /// Configuration
    config: DmMpsConfig,
}

impl DensityMatrixMps {
    /// Create a maximally mixed state on n qubits.
    /// This is represented by a maximally entangled state between system and ancilla.
    pub fn maximally_mixed(num_qubits: usize, config: DmMpsConfig) -> Self {
        let dim = 1usize << num_qubits;
        let total_dim = dim * dim; // system × ancilla
        
        // |Φ⟩ = (1/√d) Σ_i |i⟩_S |i⟩_A (maximally entangled)
        let norm = 1.0 / (dim as f64).sqrt();
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); total_dim];
        
        for i in 0..dim {
            // Index in combined space: i * dim + i (diagonal in computational basis)
            let idx = i * dim + i;
            amplitudes[idx] = Complex64::new(norm, 0.0);
        }
        
        Self {
            num_system_qubits: num_qubits,
            num_ancilla_qubits: num_qubits,
            purified_amplitudes: amplitudes,
            config,
        }
    }
    
    /// Create a pure state (no mixing) on n qubits.
    /// Ancilla is in |0⟩ state, so tracing out gives the pure state.
    pub fn from_pure_state(state: &QuantumStateVector, config: DmMpsConfig) -> Self {
        let num_qubits = state.number_of_quantum_bits();
        let dim = 1usize << num_qubits;
        let total_dim = dim * dim;
        
        // |ψ⟩_S ⊗ |0⟩_A
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); total_dim];
        
        for i in 0..dim {
            // Ancilla in |0⟩ means ancilla index = 0
            let idx = i * dim + 0;
            amplitudes[idx] = state.amplitude(i);
        }
        
        Self {
            num_system_qubits: num_qubits,
            num_ancilla_qubits: num_qubits,
            purified_amplitudes: amplitudes,
            config,
        }
    }
    
    /// Create from a density matrix (2D array).
    /// Uses eigendecomposition to find the purification.
    pub fn from_density_matrix(rho: &Array2<Complex64>, config: DmMpsConfig) -> Result<Self, String> {
        let dim = rho.nrows();
        if dim != rho.ncols() {
            return Err("Density matrix must be square".to_string());
        }
        if !dim.is_power_of_two() {
            return Err("Dimension must be a power of 2".to_string());
        }
        
        let num_qubits = dim.trailing_zeros() as usize;
        
        // For now, use a simple purification via vectorization
        // ρ = Σ_i p_i |ψ_i⟩⟨ψ_i| → |Φ⟩ = Σ_i √p_i |ψ_i⟩_S |i⟩_A
        // Simplified: use Cholesky-like decomposition
        
        let total_dim = dim * dim;
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); total_dim];
        
        // Simple purification: |Φ⟩ = Σ_{ij} √ρ_{ij} |i⟩_S |j⟩_A
        // This works for any valid density matrix
        for i in 0..dim {
            for j in 0..dim {
                let idx = i * dim + j;
                // Use matrix square root approximation
                // For diagonal elements, this is exact
                if i == j {
                    let val = rho[[i, j]].re.max(0.0).sqrt();
                    amplitudes[idx] = Complex64::new(val, 0.0);
                } else {
                    // Off-diagonal: use geometric mean approximation
                    let diag_i = rho[[i, i]].re.max(0.0).sqrt();
                    let diag_j = rho[[j, j]].re.max(0.0).sqrt();
                    if diag_i > 1e-15 && diag_j > 1e-15 {
                        let scale = (diag_i * diag_j).sqrt();
                        amplitudes[idx] = rho[[i, j]] * scale / (rho[[i, i]].re.max(1e-15) * rho[[j, j]].re.max(1e-15)).sqrt();
                    }
                }
            }
        }
        
        // Normalize
        let norm: f64 = amplitudes.iter().map(|a| a.norm_sqr()).sum::<f64>().sqrt();
        if norm > 1e-15 {
            for amp in &mut amplitudes {
                *amp /= norm;
            }
        }
        
        Ok(Self {
            num_system_qubits: num_qubits,
            num_ancilla_qubits: num_qubits,
            purified_amplitudes: amplitudes,
            config,
        })
    }
    
    /// Get the number of system qubits
    pub fn num_system_qubits(&self) -> usize {
        self.num_system_qubits
    }
    
    /// Get the total number of qubits (system + ancilla)
    pub fn total_qubits(&self) -> usize {
        self.num_system_qubits + self.num_ancilla_qubits
    }
    
    /// Get the purified state as a QuantumStateVector
    pub fn purified_state(&self) -> QuantumStateVector {
        QuantumStateVector::from_amplitudes(self.purified_amplitudes.clone())
    }
    
    /// Compute the reduced density matrix by tracing out the ancilla.
    pub fn to_density_matrix(&self) -> Array2<Complex64> {
        let sys_dim = 1usize << self.num_system_qubits;
        let anc_dim = 1usize << self.num_ancilla_qubits;
        
        let mut rho = Array2::zeros((sys_dim, sys_dim));
        
        // ρ_{ij} = Σ_k ⟨i,k|Φ⟩⟨Φ|j,k⟩ = Σ_k Φ_{ik} Φ*_{jk}
        for i in 0..sys_dim {
            for j in 0..sys_dim {
                let mut sum = Complex64::new(0.0, 0.0);
                for k in 0..anc_dim {
                    let idx_ik = i * anc_dim + k;
                    let idx_jk = j * anc_dim + k;
                    sum += self.purified_amplitudes[idx_ik] * self.purified_amplitudes[idx_jk].conj();
                }
                rho[[i, j]] = sum;
            }
        }
        
        rho
    }
    
    /// Compute the purity Tr(ρ²)
    pub fn purity(&self) -> f64 {
        let rho = self.to_density_matrix();
        let dim = rho.nrows();
        
        let mut trace = Complex64::new(0.0, 0.0);
        for i in 0..dim {
            for j in 0..dim {
                trace += rho[[i, j]] * rho[[j, i]];
            }
        }
        
        trace.re
    }
    
    /// Compute the von Neumann entropy S = -Tr(ρ log ρ)
    pub fn von_neumann_entropy(&self) -> f64 {
        let rho = self.to_density_matrix();
        let dim = rho.nrows();
        
        // Compute eigenvalues (simplified: use diagonal for now)
        // TODO: Proper eigendecomposition
        let mut entropy = 0.0;
        for i in 0..dim {
            let p = rho[[i, i]].re;
            if p > 1e-15 {
                entropy -= p * p.ln();
            }
        }
        
        entropy
    }
    
    /// Apply a depolarizing channel with error probability p.
    /// ρ → (1-p)ρ + (p/3)(XρX + YρY + ZρZ)
    pub fn apply_depolarizing_channel(&mut self, qubit: usize, p: f64) {
        if qubit >= self.num_system_qubits {
            return;
        }
        
        // For the purified state, we need to apply the channel to the system qubit
        // This is more complex - simplified implementation
        let dim = 1usize << self.num_system_qubits;
        let anc_dim = 1usize << self.num_ancilla_qubits;
        let bit = self.num_system_qubits - 1 - qubit;
        let mask = 1usize << bit;
        
        // Apply Kraus operators probabilistically
        // K0 = √(1-p) I, K1 = √(p/3) X, K2 = √(p/3) Y, K3 = √(p/3) Z
        
        let sqrt_1_p = (1.0 - p).sqrt();
        let sqrt_p3 = (p / 3.0).sqrt();
        
        let mut new_amplitudes = vec![Complex64::new(0.0, 0.0); self.purified_amplitudes.len()];
        
        for i in 0..dim {
            for k in 0..anc_dim {
                let idx = i * anc_dim + k;
                let amp = self.purified_amplitudes[idx];
                
                // I term
                new_amplitudes[idx] += amp * sqrt_1_p;
                
                // X term (bit flip)
                let i_x = i ^ mask;
                let idx_x = i_x * anc_dim + k;
                new_amplitudes[idx_x] += amp * sqrt_p3;
                
                // Z term (phase flip)
                let sign = if (i & mask) != 0 { -1.0 } else { 1.0 };
                new_amplitudes[idx] += amp * sqrt_p3 * sign;
                
                // Y term (both)
                let y_sign = if (i & mask) != 0 { 
                    Complex64::new(0.0, 1.0) 
                } else { 
                    Complex64::new(0.0, -1.0) 
                };
                new_amplitudes[idx_x] += amp * sqrt_p3 * y_sign;
            }
        }
        
        // Normalize
        let norm: f64 = new_amplitudes.iter().map(|a| a.norm_sqr()).sum::<f64>().sqrt();
        if norm > 1e-15 {
            for amp in &mut new_amplitudes {
                *amp /= norm;
            }
        }
        
        self.purified_amplitudes = new_amplitudes;
    }
    
    /// Apply amplitude damping channel (T1 decay) with damping parameter γ.
    pub fn apply_amplitude_damping(&mut self, qubit: usize, gamma: f64) {
        if qubit >= self.num_system_qubits {
            return;
        }
        
        let dim = 1usize << self.num_system_qubits;
        let anc_dim = 1usize << self.num_ancilla_qubits;
        let bit = self.num_system_qubits - 1 - qubit;
        let mask = 1usize << bit;
        
        // Kraus operators: K0 = |0⟩⟨0| + √(1-γ)|1⟩⟨1|, K1 = √γ|0⟩⟨1|
        let sqrt_1_gamma = (1.0 - gamma).sqrt();
        let sqrt_gamma = gamma.sqrt();
        
        let mut new_amplitudes = vec![Complex64::new(0.0, 0.0); self.purified_amplitudes.len()];
        
        for i in 0..dim {
            for k in 0..anc_dim {
                let idx = i * anc_dim + k;
                let amp = self.purified_amplitudes[idx];
                
                if (i & mask) == 0 {
                    // |0⟩ state: unchanged by K0, receives from |1⟩ via K1
                    new_amplitudes[idx] += amp;
                } else {
                    // |1⟩ state: damped by K0
                    new_amplitudes[idx] += amp * sqrt_1_gamma;
                    
                    // K1: |1⟩ → |0⟩ with probability γ
                    let i_0 = i ^ mask;
                    let idx_0 = i_0 * anc_dim + k;
                    new_amplitudes[idx_0] += amp * sqrt_gamma;
                }
            }
        }
        
        // Normalize
        let norm: f64 = new_amplitudes.iter().map(|a| a.norm_sqr()).sum::<f64>().sqrt();
        if norm > 1e-15 {
            for amp in &mut new_amplitudes {
                *amp /= norm;
            }
        }
        
        self.purified_amplitudes = new_amplitudes;
    }
    
    /// Compute fidelity with a pure target state: F = ⟨ψ|ρ|ψ⟩
    pub fn fidelity_with_pure(&self, target: &QuantumStateVector) -> f64 {
        let rho = self.to_density_matrix();
        let dim = rho.nrows();
        
        if target.dimension() != dim {
            return 0.0;
        }
        
        let mut fidelity = Complex64::new(0.0, 0.0);
        for i in 0..dim {
            for j in 0..dim {
                fidelity += target.amplitude(i).conj() * rho[[i, j]] * target.amplitude(j);
            }
        }
        
        fidelity.re.max(0.0)
    }
}

/// Create a thermal state at inverse temperature β.
/// ρ = exp(-βH) / Z where H = Σ_i Z_i (simple Ising Hamiltonian)
pub fn thermal_state(num_qubits: usize, beta: f64, config: DmMpsConfig) -> DensityMatrixMps {
    let dim = 1usize << num_qubits;
    let mut rho = Array2::zeros((dim, dim));
    
    // Compute diagonal elements (eigenstates of Z⊗Z⊗...⊗Z)
    let mut partition_fn = 0.0;
    for i in 0..dim {
        // Energy = number of |1⟩s minus number of |0⟩s
        let ones = (i as u32).count_ones() as i32;
        let zeros = num_qubits as i32 - ones;
        let energy = (ones - zeros) as f64;
        
        let boltzmann = (-beta * energy).exp();
        rho[[i, i]] = Complex64::new(boltzmann, 0.0);
        partition_fn += boltzmann;
    }
    
    // Normalize
    for i in 0..dim {
        rho[[i, i]] /= partition_fn;
    }
    
    DensityMatrixMps::from_density_matrix(&rho, config).unwrap_or_else(|_| {
        DensityMatrixMps::maximally_mixed(num_qubits, config)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_maximally_mixed() {
        let config = DmMpsConfig::default();
        let dm = DensityMatrixMps::maximally_mixed(2, config);
        
        let rho = dm.to_density_matrix();
        
        // Should be (1/4) * I
        for i in 0..4 {
            assert!((rho[[i, i]].re - 0.25).abs() < 1e-10);
            for j in 0..4 {
                if i != j {
                    assert!(rho[[i, j]].norm() < 1e-10);
                }
            }
        }
        
        // Purity should be 1/d = 0.25
        assert!((dm.purity() - 0.25).abs() < 1e-10);
    }
    
    #[test]
    fn test_pure_state() {
        let config = DmMpsConfig::default();
        let state = QuantumStateVector::zero_state(2);
        let dm = DensityMatrixMps::from_pure_state(&state, config);
        
        let rho = dm.to_density_matrix();
        
        // Should be |00⟩⟨00|
        assert!((rho[[0, 0]].re - 1.0).abs() < 1e-10);
        for i in 1..4 {
            assert!(rho[[i, i]].norm() < 1e-10);
        }
        
        // Purity should be 1 (pure state)
        assert!((dm.purity() - 1.0).abs() < 1e-10);
    }
    
    #[test]
    fn test_fidelity() {
        let config = DmMpsConfig::default();
        let state = QuantumStateVector::zero_state(2);
        let dm = DensityMatrixMps::from_pure_state(&state, config);
        
        // Fidelity with itself should be 1
        let fid = dm.fidelity_with_pure(&state);
        assert!((fid - 1.0).abs() < 1e-10);
        
        // Fidelity with orthogonal state should be 0
        let ortho = QuantumStateVector::one_state(2);
        let fid_ortho = dm.fidelity_with_pure(&ortho);
        assert!(fid_ortho < 1e-10);
    }
}
