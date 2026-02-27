// =============================================================================
// V-OS - WebTransport/UDP Streaming Infrastructure
// =============================================================================
// Table of Contents:
//   1. WebTransport Protocol Types
//   2. QUIC Transport Layer
//   3. Datagram Channel (Unreliable, Low-Latency)
//   4. Stream Channel (Reliable, Ordered)
//   5. Hybrid Transport (Adaptive UDP/TCP)
//   6. OS-Level UDP Socket Integration
//   7. Connection Management
// =============================================================================
// Purpose: Provides WebTransport (HTTP/3 over QUIC/UDP) for ultra-low-latency
//          quantum measurement streaming. Supports both unreliable datagrams
//          for real-time data and reliable streams for critical results.
// =============================================================================

use crate::error::{ExecutionError, QuantumResult, QuantumRuntimeError};
use crate::measurement::MeasurementEvent;
use crate::streaming::{MeasurementEventPayload, StreamingMessage};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, oneshot};
use uuid::Uuid;

// =============================================================================
// 1. WebTransport Protocol Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebTransportConfig {
    pub bind_address: SocketAddr,
    pub max_concurrent_streams: u32,
    pub max_datagram_size: usize,
    pub idle_timeout: Duration,
    pub keep_alive_interval: Duration,
    pub enable_0rtt: bool,
    pub congestion_control: CongestionControl,
    pub priority_scheduling: bool,
}

impl Default for WebTransportConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0:4433".parse().unwrap(),
            max_concurrent_streams: 100,
            max_datagram_size: 1200,
            idle_timeout: Duration::from_secs(30),
            keep_alive_interval: Duration::from_secs(5),
            enable_0rtt: true,
            congestion_control: CongestionControl::Bbr,
            priority_scheduling: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CongestionControl {
    Cubic,
    Bbr,
    Reno,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportMode {
    Datagram,
    Stream,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "channel", content = "data")]
