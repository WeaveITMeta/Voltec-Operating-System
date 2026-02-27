// =============================================================================
// V-OS - Circuit Program IR
// =============================================================================
// Table of Contents:
//   1. QuantumCircuitStructure - Renamed from LogosQ Circuit
//   2. GateApplicationInstance - Renamed from Operation
//   3. ParameterizedQuantumCircuit - Variational circuit support
//   4. VariationalCircuitTemplate - Ansatz trait wrapper
// =============================================================================
// Purpose: Provides the circuit program intermediate representation (IR) that
//          serves as the instruction format for quantum jobs. Wraps LogosQ
//          types with full-word snake_case naming.
// =============================================================================

use crate::gate_operations::QuantumGateInterface;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

// =============================================================================
// 1. QuantumCircuitStructure - Main circuit container
// =============================================================================

#[derive(Debug, Clone)]
pub struct QuantumCircuitStructure {
    id: Uuid,
    number_of_quantum_bits: usize,
    gate_application_instances: Vec<GateApplicationInstance>,
    noise_simulation_models: Vec<Arc<dyn NoiseSimulationModel>>,
}

impl QuantumCircuitStructure {
    pub fn new(number_of_quantum_bits: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            number_of_quantum_bits,
            gate_application_instances: Vec::new(),
            noise_simulation_models: Vec::new(),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn number_of_quantum_bits(&self) -> usize {
        self.number_of_quantum_bits
    }

    pub fn gate_count(&self) -> usize {
        self.gate_application_instances.len()
    }

    pub fn gate_application_instances(&self) -> &[GateApplicationInstance] {
        &self.gate_application_instances
    }

    pub fn add_gate_application(
        &mut self,
        gate: Arc<dyn QuantumGateInterface>,
        target_quantum_bits: Vec<usize>,
    ) -> &mut Self {
        let instance = GateApplicationInstance {
            quantum_gate_interface: gate,
            target_quantum_bits,
            operation_name: String::new(),
        };
        self.gate_application_instances.push(instance);
        self
    }

    pub fn add_noise_model(&mut self, model: Arc<dyn NoiseSimulationModel>) -> &mut Self {
        self.noise_simulation_models.push(model);
        self
    }

    pub fn apply_hadamard_gate(&mut self, qubit: usize) -> &mut Self {
        self.add_gate_application(
            Arc::new(crate::gate_operations::HadamardGate::new(qubit)),
            vec![qubit],
        )
    }

    pub fn apply_pauli_x_gate(&mut self, qubit: usize) -> &mut Self {
        self.add_gate_application(
            Arc::new(crate::gate_operations::PauliXGate::new(qubit)),
            vec![qubit],
        )
    }

    pub fn apply_pauli_y_gate(&mut self, qubit: usize) -> &mut Self {
        self.add_gate_application(
            Arc::new(crate::gate_operations::PauliYGate::new(qubit)),
            vec![qubit],
        )
    }

    pub fn apply_pauli_z_gate(&mut self, qubit: usize) -> &mut Self {
        self.add_gate_application(
            Arc::new(crate::gate_operations::PauliZGate::new(qubit)),
            vec![qubit],
        )
    }

    pub fn apply_rotation_x_gate(&mut self, qubit: usize, theta: f64) -> &mut Self {
        self.add_gate_application(
            Arc::new(crate::gate_operations::RotationXGate::new(qubit, theta)),
            vec![qubit],
        )
    }

    pub fn apply_rotation_y_gate(&mut self, qubit: usize, theta: f64) -> &mut Self {
        self.add_gate_application(
            Arc::new(crate::gate_operations::RotationYGate::new(qubit, theta)),
            vec![qubit],
        )
    }

    pub fn apply_rotation_z_gate(&mut self, qubit: usize, theta: f64) -> &mut Self {
        self.add_gate_application(
            Arc::new(crate::gate_operations::RotationZGate::new(qubit, theta)),
            vec![qubit],
        )
    }

    pub fn apply_controlled_not_gate(&mut self, control: usize, target: usize) -> &mut Self {
        self.add_gate_application(
            Arc::new(crate::gate_operations::ControlledNotGate::new(
                control,
                target,
                self.number_of_quantum_bits,
            )),
            vec![control, target],
        )
    }

    pub fn apply_controlled_phase_gate(
        &mut self,
        control: usize,
        target: usize,
        theta: f64,
    ) -> &mut Self {
        self.add_gate_application(
            Arc::new(crate::gate_operations::ControlledPhaseGate::new(
                control,
                target,
                theta,
                self.number_of_quantum_bits,
            )),
            vec![control, target],
        )
    }

    pub fn apply_swap_gate(&mut self, qubit_a: usize, qubit_b: usize) -> &mut Self {
        self.add_gate_application(
            Arc::new(crate::gate_operations::SwapGate::new(
                qubit_a,
                qubit_b,
                self.number_of_quantum_bits,
            )),
            vec![qubit_a, qubit_b],
        )
    }
}

// =============================================================================
// 2. GateApplicationInstance - Single gate operation
// =============================================================================

#[derive(Debug, Clone)]
pub struct GateApplicationInstance {
    pub quantum_gate_interface: Arc<dyn QuantumGateInterface>,
    pub target_quantum_bits: Vec<usize>,
    pub operation_name: String,
}

impl GateApplicationInstance {
    pub fn gate_name(&self) -> &str {
        self.quantum_gate_interface.gate_name()
    }

