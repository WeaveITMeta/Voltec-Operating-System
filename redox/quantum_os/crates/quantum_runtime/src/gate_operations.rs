// =============================================================================
// V-OS - Gate Operations
// =============================================================================
// Table of Contents:
//   1. QuantumGateInterface - Core trait for all gates
//   2. Single-qubit gates (Hadamard, Pauli X/Y/Z, Rotations)
//   3. Two-qubit gates (CNOT, CPhase, SWAP)
//   4. Multi-qubit gates (Toffoli)
// =============================================================================
// Purpose: Defines quantum gate operations with the QuantumGateInterface trait.
//          Each gate implements apply methods for different state backends.
//          Uses full-word snake_case naming per glossary.
// =============================================================================

use crate::state_backend::QuantumStateVector;
use num_complex::Complex64;
use std::f64::consts::PI;

// =============================================================================
// 1. QuantumGateInterface - Core trait for all gates
// =============================================================================

pub trait QuantumGateInterface: Send + Sync + std::fmt::Debug {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector);
    fn gate_name(&self) -> &str;
    fn target_quantum_bits(&self) -> Vec<usize>;
    fn gate_matrix(&self) -> Vec<Vec<Complex64>>;
}

// =============================================================================
// 2. Single-qubit gates
// =============================================================================

#[derive(Debug, Clone)]
pub struct HadamardGate {
    target_qubit: usize,
}

impl HadamardGate {
    pub fn new(target_qubit: usize) -> Self {
        Self { target_qubit }
    }
}

impl QuantumGateInterface for HadamardGate {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector) {
        let n = state.number_of_quantum_bits();
        let target_bit = n - 1 - self.target_qubit;
        let target_mask = 1usize << target_bit;
        let inv_sqrt2 = 1.0 / std::f64::consts::SQRT_2;

        let dim = state.dimension();
        for i in 0..dim {
            if (i & target_mask) == 0 {
                let j = i | target_mask;
                let a = state.amplitude(i);
                let b = state.amplitude(j);
                state.set_amplitude(i, (a + b) * inv_sqrt2);
                state.set_amplitude(j, (a - b) * inv_sqrt2);
            }
        }
    }

    fn gate_name(&self) -> &str {
        "hadamard_gate"
    }

    fn target_quantum_bits(&self) -> Vec<usize> {
        vec![self.target_qubit]
    }

    fn gate_matrix(&self) -> Vec<Vec<Complex64>> {
        let inv_sqrt2 = Complex64::new(1.0 / std::f64::consts::SQRT_2, 0.0);
        vec![
            vec![inv_sqrt2, inv_sqrt2],
            vec![inv_sqrt2, -inv_sqrt2],
        ]
    }
}

#[derive(Debug, Clone)]
pub struct PauliXGate {
    target_qubit: usize,
}

impl PauliXGate {
    pub fn new(target_qubit: usize) -> Self {
        Self { target_qubit }
    }
}

impl QuantumGateInterface for PauliXGate {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector) {
        let n = state.number_of_quantum_bits();
        let target_bit = n - 1 - self.target_qubit;
        let target_mask = 1usize << target_bit;

        let dim = state.dimension();
        for i in 0..dim {
            if (i & target_mask) == 0 {
                let j = i | target_mask;
                state.swap_amplitudes(i, j);
            }
        }
    }

    fn gate_name(&self) -> &str {
        "pauli_x_gate"
    }

    fn target_quantum_bits(&self) -> Vec<usize> {
        vec![self.target_qubit]
    }

    fn gate_matrix(&self) -> Vec<Vec<Complex64>> {
        vec![
            vec![Complex64::new(0.0, 0.0), Complex64::new(1.0, 0.0)],
            vec![Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)],
        ]
    }
}

#[derive(Debug, Clone)]
pub struct PauliYGate {
    target_qubit: usize,
}

impl PauliYGate {
    pub fn new(target_qubit: usize) -> Self {
        Self { target_qubit }
    }
}

impl QuantumGateInterface for PauliYGate {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector) {
        let n = state.number_of_quantum_bits();
        let target_bit = n - 1 - self.target_qubit;
        let target_mask = 1usize << target_bit;

