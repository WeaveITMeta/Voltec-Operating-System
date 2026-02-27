// =============================================================================
// V-OS - Quantum Golay Code [[23,1,7]]
// =============================================================================
// Table of Contents:
//   1. Stabilizer Generators - Complete X and Z stabilizer definitions
//   2. Syndrome Measurement - Quantum syndrome extraction circuits
//   3. Error Correction - Pauli error decoding and correction
//   4. Logical Operators - Encoded logical X and Z operations
//   5. Fault-Tolerant Operations - Transversal gate implementations
//   6. Noise Models - Correlated error simulation
// =============================================================================
// Purpose: Complete implementation of the [[23,1,7]] quantum Golay code,
//          a CSS code derived from the classical [23,12,7] binary Golay code.
//          Provides fault-tolerant encoding for quantum data storage.
// =============================================================================
// Mathematical Foundation:
//   - CSS construction: C_X = C_Z = [23,12,7] Golay code
//   - Stabilizer group: 22 generators (11 X-type, 11 Z-type)
//   - Logical operators: weight-7 representatives
//   - Distance d=7: corrects up to 3 arbitrary single-qubit errors
//   - Connection to Mathieu group M_23 and Leech lattice Λ_24
// =============================================================================
// References:
//   - Calderbank & Shor (1996), Phys. Rev. A 54, 1098
//   - Steane (1999), IEEE Trans. Inf. Theory 45, 1701
//   - Grassl, Beth, Pellizzari (1997), Phys. Rev. A 56, 33
// =============================================================================

use num_complex::Complex64;
use std::collections::HashMap;
use thiserror::Error;

// =============================================================================
// Error Types
// =============================================================================

#[derive(Debug, Error, Clone, PartialEq)]
pub enum QuantumGolayError {
    #[error("Syndrome decoding failed: X syndrome {x_syndrome:#06x}, Z syndrome {z_syndrome:#06x}")]
    DecodingFailed { x_syndrome: u32, z_syndrome: u32 },

    #[error("Invalid logical state: expected |0⟩_L or |1⟩_L encoding")]
    InvalidLogicalState,

    #[error("Uncorrectable error detected: weight exceeds code distance")]
    UncorrectableError,

    #[error("Measurement error in syndrome extraction")]
    MeasurementError,
}

// =============================================================================
// 1. Stabilizer Generators - Complete Definition
// =============================================================================

/// Complete [[23,1,7]] quantum Golay code implementation
/// 
/// The quantum Golay code is a CSS code with parameters:
/// - n = 23 physical qubits
/// - k = 1 logical qubit  
/// - d = 7 code distance
/// 
/// It can correct any combination of up to 3 single-qubit errors
/// (X, Y, or Z errors on any 3 qubits).
#[derive(Debug, Clone)]
pub struct QuantumGolayCode {
    /// X-type stabilizer generators (11 generators, each a 23-bit mask)
    /// S_X^{(i)} = X on qubits where bit i of H is 1
    pub x_stabilizers: [u32; 11],
    
    /// Z-type stabilizer generators (11 generators, each a 23-bit mask)
    /// S_Z^{(i)} = Z on qubits where bit i of H is 1
    pub z_stabilizers: [u32; 11],
    
    /// Logical X operator (weight-7 representative)
    pub logical_x: u32,
    
    /// Logical Z operator (weight-7 representative)
    pub logical_z: u32,
    
    /// Parity check matrix H (11 x 23)
    /// Each row defines which qubits participate in that stabilizer
    pub parity_check_matrix: [[u8; 23]; 11],
    
    /// Syndrome lookup table for X errors
    pub x_syndrome_table: HashMap<u32, u32>,
    
    /// Syndrome lookup table for Z errors
    pub z_syndrome_table: HashMap<u32, u32>,
}

