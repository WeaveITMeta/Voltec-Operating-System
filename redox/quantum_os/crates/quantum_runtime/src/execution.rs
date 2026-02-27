// =============================================================================
// V-OS - Execution Engine
// =============================================================================
// Table of Contents:
//   1. QuantumExecutionEngine - Main execution coordinator
//   2. CircuitExecutor - Executes circuits on backends
//   3. ParameterShiftGradientMethod - Gradient computation
//   4. ExecutionResult - Result container
// =============================================================================
// Purpose: Coordinates quantum circuit execution across different backends,
//          handles parameter-shift gradient computation for variational
//          algorithms, and manages execution results.
// =============================================================================

use crate::circuit_program::{QuantumCircuitStructure, VariationalCircuitTemplate};
use crate::measurement::{MeasurementEvent, MeasurementStatistics, MeasurementStream, ObservableOperatorInterface};
use crate::state_backend::{MatrixProductStateConfiguration, MatrixProductStateRepresentation, QuantumStateBackendInterface, QuantumStateVector};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

// =============================================================================
// 1. QuantumExecutionEngine - Main execution coordinator
// =============================================================================

#[derive(Debug)]
pub struct QuantumExecutionEngine {
    engine_id: Uuid,
    default_backend: ExecutionBackend,
    maximum_qubits_for_dense: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionBackend {
    FullStateVectorSimulator,
    MatrixProductStateSimulator,
    AutoSelect,
}

impl Default for QuantumExecutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl QuantumExecutionEngine {
    pub fn new() -> Self {
        Self {
            engine_id: Uuid::new_v4(),
            default_backend: ExecutionBackend::AutoSelect,
            maximum_qubits_for_dense: 12,
        }
    }

    pub fn with_backend(mut self, backend: ExecutionBackend) -> Self {
        self.default_backend = backend;
        self
    }

    pub fn with_max_dense_qubits(mut self, max_qubits: usize) -> Self {
        self.maximum_qubits_for_dense = max_qubits;
        self
    }

    pub fn execute_circuit(
        &self,
        circuit: &QuantumCircuitStructure,
        shots: usize,
    ) -> ExecutionResult {
        let backend = self.select_backend(circuit.number_of_quantum_bits());
        let executor = CircuitExecutor::new(backend);
        executor.execute(circuit, shots)
    }

    pub fn execute_and_measure(
        &self,
        circuit: &QuantumCircuitStructure,
        shots: usize,
    ) -> MeasurementStream {
        let job_id = Uuid::new_v4();
        let mut stream = MeasurementStream::new(job_id, shots);

        let backend = self.select_backend(circuit.number_of_quantum_bits());
        
        for shot_idx in 0..shots {
            let mut state = match backend {
                ExecutionBackend::FullStateVectorSimulator | ExecutionBackend::AutoSelect => {
                    Box::new(QuantumStateVector::zero_state(circuit.number_of_quantum_bits()))
                        as Box<dyn QuantumStateBackendInterface>
                }
                ExecutionBackend::MatrixProductStateSimulator => {
                    Box::new(MatrixProductStateRepresentation::zero_state(
                        circuit.number_of_quantum_bits(),
                        MatrixProductStateConfiguration::default(),
                    )) as Box<dyn QuantumStateBackendInterface>
                }
            };

            for gate_instance in circuit.gate_application_instances() {
                if let Some(dense_state) = state.as_any_mut().downcast_mut::<QuantumStateVector>() {
                    gate_instance.quantum_gate_interface.apply_to_full_state_vector(dense_state);
                }
            }

            let bitstring = state.measure_all();
            let event = MeasurementEvent::new(job_id, shot_idx, bitstring);
            stream.add_event(event);
        }

        stream
    }

    pub fn compute_expectation_value(
        &self,
        circuit: &QuantumCircuitStructure,
        observable: &dyn ObservableOperatorInterface,
    ) -> f64 {
        let mut state = QuantumStateVector::zero_state(circuit.number_of_quantum_bits());
        
        for gate_instance in circuit.gate_application_instances() {
            gate_instance.quantum_gate_interface.apply_to_full_state_vector(&mut state);
        }

        observable.compute_expectation_value(&state)
    }

    fn select_backend(&self, number_of_qubits: usize) -> ExecutionBackend {
        match self.default_backend {
            ExecutionBackend::AutoSelect => {
                if number_of_qubits <= self.maximum_qubits_for_dense {
                    ExecutionBackend::FullStateVectorSimulator
                } else {
                    ExecutionBackend::MatrixProductStateSimulator
                }
            }
            other => other,
        }
    }
}

trait AsAny {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl AsAny for Box<dyn QuantumStateBackendInterface> {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self.as_mut() as &mut dyn std::any::Any
    }
}

impl<T: 'static> AsAny for T {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// =============================================================================
// 2. CircuitExecutor - Executes circuits on specific backend
// =============================================================================

#[derive(Debug)]
pub struct CircuitExecutor {
    backend: ExecutionBackend,
}

impl CircuitExecutor {
    pub fn new(backend: ExecutionBackend) -> Self {
        Self { backend }
    }

