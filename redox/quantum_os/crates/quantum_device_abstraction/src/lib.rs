// =============================================================================
// V-OS - Quantum Device Abstraction
// =============================================================================
// Table of Contents:
//   1. QuantumDeviceInterface - Core device trait
//   2. SimulatorDevice - Local simulator implementation
//   3. DeviceRegistry - Device discovery and management
//   4. CalibrationProfile - Device calibration data
// =============================================================================
// Purpose: Provides hardware abstraction layer for quantum devices. Enables
//          uniform access to simulators and future hardware backends through
//          a capability-based interface.
// =============================================================================

use kernel_services::handle::{QuantumBackendType, QuantumDeviceHandle};
use kernel_services::message::CircuitProgramMessage;
use quantum_runtime::circuit_program::QuantumCircuitStructure;
use quantum_runtime::execution::{ExecutionBackend, ExecutionResult, QuantumExecutionEngine};
use quantum_runtime::measurement::MeasurementStream;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

// =============================================================================
// 1. QuantumDeviceInterface - Core device trait
// =============================================================================

pub trait QuantumDeviceInterface: Send + Sync {
    fn device_id(&self) -> Uuid;
    fn device_name(&self) -> &str;
    fn number_of_quantum_bits(&self) -> usize;
    fn backend_type(&self) -> QuantumBackendType;
    fn is_available(&self) -> bool;
    
    fn submit_circuit(
        &self,
        circuit: &QuantumCircuitStructure,
        shots: usize,
    ) -> Result<Uuid, DeviceError>;
    
    fn get_results(&self, job_id: Uuid) -> Result<Option<MeasurementStream>, DeviceError>;
    
    fn calibration_profile(&self) -> Option<&CalibrationProfile>;
}

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("Device not available")]
    NotAvailable,
    #[error("Job not found: {0}")]
    JobNotFound(Uuid),
    #[error("Circuit too large: requires {required} qubits, device has {available}")]
    CircuitTooLarge { required: usize, available: usize },
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Device busy")]
    DeviceBusy,
}

// =============================================================================
// 2. SimulatorDevice - Local simulator implementation
// =============================================================================

pub struct SimulatorDevice {
    id: Uuid,
    name: String,
    number_of_quantum_bits: usize,
    backend_type: QuantumBackendType,
    execution_engine: QuantumExecutionEngine,
    pending_jobs: RwLock<HashMap<Uuid, JobState>>,
    completed_results: RwLock<HashMap<Uuid, MeasurementStream>>,
}

#[derive(Debug, Clone)]
enum JobState {
    Queued,
    Running,
    Completed,
    Failed(String),
}

impl SimulatorDevice {
    pub fn new_dense_simulator(name: impl Into<String>, number_of_qubits: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            number_of_quantum_bits: number_of_qubits,
            backend_type: QuantumBackendType::FullStateVectorSimulator,
            execution_engine: QuantumExecutionEngine::new()
                .with_backend(ExecutionBackend::FullStateVectorSimulator),
            pending_jobs: RwLock::new(HashMap::new()),
            completed_results: RwLock::new(HashMap::new()),
        }
    }

    pub fn new_mps_simulator(name: impl Into<String>, number_of_qubits: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            number_of_quantum_bits: number_of_qubits,
            backend_type: QuantumBackendType::MatrixProductStateSimulator,
            execution_engine: QuantumExecutionEngine::new()
                .with_backend(ExecutionBackend::MatrixProductStateSimulator),
            pending_jobs: RwLock::new(HashMap::new()),
            completed_results: RwLock::new(HashMap::new()),
        }
    }

    fn execute_job(&self, job_id: Uuid, circuit: &QuantumCircuitStructure, shots: usize) {
        {
            let mut jobs = self.pending_jobs.write();
            jobs.insert(job_id, JobState::Running);
        }

        let stream = self.execution_engine.execute_and_measure(circuit, shots);

        {
            let mut jobs = self.pending_jobs.write();
            jobs.insert(job_id, JobState::Completed);
        }

        {
            let mut results = self.completed_results.write();
            results.insert(job_id, stream);
        }
    }
}

impl QuantumDeviceInterface for SimulatorDevice {
    fn device_id(&self) -> Uuid {
        self.id
    }

    fn device_name(&self) -> &str {
        &self.name
    }

    fn number_of_quantum_bits(&self) -> usize {
        self.number_of_quantum_bits
    }

    fn backend_type(&self) -> QuantumBackendType {
        self.backend_type
    }

    fn is_available(&self) -> bool {
        true
    }

    fn submit_circuit(
        &self,
        circuit: &QuantumCircuitStructure,
        shots: usize,
    ) -> Result<Uuid, DeviceError> {
        if circuit.number_of_quantum_bits() > self.number_of_quantum_bits {
            return Err(DeviceError::CircuitTooLarge {
                required: circuit.number_of_quantum_bits(),
                available: self.number_of_quantum_bits,
            });
        }

        let job_id = Uuid::new_v4();

        {
            let mut jobs = self.pending_jobs.write();
            jobs.insert(job_id, JobState::Queued);
        }

        self.execute_job(job_id, circuit, shots);

        Ok(job_id)
    }

