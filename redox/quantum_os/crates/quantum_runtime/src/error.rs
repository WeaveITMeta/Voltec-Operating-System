// =============================================================================
// V-OS - Unified Error Types
// =============================================================================
// Table of Contents:
//   1. QuantumRuntimeError - Main error enum
//   2. CircuitError - Circuit construction errors
//   3. ExecutionError - Execution-time errors
//   4. MeasurementError - Measurement errors
//   5. BackendError - Backend-specific errors
//   6. LogosQ error bridging (feature-gated)
// =============================================================================
// Purpose: Unified error handling across the quantum runtime layer with
//          seamless bridging to LogosQ errors when logosq_compat is enabled.
// =============================================================================

use thiserror::Error;
use uuid::Uuid;

// =============================================================================
// 1. QuantumRuntimeError - Main error enum
// =============================================================================

#[derive(Debug, Error)]
pub enum QuantumRuntimeError {
    #[error("Circuit error: {0}")]
    Circuit(#[from] CircuitError),

    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),

    #[error("Measurement error: {0}")]
    Measurement(#[from] MeasurementError),

    #[error("Backend error: {0}")]
    Backend(#[from] BackendError),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Timeout after {0} milliseconds")]
    Timeout(u64),

    #[error("Internal error: {0}")]
    Internal(String),
}

// =============================================================================
// 2. CircuitError - Circuit construction errors
// =============================================================================

#[derive(Debug, Error)]
pub enum CircuitError {
    #[error("Invalid qubit index {index}: circuit has {total} qubits")]
    InvalidQubitIndex { index: usize, total: usize },

    #[error("Gate requires {required} qubits, but {provided} were provided")]
    QubitCountMismatch { required: usize, provided: usize },

    #[error("Parameter count mismatch: expected {expected}, got {actual}")]
    ParameterCountMismatch { expected: usize, actual: usize },

    #[error("Invalid gate parameter: {0}")]
    InvalidGateParameter(String),

    #[error("Circuit too large: {qubits} qubits exceeds maximum {max}")]
    CircuitTooLarge { qubits: usize, max: usize },

    #[error("Empty circuit: no gates to execute")]
    EmptyCircuit,

    #[error("Duplicate qubit in gate targets: qubit {0}")]
    DuplicateQubit(usize),

    #[error("Control and target qubits must be different")]
    SameControlTarget,

    #[error("Circuit construction failed: {0}")]
    ConstructionFailed(String),
}

// =============================================================================
// 3. ExecutionError - Execution-time errors
// =============================================================================

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("Job not found: {0}")]
    JobNotFound(Uuid),

    #[error("Job already completed: {0}")]
    JobAlreadyCompleted(Uuid),

    #[error("Job failed: {0}")]
    JobFailed(String),

    #[error("Execution cancelled for job {0}")]
    ExecutionCancelled(Uuid),

    #[error("Backend unavailable: {0}")]
    BackendUnavailable(String),

    #[error("Invalid shot count: {0} (must be > 0)")]
    InvalidShotCount(usize),

    #[error("State vector computation failed: {0}")]
    StateVectorFailed(String),

    #[error("Gradient computation failed: {0}")]
    GradientFailed(String),

    #[error("Async execution error: {0}")]
    AsyncError(String),

    #[error("Batch execution failed: {successful}/{total} jobs completed")]
    BatchFailed { successful: usize, total: usize },
}

// =============================================================================
// 4. MeasurementError - Measurement errors
// =============================================================================

#[derive(Debug, Error)]
pub enum MeasurementError {
    #[error("No measurements available")]
    NoMeasurements,

    #[error("Invalid bitstring length: expected {expected}, got {actual}")]
    InvalidBitstringLength { expected: usize, actual: usize },

    #[error("Stream not complete: {completed}/{total} shots")]
    StreamIncomplete { completed: usize, total: usize },

    #[error("Observable computation failed: {0}")]
    ObservableFailed(String),

