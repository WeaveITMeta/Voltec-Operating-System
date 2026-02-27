// =============================================================================
// V-OS - Async Execution Runtime
// =============================================================================
// Table of Contents:
//   1. AsyncQuantumExecutionEngine - Async circuit execution
//   2. AsyncMeasurementStream - Streaming measurements via channels
//   3. QuantumJobFuture - Awaitable job handle
//   4. BatchExecutor - Parallel job execution
//   5. CancellationToken - Job cancellation support
// =============================================================================
// Purpose: Provides asynchronous quantum circuit execution with Tokio runtime,
//          enabling non-blocking measurement streaming, batch execution, and
//          cancellation support for long-running quantum jobs.
// =============================================================================

use crate::circuit_program::QuantumCircuitStructure;
use crate::error::{ExecutionError, QuantumResult, QuantumRuntimeError};
use crate::execution::{CircuitExecutor, ExecutionBackend, ExecutionResult};
use crate::measurement::{MeasurementEvent, MeasurementStatistics, MeasurementStream};
use crate::state_backend::QuantumStateVector;

use async_trait::async_trait;
use futures::future::BoxFuture;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::timeout;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

// =============================================================================
// 1. AsyncQuantumExecutionEngine - Async circuit execution
// =============================================================================

#[derive(Debug)]
pub struct AsyncQuantumExecutionEngine {
    engine_id: Uuid,
    default_backend: ExecutionBackend,
    max_concurrent_jobs: usize,
    active_jobs: Arc<RwLock<HashMap<Uuid, JobHandle>>>,
    job_semaphore: Arc<tokio::sync::Semaphore>,
}

impl Default for AsyncQuantumExecutionEngine {
    fn default() -> Self {
        Self::new(num_cpus::get())
    }
}

impl AsyncQuantumExecutionEngine {
    pub fn new(max_concurrent_jobs: usize) -> Self {
        Self {
            engine_id: Uuid::new_v4(),
            default_backend: ExecutionBackend::AutoSelect,
            max_concurrent_jobs,
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            job_semaphore: Arc::new(tokio::sync::Semaphore::new(max_concurrent_jobs)),
        }
    }

    pub fn with_backend(mut self, backend: ExecutionBackend) -> Self {
        self.default_backend = backend;
        self
    }

    pub async fn execute_circuit_async(
        &self,
        circuit: QuantumCircuitStructure,
        shots: usize,
    ) -> QuantumResult<ExecutionResult> {
        let job_id = Uuid::new_v4();
        let permit = self
            .job_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| QuantumRuntimeError::Execution(ExecutionError::BackendUnavailable(
                "Semaphore closed".to_string(),
            )))?;

        let executor = CircuitExecutor::new(self.default_backend);

