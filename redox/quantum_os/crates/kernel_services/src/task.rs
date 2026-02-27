// =============================================================================
// V-OS - Task and Process Management
// =============================================================================
// Table of Contents:
//   1. ProcessContext - Classical process context
//   2. TaskContext - Lightweight task within a process
//   3. QuantumJobContext - Quantum job execution context
//   4. HybridScheduler - Schedules both classical and quantum tasks
// =============================================================================
// Purpose: Manages classical processes and quantum jobs. The hybrid scheduler
//          coordinates between classical CPU tasks and quantum device jobs,
//          implementing fair scheduling with priority support.
// =============================================================================

use crate::capability::CapabilitySet;
use crate::handle::{QuantumDeviceHandle, QuantumJobHandle, QuantumJobStatus};
use crate::message::CircuitProgramMessage;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use uuid::Uuid;

// =============================================================================
// 1. ProcessContext - Classical process context
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessContext {
    pub id: Uuid,
    pub name: String,
    pub state: ProcessState,
    pub priority: ProcessPriority,
    pub capabilities: CapabilitySet,
    pub parent_id: Option<Uuid>,
    pub child_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessState {
    Created,
    Ready,
    Running,
    Blocked,
    WaitingForQuantumResult,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProcessPriority {
    Idle = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Realtime = 4,
}

impl ProcessContext {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            state: ProcessState::Created,
            priority: ProcessPriority::Normal,
            capabilities: CapabilitySet::new(),
            parent_id: None,
            child_ids: Vec::new(),
        }
    }

    pub fn with_priority(mut self, priority: ProcessPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_parent(mut self, parent_id: Uuid) -> Self {
        self.parent_id = Some(parent_id);
        self
    }
}

// =============================================================================
// 2. TaskContext - Lightweight task within a process
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    pub id: Uuid,
    pub process_id: Uuid,
    pub name: String,
    pub state: TaskState,
    pub stack_size_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Sleeping,
    Completed,
}

impl TaskContext {
    pub fn new(process_id: Uuid, name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            process_id,
            name: name.into(),
            state: TaskState::Ready,
            stack_size_bytes: 1024 * 1024,
        }
    }

    pub fn with_stack_size(mut self, size_bytes: usize) -> Self {
        self.stack_size_bytes = size_bytes;
        self
    }
}

// =============================================================================
// 3. QuantumJobContext - Quantum job execution context
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumJobContext {
    pub job_handle: QuantumJobHandle,
    pub requesting_process_id: Uuid,
    pub circuit_program: CircuitProgramMessage,
    pub priority: QuantumJobPriority,
    pub queued_at: u64,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum QuantumJobPriority {
    Background = 0,
    Normal = 1,
    Elevated = 2,
    Critical = 3,
}

impl QuantumJobContext {
    pub fn new(
        device_handle: &QuantumDeviceHandle,
        requesting_process_id: Uuid,
        circuit_program: CircuitProgramMessage,
    ) -> Self {
        Self {
            job_handle: QuantumJobHandle::new(device_handle),
            requesting_process_id,
            circuit_program,
            priority: QuantumJobPriority::Normal,
            queued_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            started_at: None,
            completed_at: None,
        }
    }

    pub fn with_priority(mut self, priority: QuantumJobPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn mark_started(&mut self) {
        self.started_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        self.job_handle.set_status(QuantumJobStatus::Executing);
    }

    pub fn mark_completed(&mut self) {
        self.completed_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        self.job_handle.set_status(QuantumJobStatus::Completed);
    }

    pub fn execution_time_seconds(&self) -> Option<u64> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => Some(end - start),
            _ => None,
        }
    }
}

// =============================================================================
// 4. HybridScheduler - Schedules both classical and quantum tasks
// =============================================================================

#[derive(Debug)]
pub struct HybridScheduler {
    classical_ready_queue: RwLock<VecDeque<ProcessContext>>,
    quantum_job_queue: RwLock<VecDeque<QuantumJobContext>>,
    running_quantum_jobs: RwLock<Vec<QuantumJobContext>>,
    max_concurrent_quantum_jobs: usize,
}