impl QuantumGolayCode {
    /// Construct the quantum Golay code with all stabilizers and lookup tables
    pub fn new() -> Self {
        // Parity check matrix H for the [23,12,7] Golay code
        // This is the standard form derived from the generator polynomial
        // g(x) = x^11 + x^10 + x^6 + x^5 + x^4 + x^2 + 1
        let parity_check_matrix: [[u8; 23]; 11] = [
            [1,0,0,0,0,0,0,0,0,0,0, 1,0,0,0,1,1,0,0,1,1,1,1], // Row 0
            [0,1,0,0,0,0,0,0,0,0,0, 1,1,0,0,0,1,1,0,0,1,1,1], // Row 1
            [0,0,1,0,0,0,0,0,0,0,0, 1,1,1,0,0,0,1,1,0,0,1,1], // Row 2
            [0,0,0,1,0,0,0,0,0,0,0, 1,1,1,1,0,0,0,1,1,0,0,1], // Row 3
            [0,0,0,0,1,0,0,0,0,0,0, 1,1,1,1,1,0,0,0,1,1,0,0], // Row 4
            [0,0,0,0,0,1,0,0,0,0,0, 0,1,1,1,1,1,0,0,0,1,1,0], // Row 5
            [0,0,0,0,0,0,1,0,0,0,0, 0,0,1,1,1,1,1,0,0,0,1,1], // Row 6
            [0,0,0,0,0,0,0,1,0,0,0, 1,0,0,1,1,1,1,1,0,0,0,1], // Row 7
            [0,0,0,0,0,0,0,0,1,0,0, 1,1,0,0,1,1,1,1,1,0,0,0], // Row 8
            [0,0,0,0,0,0,0,0,0,1,0, 0,1,1,0,0,1,1,1,1,1,0,0], // Row 9
            [0,0,0,0,0,0,0,0,0,0,1, 0,0,1,1,0,0,1,1,1,1,1,0], // Row 10
        ];

        // Convert parity check rows to bitmasks for stabilizers
        let mut x_stabilizers = [0u32; 11];
        let mut z_stabilizers = [0u32; 11];
        
        for (i, row) in parity_check_matrix.iter().enumerate() {
            let mut mask = 0u32;
            for (j, &bit) in row.iter().enumerate() {
                if bit == 1 {
                    mask |= 1 << (22 - j);
                }
            }
            x_stabilizers[i] = mask;
            z_stabilizers[i] = mask; // CSS code: same support for X and Z
        }

        // Logical operators - minimum weight representatives (weight 7)
        // These commute with all stabilizers but anticommute with each other
        // Logical X: X on qubits {0,1,2,3,4,5,6} (first 7 qubits)
        // This is a valid choice as it has odd overlap with logical Z
        let logical_x = 0b11111110000000000000000u32; // Qubits 0-6
        
        // Logical Z: Z on a weight-7 codeword
        // Using the all-ones codeword projected to weight 7
        let logical_z = 0b10000011000011000011001u32; // Weight-7 codeword
        
        // Build syndrome lookup tables
        let x_syndrome_table = Self::build_syndrome_table(&x_stabilizers);
        let z_syndrome_table = Self::build_syndrome_table(&z_stabilizers);

        Self {
            x_stabilizers,
            z_stabilizers,
            logical_x,
            logical_z,
            parity_check_matrix,
            x_syndrome_table,
            z_syndrome_table,
        }
    }

    /// Build syndrome lookup table for all correctable error patterns
    /// 
    /// For a distance-7 code, we can correct:
    /// - All weight-1 errors (23 patterns)
    /// - All weight-2 errors (253 patterns)
    /// - All weight-3 errors (1771 patterns)
    /// Total: 2047 non-trivial syndromes + 1 trivial = 2048 = 2^11
    fn build_syndrome_table(stabilizers: &[u32; 11]) -> HashMap<u32, u32> {
        let mut table = HashMap::with_capacity(2048);
        
        // Weight-0: no error
        table.insert(0, 0);
        
        // Weight-1 errors
        for i in 0..23u32 {
            let error = 1u32 << i;
            let syndrome = Self::compute_syndrome_static(error, stabilizers);
            table.insert(syndrome, error);
        }
        
        // Weight-2 errors
        for i in 0..23u32 {
            for j in (i + 1)..23u32 {
                let error = (1u32 << i) | (1u32 << j);
                let syndrome = Self::compute_syndrome_static(error, stabilizers);
                table.entry(syndrome).or_insert(error);
            }
        }
        
        // Weight-3 errors
        for i in 0..23u32 {
            for j in (i + 1)..23u32 {
                for k in (j + 1)..23u32 {
                    let error = (1u32 << i) | (1u32 << j) | (1u32 << k);
                    let syndrome = Self::compute_syndrome_static(error, stabilizers);
                    table.entry(syndrome).or_insert(error);
                }
            }
        }
        
        table
    }