        let result = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            executor.execute(&circuit, shots)
        })
        .await
        .map_err(|e| QuantumRuntimeError::Execution(ExecutionError::AsyncError(e.to_string())))?;

        Ok(result)
    }

    pub async fn execute_with_timeout(
        &self,
        circuit: QuantumCircuitStructure,
        shots: usize,
        timeout_duration: Duration,
    ) -> QuantumResult<ExecutionResult> {
        timeout(timeout_duration, self.execute_circuit_async(circuit, shots))
            .await
            .map_err(|_| QuantumRuntimeError::Timeout(timeout_duration.as_millis() as u64))?
    }

    pub fn submit_job(
        &self,
        circuit: QuantumCircuitStructure,
        shots: usize,
    ) -> QuantumJobFuture {
        let job_id = Uuid::new_v4();
        let cancellation = CancellationToken::new();
        let (result_tx, result_rx) = oneshot::channel();

        let job_handle = JobHandle {
            job_id,
            cancellation_token: cancellation.clone(),
            status: Arc::new(RwLock::new(JobStatus::Queued)),
        };

        {
            let mut jobs = self.active_jobs.write();
            jobs.insert(job_id, job_handle.clone());
        }

        let semaphore = self.job_semaphore.clone();
        let backend = self.default_backend;
        let active_jobs = self.active_jobs.clone();
        let cancel_token = cancellation.clone();

        tokio::spawn(async move {
            let permit = match semaphore.acquire_owned().await {
                Ok(p) => p,
                Err(_) => {
                    let _ = result_tx.send(Err(QuantumRuntimeError::Execution(
                        ExecutionError::BackendUnavailable("Semaphore closed".to_string()),
                    )));
                    return;
                }
            };

            if cancel_token.is_cancelled() {
                let _ = result_tx.send(Err(QuantumRuntimeError::Cancelled));
                return;
            }

            {
                if let Some(handle) = active_jobs.read().get(&job_id) {
                    *handle.status.write() = JobStatus::Running;
                }
            }

            let executor = CircuitExecutor::new(backend);
            let result = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                executor.execute(&circuit, shots)
            })
            .await;

            let final_result = match result {
                Ok(r) => Ok(r),
                Err(e) => Err(QuantumRuntimeError::Execution(ExecutionError::AsyncError(
                    e.to_string(),
                ))),
            };

            {
                if let Some(handle) = active_jobs.read().get(&job_id) {
                    *handle.status.write() = if final_result.is_ok() {
                        JobStatus::Completed
                    } else {
                        JobStatus::Failed
                    };
                }
            }

            let _ = result_tx.send(final_result);
        });

        QuantumJobFuture {
            job_id,
            result_rx,
            cancellation_token: cancellation,
        }
    }

    pub fn cancel_job(&self, job_id: Uuid) -> bool {
        let jobs = self.active_jobs.read();
        if let Some(handle) = jobs.get(&job_id) {
            handle.cancellation_token.cancel();
            true
        } else {
            false
        }
    }

    pub fn job_status(&self, job_id: Uuid) -> Option<JobStatus> {
        let jobs = self.active_jobs.read();
        jobs.get(&job_id).map(|h| h.status.read().clone())
    }

    pub fn active_job_count(&self) -> usize {
        self.active_jobs.read().len()
    }
}

#[derive(Debug, Clone)]
struct JobHandle {
    job_id: Uuid,
    cancellation_token: CancellationToken,
    status: Arc<RwLock<JobStatus>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

// =============================================================================
// 2. AsyncMeasurementStream - Streaming measurements via channels
// =============================================================================

pub struct AsyncMeasurementStream {
    job_id: Uuid,
    total_shots: usize,
    receiver: mpsc::Receiver<MeasurementEvent>,
    completed_count: Arc<AtomicUsize>,
    is_complete: Arc<AtomicBool>,
}

impl AsyncMeasurementStream {
    pub fn new(job_id: Uuid, total_shots: usize) -> (Self, MeasurementStreamSender) {
        let (tx, rx) = mpsc::channel(1024);
        let completed_count = Arc::new(AtomicUsize::new(0));
        let is_complete = Arc::new(AtomicBool::new(false));

        let stream = Self {
            job_id,
            total_shots,
            receiver: rx,
            completed_count: completed_count.clone(),
            is_complete: is_complete.clone(),
        };

        let sender = MeasurementStreamSender {
            sender: tx,
            completed_count,
            is_complete,
            total_shots,
        };

        (stream, sender)
    }

    pub async fn next(&mut self) -> Option<MeasurementEvent> {
        self.receiver.recv().await
    }

    pub async fn collect_all(mut self) -> Vec<MeasurementEvent> {
        let mut events = Vec::with_capacity(self.total_shots);
        while let Some(event) = self.receiver.recv().await {
            events.push(event);
        }
        events
    }

    pub async fn collect_with_timeout(
        mut self,
        timeout_duration: Duration,
    ) -> QuantumResult<Vec<MeasurementEvent>> {
        let mut events = Vec::with_capacity(self.total_shots);

        loop {
            match timeout(timeout_duration, self.receiver.recv()).await {
                Ok(Some(event)) => events.push(event),
                Ok(None) => break,
                Err(_) => {
                    return Err(QuantumRuntimeError::Timeout(
                        timeout_duration.as_millis() as u64,
                    ));
                }
            }
        }

        Ok(events)
    }

    pub fn completed_count(&self) -> usize {
        self.completed_count.load(Ordering::SeqCst)
    }

