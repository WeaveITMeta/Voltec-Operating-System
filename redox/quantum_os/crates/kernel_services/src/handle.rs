// =============================================================================
// V-OS - Resource Handles
// =============================================================================
// Table of Contents:
//   1. ResourceHandle - Base handle type for all OS resources
//   2. QuantumDeviceHandle - Handle to quantum hardware/simulator
//   3. QuantumJobHandle - Handle to submitted quantum job
//   4. MeasurementStreamHandle - Handle to measurement result stream
//   5. CalibrationProfileHandle - Handle to device calibration data
// =============================================================================
// Purpose: Capability-based resource handles extended for quantum devices.
//          Every resource in the OS is accessed through handles, enabling
//          fine-grained access control and resource lifecycle management.
// =============================================================================

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// =============================================================================
// 1. ResourceHandle - Base handle type
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceHandle {
    id: Uuid,
    handle_type: HandleType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HandleType {
    Process,
    Memory,
    File,
    Network,
    QuantumDevice,
    QuantumJob,
    MeasurementStream,
    CalibrationProfile,
}

impl ResourceHandle {
    pub fn new(handle_type: HandleType) -> Self {
        Self {
            id: Uuid::new_v4(),
            handle_type,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn handle_type(&self) -> HandleType {
        self.handle_type
    }
}

impl fmt::Display for ResourceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.handle_type, self.id)
    }
}

impl fmt::Display for HandleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandleType::Process => write!(f, "PROC"),
            HandleType::Memory => write!(f, "MEM"),
            HandleType::File => write!(f, "FILE"),
            HandleType::Network => write!(f, "NET"),
            HandleType::QuantumDevice => write!(f, "QDEV"),
            HandleType::QuantumJob => write!(f, "QJOB"),
            HandleType::MeasurementStream => write!(f, "MSTR"),
            HandleType::CalibrationProfile => write!(f, "QCAL"),
        }
    }
}

// =============================================================================
// 2. QuantumDeviceHandle - Quantum hardware/simulator access
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumDeviceHandle {
    resource_handle: ResourceHandle,
    device_name: String,
    number_of_quantum_bits: usize,
    backend_type: QuantumBackendType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantumBackendType {
    FullStateVectorSimulator,
    MatrixProductStateSimulator,
    HardwareIonTrap,
    HardwareSuperconducting,
    HardwarePhotonic,
}

impl QuantumDeviceHandle {
    pub fn new_simulator(
        device_name: impl Into<String>,
        number_of_quantum_bits: usize,
        use_matrix_product_state: bool,
    ) -> Self {
        let backend_type = if use_matrix_product_state {
            QuantumBackendType::MatrixProductStateSimulator
        } else {
            QuantumBackendType::FullStateVectorSimulator
        };

        Self {
            resource_handle: ResourceHandle::new(HandleType::QuantumDevice),
            device_name: device_name.into(),
            number_of_quantum_bits,
            backend_type,
        }
    }

    pub fn handle(&self) -> &ResourceHandle {
        &self.resource_handle
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn number_of_quantum_bits(&self) -> usize {
        self.number_of_quantum_bits
    }

    pub fn backend_type(&self) -> QuantumBackendType {
        self.backend_type
    }
}

// =============================================================================
// 3. QuantumJobHandle - Submitted quantum job tracking
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumJobHandle {
    resource_handle: ResourceHandle,
    device_handle_id: Uuid,
    job_status: QuantumJobStatus,
    submitted_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantumJobStatus {
    Queued,
    Compiling,
    Executing,
    MeasuringResults,
    Completed,
    Failed,
    Cancelled,
}

impl QuantumJobHandle {
    pub fn new(device_handle: &QuantumDeviceHandle) -> Self {
        Self {
            resource_handle: ResourceHandle::new(HandleType::QuantumJob),
            device_handle_id: device_handle.handle().id(),
            job_status: QuantumJobStatus::Queued,
            submitted_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    pub fn handle(&self) -> &ResourceHandle {
        &self.resource_handle
    }

    pub fn status(&self) -> QuantumJobStatus {
        self.job_status
    }

    pub fn set_status(&mut self, status: QuantumJobStatus) {
        self.job_status = status;
    }
}

// =============================================================================
// 4. MeasurementStreamHandle - Measurement result streaming
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementStreamHandle {
    resource_handle: ResourceHandle,
    job_handle_id: Uuid,
    measurement_count: usize,
    is_active: bool,
}

impl MeasurementStreamHandle {
    pub fn new(job_handle: &QuantumJobHandle) -> Self {
        Self {
            resource_handle: ResourceHandle::new(HandleType::MeasurementStream),
            job_handle_id: job_handle.handle().id(),
            measurement_count: 0,
            is_active: true,
        }
    }

    pub fn handle(&self) -> &ResourceHandle {
        &self.resource_handle
    }

    pub fn increment_count(&mut self) {
        self.measurement_count += 1;
    }

    pub fn close(&mut self) {
        self.is_active = false;
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }
}

// =============================================================================
// 5. CalibrationProfileHandle - Device calibration data
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationProfileHandle {
    resource_handle: ResourceHandle,
    device_handle_id: Uuid,
    calibration_timestamp: u64,
    gate_fidelities: Vec<f64>,
    coherence_time_microseconds: f64,
}

impl CalibrationProfileHandle {
    pub fn new(device_handle: &QuantumDeviceHandle) -> Self {
        Self {
            resource_handle: ResourceHandle::new(HandleType::CalibrationProfile),
            device_handle_id: device_handle.handle().id(),
            calibration_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            gate_fidelities: Vec::new(),
            coherence_time_microseconds: 0.0,
        }
    }

    pub fn handle(&self) -> &ResourceHandle {
        &self.resource_handle
    }

    pub fn set_gate_fidelities(&mut self, fidelities: Vec<f64>) {
        self.gate_fidelities = fidelities;
    }

    pub fn set_coherence_time(&mut self, time_us: f64) {
        self.coherence_time_microseconds = time_us;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantum_device_handle_creation() {
        let device = QuantumDeviceHandle::new_simulator("test_simulator", 4, false);
        assert_eq!(device.number_of_quantum_bits(), 4);
        assert_eq!(
            device.backend_type(),
            QuantumBackendType::FullStateVectorSimulator
        );
    }

    #[test]
    fn test_job_handle_status_transitions() {
        let device = QuantumDeviceHandle::new_simulator("test", 2, false);
        let mut job = QuantumJobHandle::new(&device);
        assert_eq!(job.status(), QuantumJobStatus::Queued);

        job.set_status(QuantumJobStatus::Executing);
        assert_eq!(job.status(), QuantumJobStatus::Executing);
    }
}