    pub fn execute(&self, circuit: &QuantumCircuitStructure, shots: usize) -> ExecutionResult {
        let job_id = Uuid::new_v4();
        let start_time = std::time::Instant::now();

        let mut state = QuantumStateVector::zero_state(circuit.number_of_quantum_bits());

        for gate_instance in circuit.gate_application_instances() {
            gate_instance.quantum_gate_interface.apply_to_full_state_vector(&mut state);
        }

        let mut measurements = Vec::with_capacity(shots);
        for shot_idx in 0..shots {
            let bitstring = state.measure_all();
            measurements.push(MeasurementEvent::new(job_id, shot_idx, bitstring));
        }

        let statistics = MeasurementStatistics::from_events(&measurements);
        let execution_time = start_time.elapsed();

        ExecutionResult {
            job_id,
            circuit_id: circuit.id(),
            final_state: Some(state),
            measurements,
            statistics,
            execution_time_microseconds: execution_time.as_micros() as u64,
            backend_used: self.backend,
        }
    }

    pub fn execute_statevector_only(&self, circuit: &QuantumCircuitStructure) -> QuantumStateVector {
        let mut state = QuantumStateVector::zero_state(circuit.number_of_quantum_bits());

        for gate_instance in circuit.gate_application_instances() {
            gate_instance.quantum_gate_interface.apply_to_full_state_vector(&mut state);
        }

        state
    }
}

// =============================================================================
// 3. ParameterShiftGradientMethod - Gradient computation
// =============================================================================

pub trait GradientComputationInterface: Send + Sync {
    fn compute_gradient(
        &self,
        ansatz: &dyn VariationalCircuitTemplate,
        observable: &dyn ObservableOperatorInterface,
        parameters: &[f64],
    ) -> Vec<f64>;
}

#[derive(Debug, Clone)]
pub struct ParameterShiftGradientMethod {
    shift_value: f64,
}

impl Default for ParameterShiftGradientMethod {
    fn default() -> Self {
        Self::new()
    }
}

impl ParameterShiftGradientMethod {
    pub fn new() -> Self {
        Self {
            shift_value: std::f64::consts::FRAC_PI_2,
        }
    }

    pub fn with_shift(mut self, shift: f64) -> Self {
        self.shift_value = shift;
        self
    }

    fn evaluate_expectation(
        &self,
        ansatz: &dyn VariationalCircuitTemplate,
        observable: &dyn ObservableOperatorInterface,
        parameters: &[f64],
    ) -> f64 {
        let circuit = ansatz.construct_quantum_circuit(parameters);
        let mut state = QuantumStateVector::zero_state(ansatz.number_of_quantum_bits());

        for gate_instance in circuit.gate_application_instances() {
            gate_instance.quantum_gate_interface.apply_to_full_state_vector(&mut state);
        }

        observable.compute_expectation_value(&state)
    }
}

impl GradientComputationInterface for ParameterShiftGradientMethod {
    fn compute_gradient(
        &self,
        ansatz: &dyn VariationalCircuitTemplate,
        observable: &dyn ObservableOperatorInterface,
        parameters: &[f64],
    ) -> Vec<f64> {
        let num_params = parameters.len();
        let mut gradient = vec![0.0; num_params];

        for i in 0..num_params {
            let mut params_plus = parameters.to_vec();
            params_plus[i] += self.shift_value;
            let expectation_plus = self.evaluate_expectation(ansatz, observable, &params_plus);

            let mut params_minus = parameters.to_vec();
            params_minus[i] -= self.shift_value;
            let expectation_minus = self.evaluate_expectation(ansatz, observable, &params_minus);

            gradient[i] = (expectation_plus - expectation_minus) / 2.0;
        }

        gradient
    }
}

// =============================================================================
// 4. ExecutionResult - Result container
// =============================================================================

#[derive(Debug)]
pub struct ExecutionResult {
    pub job_id: Uuid,
    pub circuit_id: Uuid,
    pub final_state: Option<QuantumStateVector>,
    pub measurements: Vec<MeasurementEvent>,
    pub statistics: MeasurementStatistics,
    pub execution_time_microseconds: u64,
    pub backend_used: ExecutionBackend,
}

impl ExecutionResult {
    pub fn success(&self) -> bool {
        !self.measurements.is_empty()
    }

    pub fn total_shots(&self) -> usize {
        self.measurements.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit_program::ParameterizedQuantumCircuit;
    use crate::measurement::PauliZObservable;

    #[test]
    fn test_circuit_execution() {
        let engine = QuantumExecutionEngine::new();
        let mut circuit = QuantumCircuitStructure::new(2);
        circuit.apply_hadamard_gate(0);
        circuit.apply_controlled_not_gate(0, 1);

        let result = engine.execute_circuit(&circuit, 100);
        assert!(result.success());
        assert_eq!(result.total_shots(), 100);
    }

    #[test]
    fn test_expectation_value() {
        let engine = QuantumExecutionEngine::new();
        let mut circuit = QuantumCircuitStructure::new(1);
        
        let observable = PauliZObservable::new(0);
        let expectation = engine.compute_expectation_value(&circuit, &observable);
        assert!((expectation - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_parameter_shift_gradient() {
        let ansatz = ParameterizedQuantumCircuit::new(1, 1, |params| {
            let mut circuit = QuantumCircuitStructure::new(1);
            circuit.apply_rotation_y_gate(0, params[0]);
            circuit
        });

        let observable = PauliZObservable::new(0);
        let gradient_method = ParameterShiftGradientMethod::new();

        let params = vec![0.0];
        let gradient = gradient_method.compute_gradient(&ansatz, &observable, &params);

        assert_eq!(gradient.len(), 1);
        assert!((gradient[0] - 0.0).abs() < 1e-6);
    }
}