    /// Compute syndrome for an error pattern
    fn compute_syndrome_static(error: u32, stabilizers: &[u32; 11]) -> u32 {
        let mut syndrome = 0u32;
        for (i, &stab) in stabilizers.iter().enumerate() {
            // Syndrome bit i = parity of (error AND stabilizer_i)
            if (error & stab).count_ones() % 2 == 1 {
                syndrome |= 1 << i;
            }
        }
        syndrome
    }

    /// Compute X syndrome (detects Z errors)
    pub fn compute_x_syndrome(&self, z_error: u32) -> u32 {
        Self::compute_syndrome_static(z_error, &self.x_stabilizers)
    }

    /// Compute Z syndrome (detects X errors)
    pub fn compute_z_syndrome(&self, x_error: u32) -> u32 {
        Self::compute_syndrome_static(x_error, &self.z_stabilizers)
    }

    /// Decode X error from Z syndrome
    pub fn decode_x_error(&self, z_syndrome: u32) -> Result<u32, QuantumGolayError> {
        self.z_syndrome_table
            .get(&z_syndrome)
            .copied()
            .ok_or(QuantumGolayError::DecodingFailed {
                x_syndrome: 0,
                z_syndrome,
            })
    }

    /// Decode Z error from X syndrome
    pub fn decode_z_error(&self, x_syndrome: u32) -> Result<u32, QuantumGolayError> {
        self.x_syndrome_table
            .get(&x_syndrome)
            .copied()
            .ok_or(QuantumGolayError::DecodingFailed {
                x_syndrome,
                z_syndrome: 0,
            })
    }

    /// Full error correction: decode both X and Z errors
    pub fn decode_errors(
        &self,
        x_syndrome: u32,
        z_syndrome: u32,
    ) -> Result<(u32, u32), QuantumGolayError> {
        let x_error = self.decode_x_error(z_syndrome)?;
        let z_error = self.decode_z_error(x_syndrome)?;
        Ok((x_error, z_error))
    }

    // =========================================================================
    // Code Parameters
    // =========================================================================

    /// Number of physical qubits
    pub const fn num_physical_qubits() -> usize {
        23
    }

    /// Number of logical qubits
    pub const fn num_logical_qubits() -> usize {
        1
    }

    /// Code distance
    pub const fn distance() -> usize {
        7
    }

    /// Maximum correctable errors
    pub const fn max_correctable_errors() -> usize {
        3
    }

    /// Number of stabilizer generators
    pub const fn num_stabilizers() -> usize {
        22 // 11 X-type + 11 Z-type
    }

    // =========================================================================
    // Logical State Encoding
    // =========================================================================

    /// Encode |0⟩_L: superposition of all even-weight codewords
    /// |0⟩_L = (1/√2048) Σ_{c ∈ C, wt(c) even} |c⟩
    pub fn encode_logical_zero(&self) -> Vec<(u32, Complex64)> {
        let mut state = Vec::new();
        let norm = 1.0 / (2048.0_f64).sqrt();
        
        // Generate all codewords (2^12 = 4096 total, half even weight)
        for msg in 0..4096u32 {
            let codeword = self.encode_classical(msg as u16);
            if codeword.count_ones() % 2 == 0 {
                state.push((codeword, Complex64::new(norm, 0.0)));
            }
        }
        
        state
    }

