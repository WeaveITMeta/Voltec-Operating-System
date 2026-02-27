// =============================================================================
// V-OS - LogosQ Compatibility Layer
// =============================================================================
// Table of Contents:
//   1. Circuit Conversion (LogosQ Circuit ↔ QuantumCircuitStructure)
//   2. State Conversion (LogosQ State ↔ QuantumStateVector)
//   3. Gate Adapters (LogosQ Gates → QuantumGateInterface)
//   4. Ansatz Bridge (LogosQ Ansatz → VariationalCircuitTemplate)
//   5. Direct LogosQ API Access
// =============================================================================
// Purpose: Provides seamless bidirectional conversion between LogosQ types
//          and V-OS types. Enables using LogosQ directly while
//          maintaining the full-word snake_case naming convention.
// =============================================================================
// Feature: This module is only compiled when `logosq_compat` feature is enabled.
// =============================================================================

#![cfg(feature = "logosq_compat")]

use crate::circuit_program::{
    GateApplicationInstance, ParameterizedQuantumCircuit, QuantumCircuitStructure,
    VariationalCircuitTemplate,
};
use crate::error::{CircuitError, QuantumResult, QuantumRuntimeError};
use crate::gate_operations::*;
use crate::state_backend::QuantumStateVector;
use num_complex::Complex64;
use std::sync::Arc;

// =============================================================================
// 1. Circuit Conversion
// =============================================================================

pub trait FromLogosQCircuit {
    fn from_logosq_circuit(circuit: &logosq::Circuit) -> QuantumResult<Self>
    where
        Self: Sized;
}

pub trait ToLogosQCircuit {
    fn to_logosq_circuit(&self) -> QuantumResult<logosq::Circuit>;
}

impl FromLogosQCircuit for QuantumCircuitStructure {
    fn from_logosq_circuit(logosq_circuit: &logosq::Circuit) -> QuantumResult<Self> {
        let n_qubits = logosq_circuit.num_qubits();
        let mut circuit = QuantumCircuitStructure::new(n_qubits);

        for op in logosq_circuit.operations() {
            let gate = convert_logosq_gate(op, n_qubits)?;
            let targets = op.qubits().to_vec();
            circuit.add_gate_application(gate, targets);
        }

        Ok(circuit)
    }
}

impl ToLogosQCircuit for QuantumCircuitStructure {
    fn to_logosq_circuit(&self) -> QuantumResult<logosq::Circuit> {
        let mut circuit = logosq::Circuit::new(self.number_of_quantum_bits());

        for gate_instance in self.gate_application_instances() {
            let gate_name = gate_instance.gate_name();
            let targets = &gate_instance.target_quantum_bits;

            match gate_name {
                "hadamard_gate" => {
                    circuit.h(targets[0]);
                }
                "pauli_x_gate" => {
                    circuit.x(targets[0]);
                }
                "pauli_y_gate" => {
                    circuit.y(targets[0]);
                }
                "pauli_z_gate" => {
                    circuit.z(targets[0]);
                }
                "controlled_not_gate" => {
                    circuit.cx(targets[0], targets[1]);
                }
                _ => {
                    return Err(QuantumRuntimeError::Circuit(CircuitError::ConstructionFailed(
                        format!("Cannot convert gate '{}' to LogosQ", gate_name),
                    )));
                }
            }
        }

        Ok(circuit)
    }
}

fn convert_logosq_gate(
    op: &logosq::Operation,
    n_qubits: usize,
) -> QuantumResult<Arc<dyn QuantumGateInterface>> {
    let qubits = op.qubits();
    let gate_type = op.gate_type();

    let gate: Arc<dyn QuantumGateInterface> = match gate_type.as_str() {
        "H" | "Hadamard" => Arc::new(HadamardGate::new(qubits[0])),
        "X" | "PauliX" => Arc::new(PauliXGate::new(qubits[0])),
        "Y" | "PauliY" => Arc::new(PauliYGate::new(qubits[0])),
        "Z" | "PauliZ" => Arc::new(PauliZGate::new(qubits[0])),
        "RX" | "RotationX" => {
            let theta = op.parameter().unwrap_or(0.0);
            Arc::new(RotationXGate::new(qubits[0], theta))
        }
        "RY" | "RotationY" => {
            let theta = op.parameter().unwrap_or(0.0);
            Arc::new(RotationYGate::new(qubits[0], theta))
        }
        "RZ" | "RotationZ" => {
            let theta = op.parameter().unwrap_or(0.0);
            Arc::new(RotationZGate::new(qubits[0], theta))
        }
        "CNOT" | "CX" => Arc::new(ControlledNotGate::new(qubits[0], qubits[1], n_qubits)),
        "CZ" | "ControlledZ" => {
            Arc::new(ControlledPhaseGate::new(qubits[0], qubits[1], std::f64::consts::PI, n_qubits))
        }
        "SWAP" => Arc::new(SwapGate::new(qubits[0], qubits[1], n_qubits)),
        "CCX" | "Toffoli" => Arc::new(ToffoliGate::new(qubits[0], qubits[1], qubits[2], n_qubits)),
        other => {
            return Err(QuantumRuntimeError::Circuit(CircuitError::ConstructionFailed(
                format!("Unsupported LogosQ gate type: {}", other),
            )));
        }
    };

    Ok(gate)
}

