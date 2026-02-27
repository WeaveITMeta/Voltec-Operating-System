// =============================================================================
// V-OS - Message Passing Channels
// =============================================================================
// Table of Contents:
//   1. MessageChannel - Bidirectional IPC channel
//   2. MeasurementStreamChannel - Streaming channel for measurements
//   3. ChannelRegistry - Global channel management
// =============================================================================
// Purpose: Implements message-passing IPC for the V-OS. Channels are
//          capability-protected and support both request-response and
//          streaming patterns for quantum measurement results.
// =============================================================================

use crate::message::IpcMessage;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

// =============================================================================
// 1. MessageChannel - Bidirectional IPC channel
// =============================================================================

#[derive(Debug)]
pub struct MessageChannel {
    id: Uuid,
    sender: Sender<IpcMessage>,
    receiver: Receiver<IpcMessage>,
}

impl MessageChannel {
    pub fn new_unbounded() -> (Self, Self) {
        let (tx_a, rx_a) = unbounded();
        let (tx_b, rx_b) = unbounded();

        let channel_a = Self {
            id: Uuid::new_v4(),
            sender: tx_a,
            receiver: rx_b,
        };

        let channel_b = Self {
            id: Uuid::new_v4(),
            sender: tx_b,
            receiver: rx_a,
        };

        (channel_a, channel_b)
    }

    pub fn new_bounded(capacity: usize) -> (Self, Self) {
        let (tx_a, rx_a) = bounded(capacity);
        let (tx_b, rx_b) = bounded(capacity);

        let channel_a = Self {
            id: Uuid::new_v4(),
            sender: tx_a,
            receiver: rx_b,
        };

        let channel_b = Self {
            id: Uuid::new_v4(),
            sender: tx_b,
            receiver: rx_a,
        };

        (channel_a, channel_b)
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn send(&self, message: IpcMessage) -> Result<(), ChannelError> {
        self.sender
            .send(message)
            .map_err(|_| ChannelError::SendFailed)
    }

    pub fn try_receive(&self) -> Option<IpcMessage> {
        self.receiver.try_recv().ok()
    }

    pub fn receive_blocking(&self) -> Result<IpcMessage, ChannelError> {
        self.receiver.recv().map_err(|_| ChannelError::Disconnected)
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ChannelError {
    #[error("Failed to send message")]
    SendFailed,
    #[error("Channel disconnected")]
    Disconnected,
    #[error("Channel not found")]
    NotFound,
    #[error("Channel capacity exceeded")]
    CapacityExceeded,
}

// =============================================================================
// 2. MeasurementStreamChannel - Streaming channel for measurements
// =============================================================================

#[derive(Debug)]
pub struct MeasurementStreamChannel {
    id: Uuid,
    job_id: Uuid,
    sender: Sender<MeasurementStreamEvent>,
    receiver: Receiver<MeasurementStreamEvent>,
    is_active: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
pub struct MeasurementStreamEvent {
    pub shot_index: usize,
    pub measurement_bitstring: Vec<u8>,
    pub timestamp_nanoseconds: u64,
    pub is_final: bool,
}

impl MeasurementStreamChannel {
    pub fn new(job_id: Uuid, buffer_size: usize) -> (MeasurementStreamProducer, Self) {
        let (tx, rx) = bounded(buffer_size);
        let is_active = Arc::new(RwLock::new(true));

        let producer = MeasurementStreamProducer {
            sender: tx.clone(),
            is_active: Arc::clone(&is_active),
        };

        let channel = Self {
            id: Uuid::new_v4(),
            job_id,
            sender: tx,
            receiver: rx,
            is_active,
        };

        (producer, channel)
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn job_id(&self) -> Uuid {
        self.job_id
    }

    pub fn next_event(&self) -> Option<MeasurementStreamEvent> {
        self.receiver.try_recv().ok()
    }

    pub fn next_event_blocking(&self) -> Result<MeasurementStreamEvent, ChannelError> {
        self.receiver.recv().map_err(|_| ChannelError::Disconnected)
    }

    pub fn close(&self) {
        let mut active = self.is_active.write();
        *active = false;
    }

    pub fn is_active(&self) -> bool {
        *self.is_active.read()
    }
}

#[derive(Debug, Clone)]
pub struct MeasurementStreamProducer {
    sender: Sender<MeasurementStreamEvent>,
    is_active: Arc<RwLock<bool>>,
}

impl MeasurementStreamProducer {
    pub fn emit(&self, event: MeasurementStreamEvent) -> Result<(), ChannelError> {
        if !*self.is_active.read() {
            return Err(ChannelError::Disconnected);
        }
        self.sender
            .send(event)
            .map_err(|_| ChannelError::SendFailed)
    }

    pub fn close(&self) {
        let mut active = self.is_active.write();
        *active = false;
    }
}

// =============================================================================
// 3. ChannelRegistry - Global channel management
// =============================================================================

#[derive(Debug, Default)]
pub struct ChannelRegistry {
    message_channels: RwLock<HashMap<Uuid, Arc<MessageChannel>>>,
    stream_channels: RwLock<HashMap<Uuid, Arc<MeasurementStreamChannel>>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_message_channel(&self, channel: MessageChannel) -> Uuid {
        let id = channel.id();
        let mut channels = self.message_channels.write();
        channels.insert(id, Arc::new(channel));
        id
    }

    pub fn get_message_channel(&self, id: Uuid) -> Option<Arc<MessageChannel>> {
        let channels = self.message_channels.read();
        channels.get(&id).cloned()
    }

    pub fn register_stream_channel(&self, channel: MeasurementStreamChannel) -> Uuid {
        let id = channel.id();
        let mut channels = self.stream_channels.write();
        channels.insert(id, Arc::new(channel));
        id
    }

    pub fn get_stream_channel(&self, id: Uuid) -> Option<Arc<MeasurementStreamChannel>> {
        let channels = self.stream_channels.read();
        channels.get(&id).cloned()
    }

    pub fn remove_channel(&self, id: Uuid) {
        let mut msg_channels = self.message_channels.write();
        let mut stream_channels = self.stream_channels.write();
        msg_channels.remove(&id);
        stream_channels.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::MessagePayload;

    #[test]
    fn test_message_channel_send_receive() {
        let (channel_a, channel_b) = MessageChannel::new_unbounded();

        let msg = IpcMessage::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            MessagePayload::Acknowledge(crate::message::AcknowledgeMessage {
                original_message_id: Uuid::new_v4(),
                status: crate::message::AcknowledgeStatus::Received,
            }),
        );

        channel_a.send(msg.clone()).unwrap();
        let received = channel_b.try_receive().unwrap();
        assert_eq!(received.id, msg.id);
    }

    #[test]
    fn test_measurement_stream() {
        let job_id = Uuid::new_v4();
        let (producer, channel) = MeasurementStreamChannel::new(job_id, 100);

        producer
            .emit(MeasurementStreamEvent {
                shot_index: 0,
                measurement_bitstring: vec![0, 1],
                timestamp_nanoseconds: 12345,
                is_final: false,
            })
            .unwrap();

        let event = channel.next_event().unwrap();
        assert_eq!(event.shot_index, 0);
        assert_eq!(event.measurement_bitstring, vec![0, 1]);
    }
}