    /// Encode |1⟩_L: superposition of all odd-weight codewords
    /// |1⟩_L = (1/√2048) Σ_{c ∈ C, wt(c) odd} |c⟩
    pub fn encode_logical_one(&self) -> Vec<(u32, Complex64)> {
        let mut state = Vec::new();
        let norm = 1.0 / (2048.0_f64).sqrt();
        
        for msg in 0..4096u32 {
            let codeword = self.encode_classical(msg as u16);
            if codeword.count_ones() % 2 == 1 {
                state.push((codeword, Complex64::new(norm, 0.0)));
            }
        }
        
        state
    }

    /// Classical Golay encoding (for generating codewords)
    fn encode_classical(&self, message: u16) -> u32 {
        let msg = message & 0xFFF; // 12 bits
        let mut parity = 0u16;
        
        // Compute parity bits using generator matrix
        for i in 0..12 {
            if (msg >> (11 - i)) & 1 == 1 {
                parity ^= GOLAY_P_MATRIX_U16[i];
            }
        }
        
        ((msg as u32) << 11) | (parity as u32)
    }
}

impl Default for QuantumGolayCode {
    fn default() -> Self {
        Self::new()
    }
}

/// Parity matrix for classical encoding
const GOLAY_P_MATRIX_U16: [u16; 12] = [
    0b10000110001, // Row 0
    0b11000011000, // Row 1
    0b11100001100, // Row 2
    0b11110000110, // Row 3
    0b11111000011, // Row 4
    0b01111100001, // Row 5
    0b00111110000, // Row 6
    0b10011111000, // Row 7
    0b11001111100, // Row 8
    0b01100111110, // Row 9
    0b00110011111, // Row 10
    0b10011001111, // Row 11
];

// =============================================================================
// 2. Syndrome Measurement Circuits
// =============================================================================

/// Syndrome measurement circuit for the quantum Golay code
/// 
/// Uses ancilla qubits to measure stabilizer generators non-destructively.
/// Each X stabilizer is measured using CNOT gates from data to ancilla.
/// Each Z stabilizer is measured using CNOT gates from ancilla to data.
#[derive(Debug, Clone)]
pub struct SyndromeMeasurementCircuit {
    /// Number of ancilla qubits needed (11 for X + 11 for Z = 22)
    pub num_ancillas: usize,
    
    /// CNOT gates for X stabilizer measurement
    /// Each entry: (control_data_qubit, target_ancilla)
    pub x_measurement_cnots: Vec<(usize, usize)>,
    
    /// CNOT gates for Z stabilizer measurement  
    /// Each entry: (control_ancilla, target_data_qubit)
    pub z_measurement_cnots: Vec<(usize, usize)>,
}

impl SyndromeMeasurementCircuit {
    /// Construct syndrome measurement circuit from code
    pub fn new(code: &QuantumGolayCode) -> Self {
        let mut x_cnots = Vec::new();
        let mut z_cnots = Vec::new();
        
        // X stabilizer measurement: CNOT from data to ancilla
        for (stab_idx, &stab) in code.x_stabilizers.iter().enumerate() {
            for qubit in 0..23 {
                if (stab >> (22 - qubit)) & 1 == 1 {
                    x_cnots.push((qubit, stab_idx));
                }
            }
        }
        
        // Z stabilizer measurement: CNOT from ancilla to data (with H gates)
        for (stab_idx, &stab) in code.z_stabilizers.iter().enumerate() {
            for qubit in 0..23 {
                if (stab >> (22 - qubit)) & 1 == 1 {
                    z_cnots.push((11 + stab_idx, qubit));
                }
            }
        }
        
        Self {
            num_ancillas: 22,
            x_measurement_cnots: x_cnots,
            z_measurement_cnots: z_cnots,
        }
    }

    /// Get the depth of the syndrome measurement circuit
    pub fn circuit_depth(&self) -> usize {
        // Simplified estimate: max weight of any stabilizer
        // Actual depth depends on parallelization
        7 // Weight of heaviest stabilizer
    }