pub enum TransportMessage {
    Datagram(DatagramPayload),
    Stream(StreamPayload),
    Control(ControlPayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatagramPayload {
    pub sequence: u64,
    pub timestamp_ns: u64,
    pub job_id: Uuid,
    pub measurement: MeasurementEventPayload,
    pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamPayload {
    pub stream_id: u64,
    pub message: StreamingMessage,
    pub requires_ack: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlPayload {
    Connect { client_id: Uuid, capabilities: ClientCapabilities },
    Disconnect { client_id: Uuid, reason: String },
    Subscribe { job_id: Uuid, mode: TransportMode },
    Unsubscribe { job_id: Uuid },
    Ping { timestamp: u64 },
    Pong { timestamp: u64, server_time: u64 },
    FlowControl { window_size: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub supports_datagrams: bool,
    pub supports_streams: bool,
    pub max_datagram_size: usize,
    pub preferred_mode: TransportMode,
}

// =============================================================================
// 2. QUIC Transport Layer
// =============================================================================

pub struct QuicTransportServer {
    server_id: Uuid,
    config: WebTransportConfig,
    connections: Arc<RwLock<HashMap<Uuid, ConnectionState>>>,
    datagram_tx: broadcast::Sender<DatagramPayload>,
    stream_tx: mpsc::Sender<StreamPayload>,
    metrics: Arc<TransportMetrics>,
    is_running: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct ConnectionState {
    client_id: Uuid,
    remote_addr: SocketAddr,
    capabilities: ClientCapabilities,
    subscribed_jobs: Vec<Uuid>,
    transport_mode: TransportMode,
    connected_at: Instant,
    last_activity: Arc<RwLock<Instant>>,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    datagrams_sent: AtomicU64,
    datagrams_lost: AtomicU64,
    rtt_estimate: Arc<RwLock<Duration>>,
}

impl QuicTransportServer {
    pub fn new(config: WebTransportConfig) -> Self {
        let (datagram_tx, _) = broadcast::channel(10000);
        let (stream_tx, _) = mpsc::channel(1000);

        Self {
            server_id: Uuid::new_v4(),
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
            datagram_tx,
            stream_tx,
            metrics: Arc::new(TransportMetrics::new()),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(&self) -> QuantumResult<()> {
        self.is_running.store(true, Ordering::SeqCst);
        tracing::info!(
            "WebTransport server starting on {} (QUIC/UDP)",
            self.config.bind_address
        );

        let metrics = self.metrics.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            while is_running.load(Ordering::SeqCst) {
                metrics.update_statistics();
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        Ok(())
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        tracing::info!("WebTransport server stopping");
    }

    pub fn accept_connection(
        &self,
        client_id: Uuid,
        remote_addr: SocketAddr,
        capabilities: ClientCapabilities,
    ) -> QuantumResult<()> {
        let state = ConnectionState {
            client_id,
            remote_addr,
            capabilities: capabilities.clone(),
            subscribed_jobs: Vec::new(),
            transport_mode: capabilities.preferred_mode,
            connected_at: Instant::now(),
            last_activity: Arc::new(RwLock::new(Instant::now())),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            datagrams_sent: AtomicU64::new(0),
            datagrams_lost: AtomicU64::new(0),
            rtt_estimate: Arc::new(RwLock::new(Duration::from_millis(50))),
        };

        self.connections.write().insert(client_id, state);
        self.metrics.connections_total.fetch_add(1, Ordering::SeqCst);

        tracing::debug!("Client {} connected from {}", client_id, remote_addr);
        Ok(())
    }

    pub fn disconnect(&self, client_id: Uuid) {
        if self.connections.write().remove(&client_id).is_some() {
            self.metrics.connections_total.fetch_sub(1, Ordering::SeqCst);
            tracing::debug!("Client {} disconnected", client_id);
        }
    }

    pub fn subscribe_to_job(&self, client_id: Uuid, job_id: Uuid, mode: TransportMode) -> bool {
        if let Some(conn) = self.connections.write().get_mut(&client_id) {
            conn.subscribed_jobs.push(job_id);
            conn.transport_mode = mode;
            true
        } else {
            false
        }
    }

    pub fn connection_count(&self) -> usize {
        self.connections.read().len()
    }

    pub fn metrics(&self) -> &TransportMetrics {
        &self.metrics
    }
}

// =============================================================================
// 3. Datagram Channel (Unreliable, Low-Latency)
// =============================================================================

pub struct DatagramChannel {
    job_id: Uuid,
    sequence: AtomicU64,
    sender: broadcast::Sender<DatagramPayload>,
    config: DatagramConfig,
    stats: Arc<DatagramStats>,
}

#[derive(Debug, Clone)]
pub struct DatagramConfig {
    pub max_payload_size: usize,
    pub priority_levels: u8,
    pub drop_policy: DropPolicy,
    pub redundancy_factor: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum DropPolicy {
    DropOldest,
    DropLowestPriority,
    DropNewest,
    NeverDrop,
}

impl Default for DatagramConfig {
    fn default() -> Self {
        Self {
            max_payload_size: 1200,
            priority_levels: 4,
            drop_policy: DropPolicy::DropOldest,
            redundancy_factor: 0.0,
        }
    }
}

#[derive(Debug)]
pub struct DatagramStats {
    pub sent: AtomicU64,
    pub received: AtomicU64,
    pub dropped: AtomicU64,
    pub out_of_order: AtomicU64,
    pub duplicates: AtomicU64,
}

impl DatagramStats {
    pub fn new() -> Self {
        Self {
            sent: AtomicU64::new(0),
            received: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            out_of_order: AtomicU64::new(0),
            duplicates: AtomicU64::new(0),
        }
    }

    pub fn loss_rate(&self) -> f64 {
        let sent = self.sent.load(Ordering::SeqCst) as f64;
        let received = self.received.load(Ordering::SeqCst) as f64;
        if sent > 0.0 {
            1.0 - (received / sent)
        } else {
            0.0
        }
    }
}

impl DatagramChannel {
    pub fn new(job_id: Uuid, config: DatagramConfig) -> Self {
        let (sender, _) = broadcast::channel(10000);

        Self {
            job_id,
            sequence: AtomicU64::new(0),
            sender,
            config,
            stats: Arc::new(DatagramStats::new()),
        }
    }

    pub fn send_measurement(&self, event: &MeasurementEvent, priority: u8) -> QuantumResult<u64> {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);

        let payload = DatagramPayload {
            sequence: seq,
            timestamp_ns: current_timestamp_ns(),
            job_id: self.job_id,
            measurement: MeasurementEventPayload::from(event.clone()),
            priority: priority.min(self.config.priority_levels - 1),
        };

        self.sender.send(payload).map_err(|_| {
            QuantumRuntimeError::Execution(ExecutionError::AsyncError(
                "No datagram subscribers".to_string(),
            ))
        })?;

        self.stats.sent.fetch_add(1, Ordering::SeqCst);
        Ok(seq)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DatagramPayload> {
        self.sender.subscribe()
    }

    pub fn stats(&self) -> &DatagramStats {
        &self.stats
    }
}

// =============================================================================
// 4. Stream Channel (Reliable, Ordered)
// =============================================================================

pub struct StreamChannel {
    stream_id: u64,
    job_id: Uuid,
    sender: mpsc::Sender<StreamPayload>,
    receiver: Option<mpsc::Receiver<StreamPayload>>,
    pending_acks: Arc<RwLock<HashMap<u64, PendingAck>>>,
    config: StreamConfig,
}

#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub buffer_size: usize,
    pub ack_timeout: Duration,
    pub max_retries: u32,
    pub ordered: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1000,
            ack_timeout: Duration::from_millis(100),
            max_retries: 3,
            ordered: true,
        }
    }
}

#[derive(Debug)]
struct PendingAck {
    sequence: u64,
    sent_at: Instant,
    retries: u32,
    payload: StreamPayload,
}

impl StreamChannel {
    pub fn new(stream_id: u64, job_id: Uuid, config: StreamConfig) -> Self {
        let (sender, receiver) = mpsc::channel(config.buffer_size);

        Self {
            stream_id,
            job_id,
            sender,
            receiver: Some(receiver),
            pending_acks: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub async fn send(&self, message: StreamingMessage, requires_ack: bool) -> QuantumResult<()> {
        let payload = StreamPayload {
            stream_id: self.stream_id,
            message,
            requires_ack,
        };

        self.sender.send(payload).await.map_err(|_| {
            QuantumRuntimeError::Execution(ExecutionError::AsyncError(
                "Stream channel closed".to_string(),
            ))
        })
    }

    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<StreamPayload>> {
        self.receiver.take()
    }
}

// =============================================================================
// 5. Hybrid Transport (Adaptive UDP/TCP)
// =============================================================================

pub struct HybridTransport {
    datagram_channel: DatagramChannel,
    stream_channel: StreamChannel,
    mode: Arc<RwLock<TransportMode>>,
    adaptation_config: AdaptationConfig,
    metrics: Arc<HybridMetrics>,
}

#[derive(Debug, Clone)]
pub struct AdaptationConfig {
    pub loss_threshold_to_stream: f64,
    pub loss_threshold_to_datagram: f64,
    pub latency_threshold_ms: u64,
    pub evaluation_window: Duration,
    pub min_samples: usize,
}

impl Default for AdaptationConfig {
    fn default() -> Self {
        Self {
            loss_threshold_to_stream: 0.05,
            loss_threshold_to_datagram: 0.01,
            latency_threshold_ms: 50,
            evaluation_window: Duration::from_secs(5),
            min_samples: 100,
        }
    }
}

#[derive(Debug)]
pub struct HybridMetrics {
    pub datagram_latency_us: AtomicU64,
    pub stream_latency_us: AtomicU64,
    pub current_loss_rate: RwLock<f64>,
    pub mode_switches: AtomicU64,
}

impl HybridMetrics {
    pub fn new() -> Self {
        Self {
            datagram_latency_us: AtomicU64::new(0),
            stream_latency_us: AtomicU64::new(0),
            current_loss_rate: RwLock::new(0.0),
            mode_switches: AtomicU64::new(0),
        }
    }
}

impl HybridTransport {
    pub fn new(
        job_id: Uuid,
        stream_id: u64,
        datagram_config: DatagramConfig,
        stream_config: StreamConfig,
        adaptation_config: AdaptationConfig,
    ) -> Self {
        Self {
            datagram_channel: DatagramChannel::new(job_id, datagram_config),
            stream_channel: StreamChannel::new(stream_id, job_id, stream_config),
            mode: Arc::new(RwLock::new(TransportMode::Hybrid)),
            adaptation_config,
            metrics: Arc::new(HybridMetrics::new()),
        }
    }

    pub async fn send_measurement(
        &self,
        event: &MeasurementEvent,
        priority: u8,
        critical: bool,
    ) -> QuantumResult<()> {
        let mode = *self.mode.read();

        match mode {
            TransportMode::Datagram => {
                self.datagram_channel.send_measurement(event, priority)?;
            }
            TransportMode::Stream => {
                let msg = StreamingMessage::MeasurementEvent(MeasurementEventPayload::from(
                    event.clone(),
                ));
                self.stream_channel.send(msg, critical).await?;
            }
            TransportMode::Hybrid => {
                if critical {
                    let msg = StreamingMessage::MeasurementEvent(MeasurementEventPayload::from(
                        event.clone(),
                    ));
                    self.stream_channel.send(msg, true).await?;
                }
                self.datagram_channel.send_measurement(event, priority)?;
            }
        }

        Ok(())
    }

    pub fn evaluate_and_adapt(&self) {
        let loss_rate = self.datagram_channel.stats().loss_rate();
        *self.metrics.current_loss_rate.write() = loss_rate;

        let current_mode = *self.mode.read();
        let new_mode = match current_mode {
            TransportMode::Datagram => {
                if loss_rate > self.adaptation_config.loss_threshold_to_stream {
                    TransportMode::Stream
                } else {
                    TransportMode::Datagram
                }
            }
            TransportMode::Stream => {
                if loss_rate < self.adaptation_config.loss_threshold_to_datagram {
                    TransportMode::Datagram
                } else {
                    TransportMode::Stream
                }
            }
            TransportMode::Hybrid => TransportMode::Hybrid,
        };

        if new_mode != current_mode {
            *self.mode.write() = new_mode;
            self.metrics.mode_switches.fetch_add(1, Ordering::SeqCst);
            tracing::info!("Transport mode switched: {:?} -> {:?}", current_mode, new_mode);
        }
    }

    pub fn current_mode(&self) -> TransportMode {
        *self.mode.read()
    }

    pub fn force_mode(&self, mode: TransportMode) {
        *self.mode.write() = mode;
    }

    pub fn metrics(&self) -> &HybridMetrics {
        &self.metrics
    }
}

// =============================================================================
// 6. OS-Level UDP Socket Integration
// =============================================================================

pub mod os_integration {
    use super::*;
    use std::io;

    #[derive(Debug, Clone)]
    pub struct UdpSocketConfig {
        pub recv_buffer_size: usize,
        pub send_buffer_size: usize,
        pub enable_broadcast: bool,
        pub enable_multicast: bool,
        pub multicast_ttl: u32,
        pub reuse_address: bool,
        pub nonblocking: bool,
    }

    impl Default for UdpSocketConfig {
        fn default() -> Self {
            Self {
                recv_buffer_size: 8 * 1024 * 1024,
                send_buffer_size: 8 * 1024 * 1024,
                enable_broadcast: false,
                enable_multicast: true,
                multicast_ttl: 1,
                reuse_address: true,
                nonblocking: true,
            }
        }
    }

    pub struct RawUdpTransport {
        socket: Option<tokio::net::UdpSocket>,
        local_addr: SocketAddr,
        config: UdpSocketConfig,
        stats: Arc<UdpStats>,
    }

    #[derive(Debug)]
    pub struct UdpStats {
        pub packets_sent: AtomicU64,
        pub packets_received: AtomicU64,
        pub bytes_sent: AtomicU64,
        pub bytes_received: AtomicU64,
        pub send_errors: AtomicU64,
        pub recv_errors: AtomicU64,
    }

    impl UdpStats {
        pub fn new() -> Self {
            Self {
                packets_sent: AtomicU64::new(0),
                packets_received: AtomicU64::new(0),
                bytes_sent: AtomicU64::new(0),
                bytes_received: AtomicU64::new(0),
                send_errors: AtomicU64::new(0),
                recv_errors: AtomicU64::new(0),
            }
        }
    }

    impl RawUdpTransport {
        pub async fn bind(addr: SocketAddr, config: UdpSocketConfig) -> io::Result<Self> {
            let socket = tokio::net::UdpSocket::bind(addr).await?;

            #[cfg(unix)]
            {
                use std::os::unix::io::AsRawFd;
                let fd = socket.as_raw_fd();
                unsafe {
                    let recv_buf = config.recv_buffer_size as libc::c_int;
                    let send_buf = config.send_buffer_size as libc::c_int;

                    libc::setsockopt(
                        fd,
                        libc::SOL_SOCKET,
                        libc::SO_RCVBUF,
                        &recv_buf as *const _ as *const libc::c_void,
                        std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                    );

                    libc::setsockopt(
                        fd,
                        libc::SOL_SOCKET,
                        libc::SO_SNDBUF,
                        &send_buf as *const _ as *const libc::c_void,
                        std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                    );
                }
            }

            #[cfg(windows)]
            {
                use std::os::windows::io::AsRawSocket;
                let sock = socket.as_raw_socket();
                unsafe {
                    let recv_buf = config.recv_buffer_size as i32;
                    let send_buf = config.send_buffer_size as i32;

                    windows_sys::Win32::Networking::WinSock::setsockopt(
                        sock as usize,
                        windows_sys::Win32::Networking::WinSock::SOL_SOCKET as i32,
                        windows_sys::Win32::Networking::WinSock::SO_RCVBUF as i32,
                        &recv_buf as *const _ as *const u8,
                        std::mem::size_of::<i32>() as i32,
                    );

                    windows_sys::Win32::Networking::WinSock::setsockopt(
                        sock as usize,
                        windows_sys::Win32::Networking::WinSock::SOL_SOCKET as i32,
                        windows_sys::Win32::Networking::WinSock::SO_SNDBUF as i32,
                        &send_buf as *const _ as *const u8,
                        std::mem::size_of::<i32>() as i32,
                    );
                }
            }

            let local_addr = socket.local_addr()?;

            Ok(Self {
                socket: Some(socket),
                local_addr,
                config,
                stats: Arc::new(UdpStats::new()),
            })
        }

        pub async fn send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
            if let Some(socket) = &self.socket {
                let sent = socket.send_to(buf, target).await?;
                self.stats.packets_sent.fetch_add(1, Ordering::Relaxed);
                self.stats.bytes_sent.fetch_add(sent as u64, Ordering::Relaxed);
                Ok(sent)
            } else {
                Err(io::Error::new(io::ErrorKind::NotConnected, "Socket not bound"))
            }
        }

        pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
            if let Some(socket) = &self.socket {
                let (len, addr) = socket.recv_from(buf).await?;
                self.stats.packets_received.fetch_add(1, Ordering::Relaxed);
                self.stats.bytes_received.fetch_add(len as u64, Ordering::Relaxed);
                Ok((len, addr))
            } else {
                Err(io::Error::new(io::ErrorKind::NotConnected, "Socket not bound"))
            }
        }

        pub fn local_addr(&self) -> SocketAddr {
            self.local_addr
        }

        pub fn stats(&self) -> &UdpStats {
            &self.stats
        }
    }

    pub struct MulticastGroup {
        group_addr: std::net::Ipv4Addr,
        interface: std::net::Ipv4Addr,
        joined: AtomicBool,
    }

    impl MulticastGroup {
        pub fn new(group_addr: std::net::Ipv4Addr) -> Self {
            Self {
                group_addr,
                interface: std::net::Ipv4Addr::UNSPECIFIED,
                joined: AtomicBool::new(false),
            }
        }

        pub fn with_interface(mut self, interface: std::net::Ipv4Addr) -> Self {
            self.interface = interface;
            self
        }

        pub fn join(&self, socket: &std::net::UdpSocket) -> io::Result<()> {
            socket.join_multicast_v4(&self.group_addr, &self.interface)?;
            self.joined.store(true, Ordering::SeqCst);
            Ok(())
        }

        pub fn leave(&self, socket: &std::net::UdpSocket) -> io::Result<()> {
            if self.joined.load(Ordering::SeqCst) {
                socket.leave_multicast_v4(&self.group_addr, &self.interface)?;
                self.joined.store(false, Ordering::SeqCst);
            }
            Ok(())
        }
    }
}

// =============================================================================
// 7. Connection Management
// =============================================================================

#[derive(Debug)]
pub struct TransportMetrics {
    pub connections_total: AtomicU64,
    pub datagrams_sent: AtomicU64,
    pub datagrams_received: AtomicU64,
    pub streams_opened: AtomicU64,
    pub streams_closed: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub avg_rtt_us: AtomicU64,
    pub packet_loss_rate: RwLock<f64>,
}

impl TransportMetrics {
    pub fn new() -> Self {
        Self {
            connections_total: AtomicU64::new(0),
            datagrams_sent: AtomicU64::new(0),
            datagrams_received: AtomicU64::new(0),
            streams_opened: AtomicU64::new(0),
            streams_closed: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            avg_rtt_us: AtomicU64::new(0),
            packet_loss_rate: RwLock::new(0.0),
        }
    }

    pub fn update_statistics(&self) {
        let sent = self.datagrams_sent.load(Ordering::SeqCst) as f64;
        let received = self.datagrams_received.load(Ordering::SeqCst) as f64;

        if sent > 0.0 {
            *self.packet_loss_rate.write() = 1.0 - (received / sent).min(1.0);
        }
    }

    pub fn throughput_mbps(&self, duration: Duration) -> f64 {
        let bytes = self.bytes_sent.load(Ordering::SeqCst) as f64;
        let secs = duration.as_secs_f64();
        if secs > 0.0 {
            (bytes * 8.0) / (secs * 1_000_000.0)
        } else {
            0.0
        }
    }
}

pub struct ConnectionPool {
    connections: Arc<RwLock<HashMap<Uuid, Arc<ConnectionState>>>>,
    max_connections: usize,
    idle_timeout: Duration,
}

impl ConnectionPool {
    pub fn new(max_connections: usize, idle_timeout: Duration) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            max_connections,
            idle_timeout,
        }
    }

    pub fn acquire(&self, client_id: Uuid) -> Option<Arc<ConnectionState>> {
        self.connections.read().get(&client_id).cloned()
    }

    pub fn release(&self, client_id: Uuid) {
        if let Some(conn) = self.connections.read().get(&client_id) {
            *conn.last_activity.write() = Instant::now();
        }
    }

    pub fn cleanup_idle(&self) {
        let now = Instant::now();
        let timeout = self.idle_timeout;

        self.connections.write().retain(|_, conn| {
            now.duration_since(*conn.last_activity.read()) < timeout
        });
    }

    pub fn active_count(&self) -> usize {
        self.connections.read().len()
    }
}

// =============================================================================
// Utility functions
// =============================================================================

fn current_timestamp_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datagram_stats() {
        let stats = DatagramStats::new();
        stats.sent.store(100, Ordering::SeqCst);
        stats.received.store(95, Ordering::SeqCst);

        assert!((stats.loss_rate() - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_transport_mode_adaptation() {
        let config = AdaptationConfig::default();
        assert_eq!(config.loss_threshold_to_stream, 0.05);
        assert_eq!(config.loss_threshold_to_datagram, 0.01);
    }

    #[test]
    fn test_connection_pool() {
        let pool = ConnectionPool::new(10, Duration::from_secs(30));
        assert_eq!(pool.active_count(), 0);
    }

    #[tokio::test]
    async fn test_quic_server_creation() {
        let config = WebTransportConfig::default();
        let server = QuicTransportServer::new(config);
        assert_eq!(server.connection_count(), 0);
    }
}
