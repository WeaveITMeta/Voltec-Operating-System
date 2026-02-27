// =============================================================================
// V-OS - Streaming Infrastructure
// =============================================================================
// Table of Contents:
//   1. StreamingProtocol - Message format definitions
//   2. MeasurementStreamServer - WebSocket server for streaming
//   3. StreamingClient - Client interface
//   4. BackpressureHandler - Flow control
//   5. ReplayBuffer - Late-joining subscriber support
//   6. StreamAggregator - Rolling statistics
// =============================================================================
// Purpose: Provides real-time measurement streaming infrastructure using
//          WebSocket protocol. Supports multiple subscribers, backpressure
//          handling, and late-joining with replay buffers.
// =============================================================================

use crate::async_runtime::{AsyncMeasurementStream, CancellationToken, MeasurementBroadcaster};
use crate::error::{ExecutionError, QuantumResult, QuantumRuntimeError};
use crate::measurement::{MeasurementEvent, MeasurementStatistics};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, watch};
use uuid::Uuid;

// =============================================================================
// 1. StreamingProtocol - Message format definitions
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum StreamingMessage {
    MeasurementEvent(MeasurementEventPayload),
    StatisticsUpdate(StatisticsUpdatePayload),
    ProgressUpdate(ProgressUpdatePayload),
    JobStarted(JobStartedPayload),
    JobCompleted(JobCompletedPayload),
    Error(ErrorPayload),
    Heartbeat(HeartbeatPayload),
    Subscribe(SubscribePayload),
    Unsubscribe(UnsubscribePayload),
    Ack(AckPayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementEventPayload {
    pub job_id: Uuid,
    pub shot_index: usize,
    pub bitstring: String,
    pub timestamp_ns: u64,
}

impl From<MeasurementEvent> for MeasurementEventPayload {
    fn from(event: MeasurementEvent) -> Self {
        Self {
            job_id: event.job_id,
            shot_index: event.shot_index,
            bitstring: event.bitstring_as_string(),
            timestamp_ns: event.timestamp_nanoseconds,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsUpdatePayload {
    pub job_id: Uuid,
    pub total_shots: usize,
    pub completed_shots: usize,
    pub entropy: f64,
    pub top_bitstrings: Vec<(String, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdatePayload {
    pub job_id: Uuid,
    pub completed: usize,
    pub total: usize,
    pub progress_percent: f64,
    pub estimated_remaining_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStartedPayload {
    pub job_id: Uuid,
    pub circuit_id: Uuid,
    pub total_shots: usize,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobCompletedPayload {
    pub job_id: Uuid,
    pub total_shots: usize,
    pub execution_time_ms: u64,
    pub final_statistics: StatisticsUpdatePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub job_id: Option<Uuid>,
    pub error_code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatPayload {
    pub timestamp: u64,
    pub server_load: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribePayload {
    pub job_id: Uuid,
    pub include_measurements: bool,
    pub include_statistics: bool,
    pub statistics_interval_shots: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribePayload {
    pub job_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckPayload {
    pub message_id: u64,
}

// =============================================================================
// 2. MeasurementStreamServer - Streaming server
// =============================================================================

pub struct MeasurementStreamServer {
    server_id: Uuid,
    jobs: Arc<RwLock<HashMap<Uuid, JobStreamContext>>>,
    subscribers: Arc<RwLock<HashMap<Uuid, Vec<SubscriberHandle>>>>,
    global_broadcast: broadcast::Sender<StreamingMessage>,
    config: StreamingServerConfig,
    cancellation: CancellationToken,
}

#[derive(Debug, Clone)]
pub struct StreamingServerConfig {
    pub max_subscribers_per_job: usize,
    pub replay_buffer_size: usize,
    pub statistics_interval_shots: usize,
    pub heartbeat_interval: Duration,
    pub message_buffer_size: usize,
    pub backpressure_threshold: usize,
}

impl Default for StreamingServerConfig {
    fn default() -> Self {
        Self {
            max_subscribers_per_job: 100,
            replay_buffer_size: 1000,
            statistics_interval_shots: 100,
            heartbeat_interval: Duration::from_secs(30),
            message_buffer_size: 10000,
            backpressure_threshold: 5000,
        }
    }
}

struct JobStreamContext {
    job_id: Uuid,
    broadcaster: MeasurementBroadcaster,
    replay_buffer: Arc<RwLock<ReplayBuffer>>,
    aggregator: Arc<RwLock<StreamAggregator>>,
    backpressure: BackpressureHandler,
    total_shots: usize,
}

struct SubscriberHandle {
    subscriber_id: Uuid,
    sender: mpsc::Sender<StreamingMessage>,
    subscription: SubscribePayload,
}

impl MeasurementStreamServer {
    pub fn new(config: StreamingServerConfig) -> Self {
        let (global_tx, _) = broadcast::channel(config.message_buffer_size);

        Self {
            server_id: Uuid::new_v4(),
            jobs: Arc::new(RwLock::new(HashMap::new())),
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            global_broadcast: global_tx,
            config,
            cancellation: CancellationToken::new(),
        }
    }

    pub fn register_job(&self, job_id: Uuid, total_shots: usize) {
        let context = JobStreamContext {
            job_id,
            broadcaster: MeasurementBroadcaster::new(job_id, total_shots, self.config.message_buffer_size),
            replay_buffer: Arc::new(RwLock::new(ReplayBuffer::new(self.config.replay_buffer_size))),
            aggregator: Arc::new(RwLock::new(StreamAggregator::new(
                self.config.statistics_interval_shots,
            ))),
            backpressure: BackpressureHandler::new(self.config.backpressure_threshold),
            total_shots,
        };

        self.jobs.write().insert(job_id, context);
        self.subscribers.write().insert(job_id, Vec::new());

        let started_msg = StreamingMessage::JobStarted(JobStartedPayload {
            job_id,
            circuit_id: Uuid::new_v4(),
            total_shots,
            timestamp: current_timestamp(),
        });

        let _ = self.global_broadcast.send(started_msg);
    }

    pub async fn publish_measurement(&self, event: MeasurementEvent) -> QuantumResult<()> {
        let job_id = event.job_id;

        let (replay_buffer, aggregator, backpressure, should_emit_stats) = {
            let jobs = self.jobs.read();
            let context = jobs.get(&job_id).ok_or_else(|| {
                QuantumRuntimeError::Execution(ExecutionError::JobNotFound(job_id))
            })?;

            context.backpressure.check_and_wait().await;

            let payload = MeasurementEventPayload::from(event.clone());
            context.replay_buffer.write().push(StreamingMessage::MeasurementEvent(payload.clone()));

            let should_emit = context.aggregator.write().add_measurement(&event);

            (
                context.replay_buffer.clone(),
                context.aggregator.clone(),
                context.backpressure.clone(),
                should_emit,
            )
        };

        let msg = StreamingMessage::MeasurementEvent(MeasurementEventPayload::from(event));
        self.broadcast_to_job(job_id, msg).await?;

        if should_emit_stats {
            let stats = aggregator.read().current_statistics(job_id);
            let stats_msg = StreamingMessage::StatisticsUpdate(stats);
            self.broadcast_to_job(job_id, stats_msg).await?;
        }

        Ok(())
    }

    pub async fn subscribe(
        &self,
        job_id: Uuid,
        subscription: SubscribePayload,
    ) -> QuantumResult<mpsc::Receiver<StreamingMessage>> {
        let (tx, rx) = mpsc::channel(self.config.message_buffer_size);
        let subscriber_id = Uuid::new_v4();

        {
            let jobs = self.jobs.read();
            if let Some(context) = jobs.get(&job_id) {
                let replay = context.replay_buffer.read().get_all();
                for msg in replay {
                    let _ = tx.send(msg).await;
                }
            }
        }

        let handle = SubscriberHandle {
            subscriber_id,
            sender: tx,
            subscription,
        };

        self.subscribers.write()
            .entry(job_id)
            .or_default()
            .push(handle);

        Ok(rx)
    }

    pub fn unsubscribe(&self, job_id: Uuid, subscriber_id: Uuid) {
        if let Some(subs) = self.subscribers.write().get_mut(&job_id) {
            subs.retain(|h| h.subscriber_id != subscriber_id);
        }
    }

    pub async fn complete_job(&self, job_id: Uuid, execution_time_ms: u64) -> QuantumResult<()> {
        let final_stats = {
            let jobs = self.jobs.read();
            let context = jobs.get(&job_id).ok_or_else(|| {
                QuantumRuntimeError::Execution(ExecutionError::JobNotFound(job_id))
            })?;

            context.aggregator.read().current_statistics(job_id)
        };

        let completed_msg = StreamingMessage::JobCompleted(JobCompletedPayload {
            job_id,
            total_shots: final_stats.total_shots,
            execution_time_ms,
            final_statistics: final_stats,
        });

        self.broadcast_to_job(job_id, completed_msg).await?;

        Ok(())
    }

    async fn broadcast_to_job(&self, job_id: Uuid, msg: StreamingMessage) -> QuantumResult<()> {
        let subscribers = self.subscribers.read();
        if let Some(subs) = subscribers.get(&job_id) {
            for sub in subs {
                let should_send = match &msg {
                    StreamingMessage::MeasurementEvent(_) => sub.subscription.include_measurements,
                    StreamingMessage::StatisticsUpdate(_) => sub.subscription.include_statistics,
                    _ => true,
                };

                if should_send {
                    let _ = sub.sender.send(msg.clone()).await;
                }
            }
        }

        let _ = self.global_broadcast.send(msg);
        Ok(())
    }

    pub fn subscriber_count(&self, job_id: Uuid) -> usize {
        self.subscribers
            .read()
            .get(&job_id)
            .map(|s| s.len())
            .unwrap_or(0)
    }

    pub fn shutdown(&self) {
        self.cancellation.cancel();
    }
}

// =============================================================================
// 4. BackpressureHandler - Flow control
// =============================================================================

#[derive(Debug, Clone)]
pub struct BackpressureHandler {
    pending_count: Arc<AtomicUsize>,
    threshold: usize,
    notify: Arc<tokio::sync::Notify>,
}

impl BackpressureHandler {
    pub fn new(threshold: usize) -> Self {
        Self {
            pending_count: Arc::new(AtomicUsize::new(0)),
            threshold,
            notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    pub async fn check_and_wait(&self) {
        while self.pending_count.load(Ordering::SeqCst) >= self.threshold {
            self.notify.notified().await;
        }
        self.pending_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn release(&self) {
        self.pending_count.fetch_sub(1, Ordering::SeqCst);
        self.notify.notify_one();
    }

    pub fn current_pressure(&self) -> f64 {
        self.pending_count.load(Ordering::SeqCst) as f64 / self.threshold as f64
    }

    pub fn is_under_pressure(&self) -> bool {
        self.pending_count.load(Ordering::SeqCst) >= self.threshold / 2
    }
}

// =============================================================================
// 5. ReplayBuffer - Late-joining subscriber support
// =============================================================================

#[derive(Debug)]
pub struct ReplayBuffer {
    buffer: VecDeque<StreamingMessage>,
    capacity: usize,
    message_id: AtomicU64,
}

impl ReplayBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
            message_id: AtomicU64::new(0),
        }
    }

    pub fn push(&mut self, msg: StreamingMessage) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(msg);
        self.message_id.fetch_add(1, Ordering::SeqCst);
    }

    pub fn get_all(&self) -> Vec<StreamingMessage> {
        self.buffer.iter().cloned().collect()
    }

    pub fn get_since(&self, message_id: u64) -> Vec<StreamingMessage> {
        let current_id = self.message_id.load(Ordering::SeqCst);
        if message_id >= current_id {
            return Vec::new();
        }

        let skip = if current_id - message_id > self.buffer.len() as u64 {
            0
        } else {
            self.buffer.len() - (current_id - message_id) as usize
        };

        self.buffer.iter().skip(skip).cloned().collect()
    }

    pub fn current_message_id(&self) -> u64 {
        self.message_id.load(Ordering::SeqCst)
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

// =============================================================================
// 6. StreamAggregator - Rolling statistics
// =============================================================================

#[derive(Debug)]
pub struct StreamAggregator {
    bitstring_counts: HashMap<String, usize>,
    total_count: usize,
    emit_interval: usize,
    since_last_emit: usize,
}

impl StreamAggregator {
    pub fn new(emit_interval: usize) -> Self {
        Self {
            bitstring_counts: HashMap::new(),
            total_count: 0,
            emit_interval,
            since_last_emit: 0,
        }
    }

    pub fn add_measurement(&mut self, event: &MeasurementEvent) -> bool {
        let bitstring = event.bitstring_as_string();
        *self.bitstring_counts.entry(bitstring).or_insert(0) += 1;
        self.total_count += 1;
        self.since_last_emit += 1;

        if self.since_last_emit >= self.emit_interval {
            self.since_last_emit = 0;
            true
        } else {
            false
        }
    }

    pub fn current_statistics(&self, job_id: Uuid) -> StatisticsUpdatePayload {
        let mut top: Vec<_> = self
            .bitstring_counts
            .iter()
            .map(|(k, &v)| (k.clone(), v as f64 / self.total_count as f64))
            .collect();

        top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        top.truncate(10);

        let entropy = self.compute_entropy();

        StatisticsUpdatePayload {
            job_id,
            total_shots: self.total_count,
            completed_shots: self.total_count,
            entropy,
            top_bitstrings: top,
        }
    }

    fn compute_entropy(&self) -> f64 {
        if self.total_count == 0 {
            return 0.0;
        }

        self.bitstring_counts
            .values()
            .map(|&count| {
                let p = count as f64 / self.total_count as f64;
                if p > 0.0 {
                    -p * p.log2()
                } else {
                    0.0
                }
            })
            .sum()
    }

    pub fn reset(&mut self) {
        self.bitstring_counts.clear();
        self.total_count = 0;
        self.since_last_emit = 0;
    }
}

// =============================================================================
// Utility functions
// =============================================================================

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_buffer() {
        let mut buffer = ReplayBuffer::new(5);

        for i in 0..10 {
            buffer.push(StreamingMessage::Heartbeat(HeartbeatPayload {
                timestamp: i,
                server_load: 0.5,
            }));
        }

        assert_eq!(buffer.len(), 5);
        assert_eq!(buffer.current_message_id(), 10);
    }

    #[test]
    fn test_stream_aggregator() {
        let mut aggregator = StreamAggregator::new(5);
        let job_id = Uuid::new_v4();

        for i in 0..10 {
            let event = MeasurementEvent::new(job_id, i, vec![0, i as u8 % 2]);
            let should_emit = aggregator.add_measurement(&event);

            if i == 4 || i == 9 {
                assert!(should_emit);
            }
        }

        let stats = aggregator.current_statistics(job_id);
        assert_eq!(stats.total_shots, 10);
    }

    #[tokio::test]
    async fn test_backpressure() {
        let handler = BackpressureHandler::new(3);

        handler.check_and_wait().await;
        handler.check_and_wait().await;

        assert!(handler.is_under_pressure());
        assert!((handler.current_pressure() - 0.67).abs() < 0.1);

        handler.release();
        assert!(!handler.is_under_pressure());
    }
}