        let dim = state.dimension();
        for i in 0..dim {
            if (i & target_mask) == 0 {
                let j = i | target_mask;
                let a = state.amplitude(i);
                let b = state.amplitude(j);
                state.set_amplitude(i, Complex64::new(0.0, 1.0) * b);
                state.set_amplitude(j, Complex64::new(0.0, -1.0) * a);
            }
        }
    }

    fn gate_name(&self) -> &str {
        "pauli_y_gate"
    }

    fn target_quantum_bits(&self) -> Vec<usize> {
        vec![self.target_qubit]
    }

    fn gate_matrix(&self) -> Vec<Vec<Complex64>> {
        vec![
            vec![Complex64::new(0.0, 0.0), Complex64::new(0.0, -1.0)],
            vec![Complex64::new(0.0, 1.0), Complex64::new(0.0, 0.0)],
        ]
    }
}

#[derive(Debug, Clone)]
pub struct PauliZGate {
    target_qubit: usize,
}

impl PauliZGate {
    pub fn new(target_qubit: usize) -> Self {
        Self { target_qubit }
    }
}

impl QuantumGateInterface for PauliZGate {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector) {
        let n = state.number_of_quantum_bits();
        let target_bit = n - 1 - self.target_qubit;
        let target_mask = 1usize << target_bit;

        let dim = state.dimension();
        for i in 0..dim {
            if (i & target_mask) != 0 {
                let amp = state.amplitude(i);
                state.set_amplitude(i, -amp);
            }
        }
    }

    fn gate_name(&self) -> &str {
        "pauli_z_gate"
    }

    fn target_quantum_bits(&self) -> Vec<usize> {
        vec![self.target_qubit]
    }

    fn gate_matrix(&self) -> Vec<Vec<Complex64>> {
        vec![
            vec![Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)],
            vec![Complex64::new(0.0, 0.0), Complex64::new(-1.0, 0.0)],
        ]
    }
}

#[derive(Debug, Clone)]
pub struct RotationXGate {
    target_qubit: usize,
    theta: f64,
}

impl RotationXGate {
    pub fn new(target_qubit: usize, theta: f64) -> Self {
        Self { target_qubit, theta }
    }
}

impl QuantumGateInterface for RotationXGate {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector) {
        let n = state.number_of_quantum_bits();
        let target_bit = n - 1 - self.target_qubit;
        let target_mask = 1usize << target_bit;

        let cos_half = (self.theta / 2.0).cos();
        let sin_half = (self.theta / 2.0).sin();

        let dim = state.dimension();
        for i in 0..dim {
            if (i & target_mask) == 0 {
                let j = i | target_mask;
                let a = state.amplitude(i);
                let b = state.amplitude(j);
                state.set_amplitude(
                    i,
                    Complex64::new(cos_half, 0.0) * a + Complex64::new(0.0, -sin_half) * b,
                );
                state.set_amplitude(
                    j,
                    Complex64::new(0.0, -sin_half) * a + Complex64::new(cos_half, 0.0) * b,
                );
            }
        }
    }

    fn gate_name(&self) -> &str {
        "rotation_x_gate"
    }

    fn target_quantum_bits(&self) -> Vec<usize> {
        vec![self.target_qubit]
    }

    fn gate_matrix(&self) -> Vec<Vec<Complex64>> {
        let cos_half = (self.theta / 2.0).cos();
        let sin_half = (self.theta / 2.0).sin();
        vec![
            vec![
                Complex64::new(cos_half, 0.0),
                Complex64::new(0.0, -sin_half),
            ],
            vec![
                Complex64::new(0.0, -sin_half),
                Complex64::new(cos_half, 0.0),
            ],
        ]
    }
}

#[derive(Debug, Clone)]
pub struct RotationYGate {
    target_qubit: usize,
    theta: f64,
}

impl RotationYGate {
    pub fn new(target_qubit: usize, theta: f64) -> Self {
        Self { target_qubit, theta }
    }
}

impl QuantumGateInterface for RotationYGate {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector) {
        let n = state.number_of_quantum_bits();
        let target_bit = n - 1 - self.target_qubit;
        let target_mask = 1usize << target_bit;

        let cos_half = (self.theta / 2.0).cos();
        let sin_half = (self.theta / 2.0).sin();

        let dim = state.dimension();
        for i in 0..dim {
            if (i & target_mask) == 0 {
                let j = i | target_mask;
                let a = state.amplitude(i);
                let b = state.amplitude(j);
                state.set_amplitude(i, cos_half * a - sin_half * b);
                state.set_amplitude(j, sin_half * a + cos_half * b);
            }
        }
    }

    fn gate_name(&self) -> &str {
        "rotation_y_gate"
    }

    fn target_quantum_bits(&self) -> Vec<usize> {
        vec![self.target_qubit]
    }

    fn gate_matrix(&self) -> Vec<Vec<Complex64>> {
        let cos_half = (self.theta / 2.0).cos();
        let sin_half = (self.theta / 2.0).sin();
        vec![
            vec![Complex64::new(cos_half, 0.0), Complex64::new(-sin_half, 0.0)],
            vec![Complex64::new(sin_half, 0.0), Complex64::new(cos_half, 0.0)],
        ]
    }
}

