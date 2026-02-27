// =============================================================================
// V-OS - IPC Message Types
// =============================================================================
// Table of Contents:
//   1. IpcMessage - Base message envelope
//   2. CircuitProgramMessage - Quantum circuit submission
//   3. MeasurementRequestMessage - Request measurement results
//   4. BasisSelectionUpdate - Update measurement basis
//   5. StatisticsSnapshotMessage - Statistics report
// =============================================================================
// Purpose: Defines message types for IPC between OS services. Extended with
//          quantum-specific message types for circuit submission, measurement
//          requests, and statistics streaming.
// =============================================================================

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =============================================================================
// 1. IpcMessage - Base message envelope
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcMessage {
    pub id: Uuid,
    pub sender_id: Uuid,
    pub recipient_id: Uuid,
    pub timestamp: u64,
    pub payload: MessagePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    CircuitProgram(CircuitProgramMessage),
    MeasurementRequest(MeasurementRequestMessage),
    BasisSelection(BasisSelectionUpdate),
    StatisticsSnapshot(StatisticsSnapshotMessage),
    MeasurementEvent(MeasurementEventMessage),
    Acknowledge(AcknowledgeMessage),
    Error(ErrorMessage),
}

impl IpcMessage {
    pub fn new(sender_id: Uuid, recipient_id: Uuid, payload: MessagePayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender_id,
            recipient_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            payload,
        }
    }
}

// =============================================================================
// 2. CircuitProgramMessage - Quantum circuit submission
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitProgramMessage {
    pub circuit_id: Uuid,
    pub number_of_quantum_bits: usize,
    pub gate_sequence: Vec<GateInstruction>,
    pub measurement_qubits: Vec<usize>,
    pub execution_shots: usize,
    pub optimization_level: OptimizationLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateInstruction {
    pub gate_type: GateType,
    pub target_quantum_bits: Vec<usize>,
    pub parameters: Vec<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum GateType {
    HadamardGate,
    PauliXGate,
    PauliYGate,
    PauliZGate,
    RotationXGate,
    RotationYGate,
    RotationZGate,
    ControlledNotGate,
    ControlledPhaseGate,
    SwapGate,
    ToffoliGate,
    ControlledRotationYGate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OptimizationLevel {
    None,
    Basic,
    Aggressive,
}

impl CircuitProgramMessage {
    pub fn new(number_of_quantum_bits: usize) -> Self {
        Self {
            circuit_id: Uuid::new_v4(),
            number_of_quantum_bits,
            gate_sequence: Vec::new(),
            measurement_qubits: (0..number_of_quantum_bits).collect(),
            execution_shots: 1024,
            optimization_level: OptimizationLevel::Basic,
        }
    }

    pub fn add_gate(&mut self, instruction: GateInstruction) {
        self.gate_sequence.push(instruction);
    }

    pub fn with_shots(mut self, shots: usize) -> Self {
        self.execution_shots = shots;
        self
    }
}

// =============================================================================
// 3. MeasurementRequestMessage - Request measurement results
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementRequestMessage {
    pub job_id: Uuid,
    pub request_type: MeasurementRequestType,
    pub timeout_milliseconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MeasurementRequestType {
    AllResults,
    LatestOnly,
    Streaming,
    ExpectationValue,
}

// =============================================================================
// 4. BasisSelectionUpdate - Update measurement basis
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasisSelectionUpdate {
    pub job_id: Uuid,
    pub qubit_bases: Vec<MeasurementBasis>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MeasurementBasis {
    ComputationalBasis,
    PauliXBasis,
    PauliYBasis,
    PauliZBasis,
    CustomBasis { theta: f64, phi: f64 },
}

impl Default for MeasurementBasis {
    fn default() -> Self {
        Self::ComputationalBasis
    }
}

// =============================================================================
// 5. StatisticsSnapshotMessage - Statistics report
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsSnapshotMessage {
    pub job_id: Uuid,
    pub total_shots: usize,
    pub completed_shots: usize,
    pub measurement_counts: Vec<(String, usize)>,
    pub expectation_values: Vec<f64>,
    pub execution_time_microseconds: u64,
}

// =============================================================================
// Additional Message Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementEventMessage {
    pub job_id: Uuid,
    pub shot_index: usize,
    pub measurement_result: Vec<u8>,
    pub timestamp_nanoseconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcknowledgeMessage {
    pub original_message_id: Uuid,
    pub status: AcknowledgeStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AcknowledgeStatus {
    Received,
    Accepted,
    Rejected,
    Processing,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub original_message_id: Option<Uuid>,
    pub error_code: u32,
    pub error_description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_message_creation() {
        let mut circuit = CircuitProgramMessage::new(2);
        circuit.add_gate(GateInstruction {
            gate_type: GateType::HadamardGate,
            target_quantum_bits: vec![0],
            parameters: vec![],
        });
        circuit.add_gate(GateInstruction {
            gate_type: GateType::ControlledNotGate,
            target_quantum_bits: vec![0, 1],
            parameters: vec![],
        });

        assert_eq!(circuit.gate_sequence.len(), 2);
        assert_eq!(circuit.number_of_quantum_bits, 2);
    }
}