    #[error("Invalid basis selection: {0}")]
    InvalidBasis(String),

    #[error("Statistics computation failed: {0}")]
    StatisticsFailed(String),
}

// =============================================================================
// 5. BackendError - Backend-specific errors
// =============================================================================

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("Dense backend: state vector too large for {qubits} qubits")]
    DenseStateTooLarge { qubits: usize },

    #[error("MPS backend: bond dimension exceeded maximum {max}")]
    MpsBondDimensionExceeded { max: usize },

    #[error("MPS backend: truncation error {error} exceeds threshold {threshold}")]
    MpsTruncationError { error: f64, threshold: f64 },

    #[error("GPU backend: initialization failed: {0}")]
    GpuInitFailed(String),

    #[error("GPU backend: memory allocation failed for {bytes} bytes")]
    GpuMemoryAlloc { bytes: usize },

    #[error("GPU backend: kernel execution failed: {0}")]
    GpuKernelFailed(String),

    #[error("Backend not available: {0}")]
    NotAvailable(String),

    #[error("Backend configuration error: {0}")]
    ConfigError(String),
}

// =============================================================================
// 6. LogosQ error bridging (feature-gated)
// =============================================================================

#[cfg(feature = "logosq_compat")]
pub mod logosq_bridge {
    use super::*;

    #[derive(Debug, Error)]
    pub enum LogosQBridgeError {
        #[error("LogosQ circuit conversion failed: {0}")]
        CircuitConversion(String),

        #[error("LogosQ state conversion failed: {0}")]
        StateConversion(String),

        #[error("LogosQ gate not supported: {0}")]
        UnsupportedGate(String),

        #[error("LogosQ ansatz conversion failed: {0}")]
        AnsatzConversion(String),

        #[error("LogosQ backend error: {0}")]
        BackendError(String),
    }

    impl From<LogosQBridgeError> for QuantumRuntimeError {
        fn from(err: LogosQBridgeError) -> Self {
            QuantumRuntimeError::Internal(err.to_string())
        }
    }
}

// =============================================================================
// Result type alias
// =============================================================================

pub type QuantumResult<T> = Result<T, QuantumRuntimeError>;

// =============================================================================
// Error context extension trait
// =============================================================================

pub trait ErrorContext<T> {
    fn context(self, msg: impl Into<String>) -> QuantumResult<T>;
    fn with_context<F>(self, f: F) -> QuantumResult<T>
    where
        F: FnOnce() -> String;
}

impl<T, E: std::error::Error> ErrorContext<T> for Result<T, E> {
    fn context(self, msg: impl Into<String>) -> QuantumResult<T> {
        self.map_err(|e| QuantumRuntimeError::Internal(format!("{}: {}", msg.into(), e)))
    }

    fn with_context<F>(self, f: F) -> QuantumResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| QuantumRuntimeError::Internal(format!("{}: {}", f(), e)))
    }
}

impl<T> ErrorContext<T> for Option<T> {
    fn context(self, msg: impl Into<String>) -> QuantumResult<T> {
        self.ok_or_else(|| QuantumRuntimeError::ResourceNotFound(msg.into()))
    }

    fn with_context<F>(self, f: F) -> QuantumResult<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| QuantumRuntimeError::ResourceNotFound(f()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_error() {
        let err = CircuitError::InvalidQubitIndex { index: 5, total: 3 };
        assert!(err.to_string().contains("5"));
        assert!(err.to_string().contains("3"));
    }

    #[test]
    fn test_error_conversion() {
        let circuit_err = CircuitError::EmptyCircuit;
        let runtime_err: QuantumRuntimeError = circuit_err.into();
        assert!(matches!(runtime_err, QuantumRuntimeError::Circuit(_)));
    }

    #[test]
    fn test_result_context() {
        let result: Result<(), std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        let quantum_result = result.context("Failed to read file");
        assert!(quantum_result.is_err());
    }
}