    fn get_results(&self, job_id: Uuid) -> Result<Option<MeasurementStream>, DeviceError> {
        let jobs = self.pending_jobs.read();
        match jobs.get(&job_id) {
            None => Err(DeviceError::JobNotFound(job_id)),
            Some(JobState::Completed) => {
                drop(jobs);
                let mut results = self.completed_results.write();
                Ok(results.remove(&job_id))
            }
            Some(JobState::Failed(msg)) => Err(DeviceError::ExecutionFailed(msg.clone())),
            Some(_) => Ok(None),
        }
    }

    fn calibration_profile(&self) -> Option<&CalibrationProfile> {
        None
    }
}

// =============================================================================
// 3. DeviceRegistry - Device discovery and management
// =============================================================================

#[derive(Default)]
pub struct DeviceRegistry {
    devices: RwLock<HashMap<Uuid, Arc<dyn QuantumDeviceInterface>>>,
    device_handles: RwLock<HashMap<Uuid, QuantumDeviceHandle>>,
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_device(&self, device: Arc<dyn QuantumDeviceInterface>) -> Uuid {
        let device_id = device.device_id();
        let handle = QuantumDeviceHandle::new_simulator(
            device.device_name(),
            device.number_of_quantum_bits(),
            matches!(
                device.backend_type(),
                QuantumBackendType::MatrixProductStateSimulator
            ),
        );

        {
            let mut devices = self.devices.write();
            devices.insert(device_id, device);
        }

        {
            let mut handles = self.device_handles.write();
            handles.insert(device_id, handle);
        }

        device_id
    }

    pub fn get_device(&self, device_id: Uuid) -> Option<Arc<dyn QuantumDeviceInterface>> {
        let devices = self.devices.read();
        devices.get(&device_id).cloned()
    }

    pub fn get_device_handle(&self, device_id: Uuid) -> Option<QuantumDeviceHandle> {
        let handles = self.device_handles.read();
        handles.get(&device_id).cloned()
    }

    pub fn list_available_devices(&self) -> Vec<(Uuid, String, usize)> {
        let devices = self.devices.read();
        devices
            .iter()
            .filter(|(_, d)| d.is_available())
            .map(|(id, d)| (*id, d.device_name().to_string(), d.number_of_quantum_bits()))
            .collect()
    }

    pub fn find_suitable_device(&self, required_qubits: usize) -> Option<Arc<dyn QuantumDeviceInterface>> {
        let devices = self.devices.read();
        devices
            .values()
            .find(|d| d.is_available() && d.number_of_quantum_bits() >= required_qubits)
            .cloned()
    }

    pub fn create_default_simulators(&self) {
        let dense_sim = Arc::new(SimulatorDevice::new_dense_simulator(
            "default_dense_simulator",
            12,
        ));
        self.register_device(dense_sim);

        let mps_sim = Arc::new(SimulatorDevice::new_mps_simulator(
            "default_mps_simulator",
            25,
        ));
        self.register_device(mps_sim);
    }
}

// =============================================================================
// 4. CalibrationProfile - Device calibration data
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationProfile {
    pub device_id: Uuid,
    pub calibration_timestamp: u64,
    pub single_qubit_gate_fidelities: HashMap<usize, f64>,
    pub two_qubit_gate_fidelities: HashMap<(usize, usize), f64>,
    pub readout_fidelities: HashMap<usize, f64>,
    pub t1_times_microseconds: HashMap<usize, f64>,
    pub t2_times_microseconds: HashMap<usize, f64>,
    pub qubit_frequencies_ghz: HashMap<usize, f64>,
}

impl CalibrationProfile {
    pub fn new(device_id: Uuid) -> Self {
        Self {
            device_id,
            calibration_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            single_qubit_gate_fidelities: HashMap::new(),
            two_qubit_gate_fidelities: HashMap::new(),
            readout_fidelities: HashMap::new(),
            t1_times_microseconds: HashMap::new(),
            t2_times_microseconds: HashMap::new(),
            qubit_frequencies_ghz: HashMap::new(),
        }
    }

    pub fn average_single_qubit_fidelity(&self) -> f64 {
        if self.single_qubit_gate_fidelities.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.single_qubit_gate_fidelities.values().sum();
        sum / self.single_qubit_gate_fidelities.len() as f64
    }

    pub fn average_two_qubit_fidelity(&self) -> f64 {
        if self.two_qubit_gate_fidelities.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.two_qubit_gate_fidelities.values().sum();
        sum / self.two_qubit_gate_fidelities.len() as f64
    }

    pub fn worst_qubit(&self) -> Option<usize> {
        self.single_qubit_gate_fidelities
            .iter()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(&qubit, _)| qubit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulator_device_creation() {
        let device = SimulatorDevice::new_dense_simulator("test_sim", 4);
        assert_eq!(device.number_of_quantum_bits(), 4);
        assert!(device.is_available());
    }

    #[test]
    fn test_device_registry() {
        let registry = DeviceRegistry::new();
        registry.create_default_simulators();

        let devices = registry.list_available_devices();
        assert_eq!(devices.len(), 2);
    }

    #[test]
    fn test_circuit_submission() {
        let device = SimulatorDevice::new_dense_simulator("test", 2);
        let mut circuit = QuantumCircuitStructure::new(2);
        circuit.apply_hadamard_gate(0);
        circuit.apply_controlled_not_gate(0, 1);

        let job_id = device.submit_circuit(&circuit, 100).unwrap();
        let results = device.get_results(job_id).unwrap();
        assert!(results.is_some());
    }
}