// =============================================================================
// 2. State Conversion
// =============================================================================

pub trait FromLogosQState {
    fn from_logosq_state(state: &logosq::State) -> QuantumResult<Self>
    where
        Self: Sized;
}

pub trait ToLogosQState {
    fn to_logosq_state(&self) -> QuantumResult<logosq::State>;
}

impl FromLogosQState for QuantumStateVector {
    fn from_logosq_state(logosq_state: &logosq::State) -> QuantumResult<Self> {
        let amplitudes: Vec<Complex64> = logosq_state
            .amplitudes()
            .iter()
            .map(|&a| Complex64::new(a.re, a.im))
            .collect();

        if !amplitudes.len().is_power_of_two() {
            return Err(QuantumRuntimeError::Circuit(CircuitError::ConstructionFailed(
                "LogosQ state amplitude count is not a power of 2".to_string(),
            )));
        }

        Ok(QuantumStateVector::from_amplitudes(amplitudes))
    }
}

impl ToLogosQState for QuantumStateVector {
    fn to_logosq_state(&self) -> QuantumResult<logosq::State> {
        let amplitudes: Vec<logosq::Complex> = self
            .amplitudes()
            .iter()
            .map(|a| logosq::Complex::new(a.re, a.im))
            .collect();

        Ok(logosq::State::from_amplitudes(amplitudes))
    }
}

// =============================================================================
// 3. Gate Adapters
// =============================================================================

pub struct LogosQGateAdapter {
    logosq_gate: logosq::Gate,
    target_qubits: Vec<usize>,
    n_qubits: usize,
}

impl LogosQGateAdapter {
    pub fn new(gate: logosq::Gate, targets: Vec<usize>, n_qubits: usize) -> Self {
        Self {
            logosq_gate: gate,
            target_qubits: targets,
            n_qubits,
        }
    }
}

impl std::fmt::Debug for LogosQGateAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogosQGateAdapter")
            .field("gate", &self.logosq_gate.name())
            .field("targets", &self.target_qubits)
            .finish()
    }
}

impl QuantumGateInterface for LogosQGateAdapter {
    fn apply_to_full_state_vector(&self, state: &mut QuantumStateVector) {
        if let Ok(logosq_state) = state.to_logosq_state() {
            let mut mutable_state = logosq_state;
            self.logosq_gate.apply(&mut mutable_state, &self.target_qubits);

            if let Ok(converted) = QuantumStateVector::from_logosq_state(&mutable_state) {
                *state = converted;
            }
        }
    }

    fn gate_name(&self) -> &str {
        "logosq_adapted_gate"
    }

    fn target_quantum_bits(&self) -> Vec<usize> {
        self.target_qubits.clone()
    }

    fn gate_matrix(&self) -> Vec<Vec<Complex64>> {
        self.logosq_gate
            .matrix()
            .iter()
            .map(|row| row.iter().map(|c| Complex64::new(c.re, c.im)).collect())
            .collect()
    }
}

// =============================================================================
// 4. Ansatz Bridge
// =============================================================================

pub struct LogosQAnsatzAdapter {
    n_qubits: usize,
    n_params: usize,
    ansatz_type: LogosQAnsatzType,
}

#[derive(Debug, Clone, Copy)]
pub enum LogosQAnsatzType {
    HardwareEfficient { layers: usize },
    Uccsd,
    Qaoa { depth: usize },
}

impl LogosQAnsatzAdapter {
    pub fn hardware_efficient(n_qubits: usize, layers: usize) -> Self {
        let n_params = n_qubits * layers * 3;
        Self {
            n_qubits,
            n_params,
            ansatz_type: LogosQAnsatzType::HardwareEfficient { layers },
        }
    }

    pub fn qaoa(n_qubits: usize, depth: usize) -> Self {
        let n_params = depth * 2;
        Self {
            n_qubits,
            n_params,
            ansatz_type: LogosQAnsatzType::Qaoa { depth },
        }
    }
}