    pub fn target_qubits(&self) -> &[usize] {
        &self.target_quantum_bits
    }
}

// =============================================================================
// 3. ParameterizedQuantumCircuit - Variational circuit
// =============================================================================

pub struct ParameterizedQuantumCircuit {
    number_of_quantum_bits: usize,
    number_of_parameters: usize,
    circuit_builder: Box<dyn Fn(&[f64]) -> QuantumCircuitStructure + Send + Sync>,
}

impl ParameterizedQuantumCircuit {
    pub fn new<F>(number_of_quantum_bits: usize, number_of_parameters: usize, builder: F) -> Self
    where
        F: Fn(&[f64]) -> QuantumCircuitStructure + Send + Sync + 'static,
    {
        Self {
            number_of_quantum_bits,
            number_of_parameters,
            circuit_builder: Box::new(builder),
        }
    }

    pub fn construct_quantum_circuit(&self, parameters: &[f64]) -> QuantumCircuitStructure {
        assert_eq!(
            parameters.len(),
            self.number_of_parameters,
            "Parameter count mismatch: expected {}, got {}",
            self.number_of_parameters,
            parameters.len()
        );
        (self.circuit_builder)(parameters)
    }

    pub fn number_of_quantum_bits(&self) -> usize {
        self.number_of_quantum_bits
    }

    pub fn number_of_parameters(&self) -> usize {
        self.number_of_parameters
    }
}

impl std::fmt::Debug for ParameterizedQuantumCircuit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParameterizedQuantumCircuit")
            .field("number_of_quantum_bits", &self.number_of_quantum_bits)
            .field("number_of_parameters", &self.number_of_parameters)
            .finish()
    }
}

// =============================================================================
// 4. VariationalCircuitTemplate - Ansatz trait
// =============================================================================

pub trait VariationalCircuitTemplate: Send + Sync {
    fn construct_quantum_circuit(&self, parameters: &[f64]) -> QuantumCircuitStructure;
    fn number_of_parameters(&self) -> usize;
    fn number_of_quantum_bits(&self) -> usize;

    fn execute_gate_operations(
        &self,
        state: &mut crate::state_backend::QuantumStateVector,
        parameters: &[f64],
    ) {
        let circuit = self.construct_quantum_circuit(parameters);
        for gate_instance in circuit.gate_application_instances() {
            gate_instance
                .quantum_gate_interface
                .apply_to_full_state_vector(state);
        }
    }
}

impl VariationalCircuitTemplate for ParameterizedQuantumCircuit {
    fn construct_quantum_circuit(&self, parameters: &[f64]) -> QuantumCircuitStructure {
        self.construct_quantum_circuit(parameters)
    }

    fn number_of_parameters(&self) -> usize {
        self.number_of_parameters
    }

    fn number_of_quantum_bits(&self) -> usize {
        self.number_of_quantum_bits
    }
}

// =============================================================================
// Noise Simulation Model Trait
// =============================================================================

pub trait NoiseSimulationModel: Send + Sync + std::fmt::Debug {
    fn apply_noise(
        &self,
        state: &mut crate::state_backend::QuantumStateVector,
        qubit: usize,
    );
    fn noise_model_name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_creation() {
        let mut circuit = QuantumCircuitStructure::new(2);
        circuit.apply_hadamard_gate(0);
        circuit.apply_controlled_not_gate(0, 1);

        assert_eq!(circuit.number_of_quantum_bits(), 2);
        assert_eq!(circuit.gate_count(), 2);
    }

    #[test]
    fn test_parameterized_circuit() {
        let param_circuit = ParameterizedQuantumCircuit::new(2, 2, |params| {
            let mut circuit = QuantumCircuitStructure::new(2);
            circuit.apply_rotation_x_gate(0, params[0]);
            circuit.apply_rotation_y_gate(1, params[1]);
            circuit.apply_controlled_not_gate(0, 1);
            circuit
        });

        let circuit = param_circuit.construct_quantum_circuit(&[0.5, 1.0]);
        assert_eq!(circuit.gate_count(), 3);
    }
}