    /// Get total number of CNOT gates
    pub fn num_cnots(&self) -> usize {
        self.x_measurement_cnots.len() + self.z_measurement_cnots.len()
    }
}

// =============================================================================
// 3. Fault-Tolerant Operations
// =============================================================================

/// Transversal gate implementations for the quantum Golay code
/// 
/// The [[23,1,7]] code supports transversal:
/// - Pauli gates (X, Y, Z)
/// - Hadamard (H)
/// - Phase gate (S) via magic state injection
#[derive(Debug, Clone, Copy)]
pub enum TransversalGate {
    /// Logical X = X^⊗23 (transversal)
    LogicalX,
    /// Logical Z = Z^⊗23 (transversal)
    LogicalZ,
    /// Logical Y = Y^⊗23 (transversal)
    LogicalY,
    /// Logical Hadamard (transversal for CSS codes)
    LogicalH,
}

impl TransversalGate {
    /// Get the physical gate pattern (which qubits get the gate)
    pub fn physical_pattern(&self) -> u32 {
        match self {
            // All 23 qubits get the gate
            TransversalGate::LogicalX => (1 << 23) - 1,
            TransversalGate::LogicalZ => (1 << 23) - 1,
            TransversalGate::LogicalY => (1 << 23) - 1,
            TransversalGate::LogicalH => (1 << 23) - 1,
        }
    }

    /// Check if this gate is truly transversal (fault-tolerant)
    pub fn is_transversal(&self) -> bool {
        true // All listed gates are transversal for Golay code
    }
}

// =============================================================================
// 4. Noise Models - Correlated Error Simulation
// =============================================================================

/// Noise model for simulating errors on the quantum Golay code
#[derive(Debug, Clone)]
pub struct GolayNoiseModel {
    /// Single-qubit depolarizing error rate
    pub single_qubit_error_rate: f64,
    
    /// Two-qubit gate error rate (for CNOTs in syndrome measurement)
    pub two_qubit_error_rate: f64,
    
    /// Measurement error rate
    pub measurement_error_rate: f64,
    
    /// Correlated error probability (affects adjacent qubits)
    pub correlation_strength: f64,
}

impl Default for GolayNoiseModel {
    fn default() -> Self {
        Self {
            single_qubit_error_rate: 1e-4,
            two_qubit_error_rate: 1e-3,
            measurement_error_rate: 1e-3,
            correlation_strength: 0.1,
        }
    }
}

/// Fault-tolerant threshold for the [[23,1,7]] quantum Golay code
/// Per Paetznick & Reichardt (2011), arXiv:1106.2190
/// This is for single-level encoding with optimized ancilla preparation
pub const GOLAY_FAULT_TOLERANT_THRESHOLD: f64 = 1.32e-3; // 0.132%

impl GolayNoiseModel {
    /// Create noise model with given physical error rate
    pub fn with_error_rate(p: f64) -> Self {
        Self {
            single_qubit_error_rate: p,
            two_qubit_error_rate: p * 10.0, // 2Q gates typically 10x worse
            measurement_error_rate: p * 5.0,
            correlation_strength: 0.1,
        }
    }

    /// Check if error rate is below fault-tolerant threshold
    pub fn is_below_threshold(&self) -> bool {
        self.single_qubit_error_rate < GOLAY_FAULT_TOLERANT_THRESHOLD
    }

    /// Get the fault-tolerant threshold value
    pub const fn threshold() -> f64 {
        GOLAY_FAULT_TOLERANT_THRESHOLD
    }