impl HybridScheduler {
    pub fn new(max_concurrent_quantum_jobs: usize) -> Self {
        Self {
            classical_ready_queue: RwLock::new(VecDeque::new()),
            quantum_job_queue: RwLock::new(VecDeque::new()),
            running_quantum_jobs: RwLock::new(Vec::new()),
            max_concurrent_quantum_jobs,
        }
    }

    pub fn enqueue_process(&self, process: ProcessContext) {
        let mut queue = self.classical_ready_queue.write();
        queue.push_back(process);
        self.sort_classical_queue(&mut queue);
    }

    pub fn dequeue_next_process(&self) -> Option<ProcessContext> {
        let mut queue = self.classical_ready_queue.write();
        queue.pop_front()
    }

    pub fn submit_quantum_job(&self, job: QuantumJobContext) {
        let mut queue = self.quantum_job_queue.write();
        queue.push_back(job);
        self.sort_quantum_queue(&mut queue);
    }

    pub fn start_next_quantum_job(&self) -> Option<QuantumJobContext> {
        let running = self.running_quantum_jobs.read();
        if running.len() >= self.max_concurrent_quantum_jobs {
            return None;
        }
        drop(running);

        let mut queue = self.quantum_job_queue.write();
        if let Some(mut job) = queue.pop_front() {
            job.mark_started();
            let mut running = self.running_quantum_jobs.write();
            running.push(job.clone());
            Some(job)
        } else {
            None
        }
    }

    pub fn complete_quantum_job(&self, job_id: Uuid) -> Option<QuantumJobContext> {
        let mut running = self.running_quantum_jobs.write();
        if let Some(pos) = running
            .iter()
            .position(|j| j.job_handle.handle().id() == job_id)
        {
            let mut job = running.remove(pos);
            job.mark_completed();
            Some(job)
        } else {
            None
        }
    }

    pub fn quantum_queue_length(&self) -> usize {
        self.quantum_job_queue.read().len()
    }

    pub fn running_quantum_jobs_count(&self) -> usize {
        self.running_quantum_jobs.read().len()
    }

    fn sort_classical_queue(&self, queue: &mut VecDeque<ProcessContext>) {
        let mut vec: Vec<_> = queue.drain(..).collect();
        vec.sort_by(|a, b| b.priority.cmp(&a.priority));
        queue.extend(vec);
    }

    fn sort_quantum_queue(&self, queue: &mut VecDeque<QuantumJobContext>) {
        let mut vec: Vec<_> = queue.drain(..).collect();
        vec.sort_by(|a, b| b.priority.cmp(&a.priority));
        queue.extend(vec);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_scheduler_classical() {
        let scheduler = HybridScheduler::new(2);

        let p1 = ProcessContext::new("low_priority").with_priority(ProcessPriority::Low);
        let p2 = ProcessContext::new("high_priority").with_priority(ProcessPriority::High);

        scheduler.enqueue_process(p1);
        scheduler.enqueue_process(p2);

        let next = scheduler.dequeue_next_process().unwrap();
        assert_eq!(next.name, "high_priority");
    }

    #[test]
    fn test_hybrid_scheduler_quantum() {
        let scheduler = HybridScheduler::new(1);
        let device = QuantumDeviceHandle::new_simulator("test", 2, false);

        let circuit = CircuitProgramMessage::new(2);
        let job = QuantumJobContext::new(&device, Uuid::new_v4(), circuit);

        scheduler.submit_quantum_job(job);
        assert_eq!(scheduler.quantum_queue_length(), 1);

        let running_job = scheduler.start_next_quantum_job().unwrap();
        assert_eq!(scheduler.running_quantum_jobs_count(), 1);

        scheduler.complete_quantum_job(running_job.job_handle.handle().id());
        assert_eq!(scheduler.running_quantum_jobs_count(), 0);
    }
}