impl VariationalCircuitTemplate for LogosQAnsatzAdapter {
    fn construct_quantum_circuit(&self, parameters: &[f64]) -> QuantumCircuitStructure {
        let mut circuit = QuantumCircuitStructure::new(self.n_qubits);

        match self.ansatz_type {
            LogosQAnsatzType::HardwareEfficient { layers } => {
                let mut param_idx = 0;
                for _layer in 0..layers {
                    for qubit in 0..self.n_qubits {
                        if param_idx < parameters.len() {
                            circuit.apply_rotation_y_gate(qubit, parameters[param_idx]);
                            param_idx += 1;
                        }
                        if param_idx < parameters.len() {
                            circuit.apply_rotation_z_gate(qubit, parameters[param_idx]);
                            param_idx += 1;
                        }
                    }
                    for qubit in 0..self.n_qubits.saturating_sub(1) {
                        circuit.apply_controlled_not_gate(qubit, qubit + 1);
                    }
                    for qubit in 0..self.n_qubits {
                        if param_idx < parameters.len() {
                            circuit.apply_rotation_y_gate(qubit, parameters[param_idx]);
                            param_idx += 1;
                        }
                    }
                }
            }
            LogosQAnsatzType::Qaoa { depth } => {
                for qubit in 0..self.n_qubits {
                    circuit.apply_hadamard_gate(qubit);
                }
                for layer in 0..depth {
                    let gamma = parameters.get(layer * 2).copied().unwrap_or(0.0);
                    let beta = parameters.get(layer * 2 + 1).copied().unwrap_or(0.0);

                    for qubit in 0..self.n_qubits.saturating_sub(1) {
                        circuit.apply_controlled_phase_gate(qubit, qubit + 1, 2.0 * gamma);
                    }

                    for qubit in 0..self.n_qubits {
                        circuit.apply_rotation_x_gate(qubit, 2.0 * beta);
                    }
                }
            }
            LogosQAnsatzType::Uccsd => {
                for qubit in 0..self.n_qubits {
                    circuit.apply_hadamard_gate(qubit);
                }
            }
        }

        circuit
    }

    fn number_of_parameters(&self) -> usize {
        self.n_params
    }

    fn number_of_quantum_bits(&self) -> usize {
        self.n_qubits
    }
}

// =============================================================================
// 5. Direct LogosQ API Access
// =============================================================================

pub mod direct {
    pub use logosq::*;
}

pub struct LogosQExecutor;

impl LogosQExecutor {
    pub fn run_circuit(circuit: &logosq::Circuit, shots: usize) -> Vec<Vec<u8>> {
        let mut results = Vec::with_capacity(shots);
        let state = circuit.execute();

        for _ in 0..shots {
            let measurement = state.measure();
            results.push(measurement);
        }

        results
    }

    pub fn run_vqe(
        ansatz: &logosq::Ansatz,
        hamiltonian: &logosq::Hamiltonian,
        initial_params: &[f64],
        max_iterations: usize,
    ) -> VqeResult {
        let mut params = initial_params.to_vec();
        let mut energy = f64::MAX;

        for iteration in 0..max_iterations {
            let circuit = ansatz.build(&params);
            let state = circuit.execute();
            energy = hamiltonian.expectation(&state);

            let gradients = ansatz.parameter_shift_gradients(&params, hamiltonian);

            let learning_rate = 0.1 / (1.0 + iteration as f64 * 0.01);
            for (p, g) in params.iter_mut().zip(gradients.iter()) {
                *p -= learning_rate * g;
            }

            if gradients.iter().map(|g| g.abs()).sum::<f64>() < 1e-6 {
                break;
            }
        }

        VqeResult {
            optimal_energy: energy,
            optimal_parameters: params,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VqeResult {
    pub optimal_energy: f64,
    pub optimal_parameters: Vec<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansatz_adapter() {
        let ansatz = LogosQAnsatzAdapter::hardware_efficient(2, 1);
        assert_eq!(ansatz.number_of_quantum_bits(), 2);
        assert!(ansatz.number_of_parameters() > 0);

        let params = vec![0.1; ansatz.number_of_parameters()];
        let circuit = ansatz.construct_quantum_circuit(&params);
        assert!(circuit.gate_count() > 0);
    }

    #[test]
    fn test_qaoa_ansatz() {
        let qaoa = LogosQAnsatzAdapter::qaoa(3, 2);
        assert_eq!(qaoa.number_of_quantum_bits(), 3);
        assert_eq!(qaoa.number_of_parameters(), 4);

        let params = vec![0.5, 0.3, 0.4, 0.2];
        let circuit = qaoa.construct_quantum_circuit(&params);
        assert!(circuit.gate_count() > 0);
    }
}