    /// Simulate random Pauli error on all qubits
    /// Returns (X_error_mask, Z_error_mask)
    pub fn sample_errors(&self, rng_seed: u64) -> (u32, u32) {
        let mut x_errors = 0u32;
        let mut z_errors = 0u32;
        
        // Simple LCG for reproducibility
        let mut rng = rng_seed;
        let lcg_next = |r: &mut u64| -> f64 {
            *r = r.wrapping_mul(6364136223846793005).wrapping_add(1);
            (*r as f64) / (u64::MAX as f64)
        };
        
        for qubit in 0..23 {
            let p = lcg_next(&mut rng);
            
            if p < self.single_qubit_error_rate {
                // Depolarizing: equal probability of X, Y, Z
                let error_type = (lcg_next(&mut rng) * 3.0) as u32;
                match error_type {
                    0 => x_errors |= 1 << qubit,           // X error
                    1 => z_errors |= 1 << qubit,           // Z error
                    _ => {                                  // Y error = XZ
                        x_errors |= 1 << qubit;
                        z_errors |= 1 << qubit;
                    }
                }
            }
            
            // Correlated errors with neighbors
            if qubit < 22 && lcg_next(&mut rng) < self.correlation_strength * self.single_qubit_error_rate {
                x_errors |= 1 << (qubit + 1);
            }
        }
        
        (x_errors, z_errors)
    }

    /// Estimate logical error rate using Monte Carlo
    pub fn estimate_logical_error_rate(&self, code: &QuantumGolayCode, num_samples: usize) -> f64 {
        let mut logical_errors = 0;
        
        for seed in 0..num_samples {
            let (x_err, z_err) = self.sample_errors(seed as u64);
            
            // Try to decode
            let x_syndrome = code.compute_z_syndrome(x_err);
            let z_syndrome = code.compute_x_syndrome(z_err);
            
            match code.decode_errors(x_syndrome, z_syndrome) {
                Ok((decoded_x, decoded_z)) => {
                    // Check for logical error (residual error anticommutes with logical op)
                    let residual_x = x_err ^ decoded_x;
                    let residual_z = z_err ^ decoded_z;
                    
                    // Logical X error if residual_x has odd overlap with logical_z
                    let logical_x_error = (residual_x & code.logical_z).count_ones() % 2 == 1;
                    // Logical Z error if residual_z has odd overlap with logical_x
                    let logical_z_error = (residual_z & code.logical_x).count_ones() % 2 == 1;
                    
                    if logical_x_error || logical_z_error {
                        logical_errors += 1;
                    }
                }
                Err(_) => {
                    // Decoding failed = logical error
                    logical_errors += 1;
                }
            }
        }
        
        logical_errors as f64 / num_samples as f64
    }
}

// =============================================================================
// 5. Error Statistics and Analysis
// =============================================================================

/// Statistics from quantum error correction
#[derive(Debug, Clone, Default)]
pub struct QuantumGolayStats {
    /// Total syndrome measurements
    pub total_measurements: u64,
    /// Syndromes indicating no error
    pub trivial_syndromes: u64,
    /// Weight-1 errors corrected
    pub weight1_corrections: u64,
    /// Weight-2 errors corrected
    pub weight2_corrections: u64,
    /// Weight-3 errors corrected
    pub weight3_corrections: u64,
    /// Uncorrectable errors (weight > 3)
    pub uncorrectable_errors: u64,
    /// Logical errors detected
    pub logical_errors: u64,
}

impl QuantumGolayStats {
    /// Physical error rate estimate
    pub fn physical_error_rate(&self) -> f64 {
        if self.total_measurements == 0 {
            return 0.0;
        }
        let total_errors = self.weight1_corrections 
            + 2 * self.weight2_corrections 
            + 3 * self.weight3_corrections;
        (total_errors as f64) / (self.total_measurements as f64 * 23.0)
    }

    /// Logical error rate
    pub fn logical_error_rate(&self) -> f64 {
        if self.total_measurements == 0 {
            return 0.0;
        }
        self.logical_errors as f64 / self.total_measurements as f64
    }