    pub fn total_shots(&self) -> usize {
        self.total_shots
    }

    pub fn progress(&self) -> f64 {
        self.completed_count() as f64 / self.total_shots as f64
    }

    pub fn is_complete(&self) -> bool {
        self.is_complete.load(Ordering::SeqCst)
    }
}

pub struct MeasurementStreamSender {
    sender: mpsc::Sender<MeasurementEvent>,
    completed_count: Arc<AtomicUsize>,
    is_complete: Arc<AtomicBool>,
    total_shots: usize,
}

impl MeasurementStreamSender {
    pub async fn send(&self, event: MeasurementEvent) -> QuantumResult<()> {
        self.sender
            .send(event)
            .await
            .map_err(|_| QuantumRuntimeError::Execution(ExecutionError::AsyncError(
                "Measurement stream receiver dropped".to_string(),
            )))?;

        let count = self.completed_count.fetch_add(1, Ordering::SeqCst) + 1;
        if count >= self.total_shots {
            self.is_complete.store(true, Ordering::SeqCst);
        }

        Ok(())
    }

    pub fn complete(&self) {
        self.is_complete.store(true, Ordering::SeqCst);
    }
}

// =============================================================================
// 3. QuantumJobFuture - Awaitable job handle
// =============================================================================

pub struct QuantumJobFuture {
    pub job_id: Uuid,
    result_rx: oneshot::Receiver<QuantumResult<ExecutionResult>>,
    cancellation_token: CancellationToken,
}

impl QuantumJobFuture {
    pub fn job_id(&self) -> Uuid {
        self.job_id
    }

    pub async fn await_result(self) -> QuantumResult<ExecutionResult> {
        self.result_rx
            .await
            .map_err(|_| QuantumRuntimeError::Execution(ExecutionError::AsyncError(
                "Job result channel closed".to_string(),
            )))?
    }

    pub async fn await_with_timeout(
        self,
        timeout_duration: Duration,
    ) -> QuantumResult<ExecutionResult> {
        timeout(timeout_duration, self.await_result())
            .await
            .map_err(|_| QuantumRuntimeError::Timeout(timeout_duration.as_millis() as u64))?
    }

    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
}

// =============================================================================
// 4. BatchExecutor - Parallel job execution
// =============================================================================

pub struct BatchExecutor {
    engine: Arc<AsyncQuantumExecutionEngine>,
}

impl BatchExecutor {
    pub fn new(engine: Arc<AsyncQuantumExecutionEngine>) -> Self {
        Self { engine }
    }

    pub async fn execute_batch(
        &self,
        circuits: Vec<(QuantumCircuitStructure, usize)>,
    ) -> Vec<QuantumResult<ExecutionResult>> {
        let mut handles = Vec::with_capacity(circuits.len());

        for (circuit, shots) in circuits {
            let engine = self.engine.clone();
            let handle = tokio::spawn(async move {
                engine.execute_circuit_async(circuit, shots).await
            });
            handles.push(handle);
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            let result = handle
                .await
                .unwrap_or_else(|e| Err(QuantumRuntimeError::Execution(
                    ExecutionError::AsyncError(e.to_string()),
                )));
            results.push(result);
        }

        results
    }

    pub async fn execute_batch_with_progress<F>(
        &self,
        circuits: Vec<(QuantumCircuitStructure, usize)>,
        progress_callback: F,
    ) -> Vec<QuantumResult<ExecutionResult>>
    where
        F: Fn(usize, usize) + Send + Sync + 'static,
    {
        let total = circuits.len();
        let completed = Arc::new(AtomicUsize::new(0));
        let callback = Arc::new(progress_callback);
        let mut handles = Vec::with_capacity(circuits.len());

        for (circuit, shots) in circuits {
            let engine = self.engine.clone();
            let completed = completed.clone();
            let callback = callback.clone();

            let handle = tokio::spawn(async move {
                let result = engine.execute_circuit_async(circuit, shots).await;
                let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                callback(done, total);
                result
            });
            handles.push(handle);
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            let result = handle
                .await
                .unwrap_or_else(|e| Err(QuantumRuntimeError::Execution(
                    ExecutionError::AsyncError(e.to_string()),
                )));
            results.push(result);
        }

        results
    }