#[derive(Debug, Clone)]
pub struct RotationZGate {
    target_qubit: usize,
    theta: f64,
}

impl RotationZGate {
    pub fn new(target_qubit: usize, theta: f64) -> Self {
        Self { target_qubit, theta }
    }
}

impl QuantumGateInterface for RotationZGate {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector) {
        let n = state.number_of_quantum_bits();
        let target_bit = n - 1 - self.target_qubit;
        let target_mask = 1usize << target_bit;

        let phase_0 = Complex64::from_polar(1.0, -self.theta / 2.0);
        let phase_1 = Complex64::from_polar(1.0, self.theta / 2.0);

        let dim = state.dimension();
        for i in 0..dim {
            if (i & target_mask) == 0 {
                let amp = state.amplitude(i);
                state.set_amplitude(i, phase_0 * amp);
            } else {
                let amp = state.amplitude(i);
                state.set_amplitude(i, phase_1 * amp);
            }
        }
    }

    fn gate_name(&self) -> &str {
        "rotation_z_gate"
    }

    fn target_quantum_bits(&self) -> Vec<usize> {
        vec![self.target_qubit]
    }

    fn gate_matrix(&self) -> Vec<Vec<Complex64>> {
        vec![
            vec![
                Complex64::from_polar(1.0, -self.theta / 2.0),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::from_polar(1.0, self.theta / 2.0),
            ],
        ]
    }
}

// =============================================================================
// 3. Two-qubit gates
// =============================================================================

#[derive(Debug, Clone)]
pub struct ControlledNotGate {
    control_qubit: usize,
    target_qubit: usize,
    number_of_quantum_bits: usize,
}

impl ControlledNotGate {
    pub fn new(control_qubit: usize, target_qubit: usize, number_of_quantum_bits: usize) -> Self {
        Self {
            control_qubit,
            target_qubit,
            number_of_quantum_bits,
        }
    }
}

impl QuantumGateInterface for ControlledNotGate {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector) {
        let n = state.number_of_quantum_bits();
        let control_bit = n - 1 - self.control_qubit;
        let target_bit = n - 1 - self.target_qubit;
        let control_mask = 1usize << control_bit;
        let target_mask = 1usize << target_bit;

        let dim = state.dimension();
        for i in 0..dim {
            if (i & control_mask) != 0 && (i & target_mask) == 0 {
                let j = i | target_mask;
                state.swap_amplitudes(i, j);
            }
        }
    }

    fn gate_name(&self) -> &str {
        "controlled_not_gate"
    }

    fn target_quantum_bits(&self) -> Vec<usize> {
        vec![self.control_qubit, self.target_qubit]
    }

    fn gate_matrix(&self) -> Vec<Vec<Complex64>> {
        vec![
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
        ]
    }
}

#[derive(Debug, Clone)]
pub struct ControlledPhaseGate {
    control_qubit: usize,
    target_qubit: usize,
    theta: f64,
    number_of_quantum_bits: usize,
}

impl ControlledPhaseGate {
    pub fn new(
        control_qubit: usize,
        target_qubit: usize,
        theta: f64,
        number_of_quantum_bits: usize,
    ) -> Self {
        Self {
            control_qubit,
            target_qubit,
            theta,
            number_of_quantum_bits,
        }
    }
}

impl QuantumGateInterface for ControlledPhaseGate {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector) {
        let n = state.number_of_quantum_bits();
        let control_bit = n - 1 - self.control_qubit;
        let target_bit = n - 1 - self.target_qubit;
        let control_mask = 1usize << control_bit;
        let target_mask = 1usize << target_bit;
        let phase = Complex64::from_polar(1.0, self.theta);

        let dim = state.dimension();
        for i in 0..dim {
            if (i & control_mask) != 0 && (i & target_mask) != 0 {
                let amp = state.amplitude(i);
                state.set_amplitude(i, phase * amp);
            }
        }
    }

    fn gate_name(&self) -> &str {
        "controlled_phase_gate"
    }

    fn target_quantum_bits(&self) -> Vec<usize> {
        vec![self.control_qubit, self.target_qubit]
    }

    fn gate_matrix(&self) -> Vec<Vec<Complex64>> {
        let phase = Complex64::from_polar(1.0, self.theta);
        vec![
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                phase,
            ],
        ]
    }
}