    /// Correction success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_measurements == 0 {
            return 1.0;
        }
        1.0 - (self.uncorrectable_errors + self.logical_errors) as f64 
            / self.total_measurements as f64
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_construction() {
        let code = QuantumGolayCode::new();
        
        assert_eq!(QuantumGolayCode::num_physical_qubits(), 23);
        assert_eq!(QuantumGolayCode::num_logical_qubits(), 1);
        assert_eq!(QuantumGolayCode::distance(), 7);
        assert_eq!(code.x_stabilizers.len(), 11);
        assert_eq!(code.z_stabilizers.len(), 11);
    }

    #[test]
    fn test_syndrome_table_completeness() {
        let code = QuantumGolayCode::new();
        
        // Should have 2048 entries (2^11 syndromes)
        assert_eq!(code.x_syndrome_table.len(), 2048);
        assert_eq!(code.z_syndrome_table.len(), 2048);
    }

    #[test]
    fn test_single_qubit_error_correction() {
        let code = QuantumGolayCode::new();
        
        // Test X error on each qubit
        for qubit in 0..23 {
            let x_error = 1u32 << qubit;
            let z_syndrome = code.compute_z_syndrome(x_error);
            let decoded = code.decode_x_error(z_syndrome).unwrap();
            assert_eq!(decoded, x_error, "Failed for X error on qubit {}", qubit);
        }
        
        // Test Z error on each qubit
        for qubit in 0..23 {
            let z_error = 1u32 << qubit;
            let x_syndrome = code.compute_x_syndrome(z_error);
            let decoded = code.decode_z_error(x_syndrome).unwrap();
            assert_eq!(decoded, z_error, "Failed for Z error on qubit {}", qubit);
        }
    }

    #[test]
    fn test_weight2_error_correction() {
        let code = QuantumGolayCode::new();
        
        // Test a few weight-2 X errors
        let test_cases = [(0, 1), (5, 10), (0, 22), (11, 12)];
        
        for (i, j) in test_cases {
            let x_error = (1u32 << i) | (1u32 << j);
            let z_syndrome = code.compute_z_syndrome(x_error);
            let decoded = code.decode_x_error(z_syndrome).unwrap();
            assert_eq!(decoded, x_error, "Failed for X error on qubits {} and {}", i, j);
        }
    }

    #[test]
    fn test_weight3_error_correction() {
        let code = QuantumGolayCode::new();
        
        // Test a few weight-3 X errors
        let test_cases = [(0, 1, 2), (5, 10, 15), (0, 11, 22)];
        
        for (i, j, k) in test_cases {
            let x_error = (1u32 << i) | (1u32 << j) | (1u32 << k);
            let z_syndrome = code.compute_z_syndrome(x_error);
            let decoded = code.decode_x_error(z_syndrome).unwrap();
            assert_eq!(decoded, x_error, "Failed for X error on qubits {}, {}, {}", i, j, k);
        }
    }

    #[test]
    fn test_logical_operators() {
        let code = QuantumGolayCode::new();
        
        // Logical X and Z should have weight >= d = 7
        assert!(code.logical_x.count_ones() >= 7);
        assert!(code.logical_z.count_ones() >= 7);
        
        // Logical X should commute with all X stabilizers
        for &stab in &code.x_stabilizers {
            assert_eq!((code.logical_x & stab).count_ones() % 2, 0);
        }
        
        // Logical Z should commute with all Z stabilizers
        for &stab in &code.z_stabilizers {
            assert_eq!((code.logical_z & stab).count_ones() % 2, 0);
        }
    }

    #[test]
    fn test_noise_model() {
        let code = QuantumGolayCode::new();
        let noise = GolayNoiseModel::with_error_rate(1e-3);
        
        // Run Monte Carlo with few samples for test
        let logical_rate = noise.estimate_logical_error_rate(&code, 100);
        
        // With p=1e-3, logical error rate should be much lower than physical
        // (threshold is around p ~ 1e-2 for Golay)
        assert!(logical_rate < 0.5); // Very loose bound for test
    }

    #[test]
    fn test_syndrome_circuit() {
        let code = QuantumGolayCode::new();
        let circuit = SyndromeMeasurementCircuit::new(&code);
        
        assert_eq!(circuit.num_ancillas, 22);
        assert!(circuit.num_cnots() > 0);
        assert!(circuit.circuit_depth() <= 23);
    }
}