    pub async fn map_circuits<F, R>(
        &self,
        circuits: Vec<QuantumCircuitStructure>,
        shots: usize,
        mapper: F,
    ) -> Vec<QuantumResult<R>>
    where
        F: Fn(ExecutionResult) -> R + Send + Sync + Clone + 'static,
        R: Send + 'static,
    {
        let circuit_shots: Vec<_> = circuits.into_iter().map(|c| (c, shots)).collect();
        let results = self.execute_batch(circuit_shots).await;

        results
            .into_iter()
            .map(|r| r.map(|exec_result| mapper(exec_result)))
            .collect()
    }
}

// =============================================================================
// 5. CancellationToken - Job cancellation support
// =============================================================================

#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    notify: Arc<tokio::sync::Notify>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        self.notify.notified().await;
    }

    pub fn child_token(&self) -> CancellationToken {
        CancellationToken {
            cancelled: self.cancelled.clone(),
            notify: self.notify.clone(),
        }
    }
}

// =============================================================================
// MeasurementBroadcaster - Multi-subscriber measurement streaming
// =============================================================================

pub struct MeasurementBroadcaster {
    job_id: Uuid,
    sender: broadcast::Sender<MeasurementEvent>,
    total_shots: usize,
    completed: Arc<AtomicUsize>,
}

impl MeasurementBroadcaster {
    pub fn new(job_id: Uuid, total_shots: usize, capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            job_id,
            sender,
            total_shots,
            completed: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn subscribe(&self) -> BroadcastStream<MeasurementEvent> {
        BroadcastStream::new(self.sender.subscribe())
    }

    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    pub fn broadcast(&self, event: MeasurementEvent) -> QuantumResult<()> {
        self.sender.send(event).map_err(|_| {
            QuantumRuntimeError::Execution(ExecutionError::AsyncError(
                "No active subscribers".to_string(),
            ))
        })?;
        self.completed.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn completed_count(&self) -> usize {
        self.completed.load(Ordering::SeqCst)
    }

    pub fn progress(&self) -> f64 {
        self.completed_count() as f64 / self.total_shots as f64
    }
}

// =============================================================================
// Async trait for pluggable execution backends
// =============================================================================

#[async_trait]
pub trait AsyncExecutionBackend: Send + Sync {
    async fn execute_async(
        &self,
        circuit: &QuantumCircuitStructure,
        shots: usize,
    ) -> QuantumResult<ExecutionResult>;

    async fn execute_streaming(
        &self,
        circuit: &QuantumCircuitStructure,
        shots: usize,
    ) -> QuantumResult<AsyncMeasurementStream>;

    fn backend_name(&self) -> &str;
    fn max_qubits(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cancellation_token() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());

        let token2 = token.child_token();
        token.cancel();

        assert!(token.is_cancelled());
        assert!(token2.is_cancelled());
    }

    #[tokio::test]
    async fn test_async_measurement_stream() {
        let job_id = Uuid::new_v4();
        let (mut stream, sender) = AsyncMeasurementStream::new(job_id, 10);

        tokio::spawn(async move {
            for i in 0..10 {
                let event = MeasurementEvent::new(job_id, i, vec![0, 1]);
                sender.send(event).await.unwrap();
            }
        });

        let mut count = 0;
        while let Some(_event) = stream.next().await {
            count += 1;
            if count >= 10 {
                break;
            }
        }

        assert_eq!(count, 10);
    }

    #[tokio::test]
    async fn test_job_status() {
        let engine = AsyncQuantumExecutionEngine::new(2);
        let mut circuit = QuantumCircuitStructure::new(2);
        circuit.apply_hadamard_gate(0);

        let job = engine.submit_job(circuit, 10);
        let job_id = job.job_id();

        let result = job.await_result().await;
        assert!(result.is_ok());

        let status = engine.job_status(job_id);
        assert_eq!(status, Some(JobStatus::Completed));
    }
}