#[derive(Debug, Clone)]
pub struct SwapGate {
    qubit_a: usize,
    qubit_b: usize,
    number_of_quantum_bits: usize,
}

impl SwapGate {
    pub fn new(qubit_a: usize, qubit_b: usize, number_of_quantum_bits: usize) -> Self {
        Self {
            qubit_a,
            qubit_b,
            number_of_quantum_bits,
        }
    }
}

impl QuantumGateInterface for SwapGate {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector) {
        let n = state.number_of_quantum_bits();
        let bit_a = n - 1 - self.qubit_a;
        let bit_b = n - 1 - self.qubit_b;
        let mask_a = 1usize << bit_a;
        let mask_b = 1usize << bit_b;

        let dim = state.dimension();
        for i in 0..dim {
            let bit_val_a = (i & mask_a) != 0;
            let bit_val_b = (i & mask_b) != 0;
            if bit_val_a && !bit_val_b {
                let j = (i & !mask_a) | mask_b;
                state.swap_amplitudes(i, j);
            }
        }
    }

    fn gate_name(&self) -> &str {
        "swap_gate"
    }

    fn target_quantum_bits(&self) -> Vec<usize> {
        vec![self.qubit_a, self.qubit_b]
    }

    fn gate_matrix(&self) -> Vec<Vec<Complex64>> {
        vec![
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
            ],
        ]
    }
}

// =============================================================================
// 4. Multi-qubit gates
// =============================================================================

#[derive(Debug, Clone)]
pub struct ToffoliGate {
    control_qubit_1: usize,
    control_qubit_2: usize,
    target_qubit: usize,
    number_of_quantum_bits: usize,
}

impl ToffoliGate {
    pub fn new(
        control_qubit_1: usize,
        control_qubit_2: usize,
        target_qubit: usize,
        number_of_quantum_bits: usize,
    ) -> Self {
        Self {
            control_qubit_1,
            control_qubit_2,
            target_qubit,
            number_of_quantum_bits,
        }
    }
}

impl QuantumGateInterface for ToffoliGate {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector) {
        let n = state.number_of_quantum_bits();
        let c1_bit = n - 1 - self.control_qubit_1;
        let c2_bit = n - 1 - self.control_qubit_2;
        let t_bit = n - 1 - self.target_qubit;
        let c1_mask = 1usize << c1_bit;
        let c2_mask = 1usize << c2_bit;
        let t_mask = 1usize << t_bit;
        let both_controls = c1_mask | c2_mask;

        let dim = state.dimension();
        for i in 0..dim {
            if (i & both_controls) == both_controls && (i & t_mask) == 0 {
                let j = i | t_mask;
                state.swap_amplitudes(i, j);
            }
        }
    }

    fn gate_name(&self) -> &str {
        "toffoli_gate"
    }

    fn target_quantum_bits(&self) -> Vec<usize> {
        vec![self.control_qubit_1, self.control_qubit_2, self.target_qubit]
    }

    fn gate_matrix(&self) -> Vec<Vec<Complex64>> {
        let mut matrix = vec![vec![Complex64::new(0.0, 0.0); 8]; 8];
        for i in 0..6 {
            matrix[i][i] = Complex64::new(1.0, 0.0);
        }
        matrix[6][7] = Complex64::new(1.0, 0.0);
        matrix[7][6] = Complex64::new(1.0, 0.0);
        matrix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hadamard_creates_superposition() {
        let mut state = QuantumStateVector::zero_state(1);
        let h = HadamardGate::new(0);
        h.apply_to_full_state_vector(&mut state);

        let prob_0 = state.amplitude(0).norm_sqr();
        let prob_1 = state.amplitude(1).norm_sqr();
        assert!((prob_0 - 0.5).abs() < 1e-10);
        assert!((prob_1 - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_cnot_entanglement() {
        let mut state = QuantumStateVector::zero_state(2);
        let h = HadamardGate::new(0);
        let cnot = ControlledNotGate::new(0, 1, 2);

        h.apply_to_full_state_vector(&mut state);
        cnot.apply_to_full_state_vector(&mut state);

        let prob_00 = state.amplitude(0).norm_sqr();
        let prob_11 = state.amplitude(3).norm_sqr();
        assert!((prob_00 - 0.5).abs() < 1e-10);
        assert!((prob_11 - 0.5).abs() < 1e-10);
    }
}
