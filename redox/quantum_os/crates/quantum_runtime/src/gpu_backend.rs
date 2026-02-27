// =============================================================================
// V-OS - GPU Backend Module
// =============================================================================
// Table of Contents:
//   1. GpuStateBackend Trait
//   2. WebGPU Backend Implementation
//   3. GPU Memory Management
//   4. Automatic Backend Selection
//   5. Kernel Definitions (WGSL shaders)
// =============================================================================
// Purpose: Provides GPU-accelerated quantum state simulation using WebGPU
//          for cross-platform compatibility. Enables significant speedup for
//          large quantum circuits on supported hardware.
// =============================================================================
// Feature: This module is only compiled when `gpu_backend` feature is enabled.
// =============================================================================

#![cfg(feature = "gpu_backend")]

use crate::circuit_program::QuantumCircuitStructure;
use crate::error::{BackendError, QuantumResult, QuantumRuntimeError};
use crate::gate_operations::QuantumGateInterface;
use crate::state_backend::QuantumStateVector;
use async_trait::async_trait;
use num_complex::Complex64;
use parking_lot::RwLock;
use std::sync::Arc;
use uuid::Uuid;

// =============================================================================
// 1. GpuStateBackend Trait
// =============================================================================

#[async_trait]
pub trait GpuStateBackend: Send + Sync {
    async fn initialize(&mut self) -> QuantumResult<()>;
    async fn upload_state(&mut self, state: &QuantumStateVector) -> QuantumResult<()>;
    async fn download_state(&self) -> QuantumResult<QuantumStateVector>;
    async fn apply_gate(&mut self, gate: &dyn QuantumGateInterface) -> QuantumResult<()>;
    async fn apply_circuit(&mut self, circuit: &QuantumCircuitStructure) -> QuantumResult<()>;
    fn backend_name(&self) -> &str;
    fn max_qubits(&self) -> usize;
    fn is_initialized(&self) -> bool;
    fn memory_usage_bytes(&self) -> usize;
}

// =============================================================================
// 2. WebGPU Backend Implementation
// =============================================================================

pub struct WebGpuBackend {
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    state_buffer: Option<wgpu::Buffer>,
    staging_buffer: Option<wgpu::Buffer>,
    number_of_quantum_bits: usize,
    is_initialized: bool,
    config: WebGpuConfig,
}

#[derive(Debug, Clone)]
pub struct WebGpuConfig {
    pub power_preference: GpuPowerPreference,
    pub max_buffer_size: usize,
    pub workgroup_size: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum GpuPowerPreference {
    LowPower,
    HighPerformance,
}

impl Default for WebGpuConfig {
    fn default() -> Self {
        Self {
            power_preference: GpuPowerPreference::HighPerformance,
            max_buffer_size: 1 << 30,
            workgroup_size: 256,
        }
    }
}

impl WebGpuBackend {
    pub fn new(number_of_quantum_bits: usize) -> Self {
        Self {
            device: None,
            queue: None,
            state_buffer: None,
            staging_buffer: None,
            number_of_quantum_bits,
            is_initialized: false,
            config: WebGpuConfig::default(),
        }
    }

    pub fn with_config(mut self, config: WebGpuConfig) -> Self {
        self.config = config;
        self
    }

    async fn create_device(&self) -> QuantumResult<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: match self.config.power_preference {
                    GpuPowerPreference::LowPower => wgpu::PowerPreference::LowPower,
                    GpuPowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
                },
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| {
                QuantumRuntimeError::Backend(BackendError::GpuInitFailed(
                    "No suitable GPU adapter found".to_string(),
                ))
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("quantum_gpu_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| {
                QuantumRuntimeError::Backend(BackendError::GpuInitFailed(e.to_string()))
            })?;

        Ok((device, queue))
    }

    fn create_state_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
        let dim = 1usize << self.number_of_quantum_bits;
        let buffer_size = (dim * 2 * std::mem::size_of::<f32>()) as u64;

        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quantum_state_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    fn create_staging_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
        let dim = 1usize << self.number_of_quantum_bits;
        let buffer_size = (dim * 2 * std::mem::size_of::<f32>()) as u64;

        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quantum_staging_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn state_to_f32_vec(&self, state: &QuantumStateVector) -> Vec<f32> {
        let mut data = Vec::with_capacity(state.dimension() * 2);
        for amp in state.amplitudes() {
            data.push(amp.re as f32);
            data.push(amp.im as f32);
        }
        data
    }

    fn f32_vec_to_state(&self, data: &[f32]) -> QuantumStateVector {
        let dim = data.len() / 2;
        let mut amplitudes = Vec::with_capacity(dim);

        for i in 0..dim {
            let re = data[i * 2] as f64;
            let im = data[i * 2 + 1] as f64;
            amplitudes.push(Complex64::new(re, im));
        }

        QuantumStateVector::from_amplitudes(amplitudes)
    }
}

#[async_trait]
impl GpuStateBackend for WebGpuBackend {
    async fn initialize(&mut self) -> QuantumResult<()> {
        let (device, queue) = self.create_device().await?;

        self.state_buffer = Some(self.create_state_buffer(&device));
        self.staging_buffer = Some(self.create_staging_buffer(&device));
        self.device = Some(device);
        self.queue = Some(queue);
        self.is_initialized = true;

        Ok(())
    }

    async fn upload_state(&mut self, state: &QuantumStateVector) -> QuantumResult<()> {
        if !self.is_initialized {
            return Err(QuantumRuntimeError::Backend(BackendError::NotAvailable(
                "GPU backend not initialized".to_string(),
            )));
        }

        let data = self.state_to_f32_vec(state);
        let bytes: &[u8] = bytemuck::cast_slice(&data);

        if let (Some(queue), Some(buffer)) = (&self.queue, &self.state_buffer) {
            queue.write_buffer(buffer, 0, bytes);
        }

        Ok(())
    }

    async fn download_state(&self) -> QuantumResult<QuantumStateVector> {
        if !self.is_initialized {
            return Err(QuantumRuntimeError::Backend(BackendError::NotAvailable(
                "GPU backend not initialized".to_string(),
            )));
        }

        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();
        let state_buffer = self.state_buffer.as_ref().unwrap();
        let staging_buffer = self.staging_buffer.as_ref().unwrap();

        let dim = 1usize << self.number_of_quantum_bits;
        let buffer_size = (dim * 2 * std::mem::size_of::<f32>()) as u64;

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("copy_encoder"),
        });

        encoder.copy_buffer_to_buffer(state_buffer, 0, staging_buffer, 0, buffer_size);
        queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = tokio::sync::oneshot::channel();

        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        device.poll(wgpu::Maintain::Wait);

        rx.await
            .map_err(|_| {
                QuantumRuntimeError::Backend(BackendError::GpuKernelFailed(
                    "Buffer mapping cancelled".to_string(),
                ))
            })?
            .map_err(|e| {
                QuantumRuntimeError::Backend(BackendError::GpuKernelFailed(e.to_string()))
            })?;

        let data: Vec<f32> = {
            let view = buffer_slice.get_mapped_range();
            bytemuck::cast_slice(&view).to_vec()
        };

        staging_buffer.unmap();

        Ok(self.f32_vec_to_state(&data))
    }

    async fn apply_gate(&mut self, gate: &dyn QuantumGateInterface) -> QuantumResult<()> {
        let mut state = self.download_state().await?;
        gate.apply_to_full_state_vector(&mut state);
        self.upload_state(&state).await?;
        Ok(())
    }

    async fn apply_circuit(&mut self, circuit: &QuantumCircuitStructure) -> QuantumResult<()> {
        for gate_instance in circuit.gate_application_instances() {
            self.apply_gate(gate_instance.quantum_gate_interface.as_ref())
                .await?;
        }
        Ok(())
    }

    fn backend_name(&self) -> &str {
        "webgpu"
    }

    fn max_qubits(&self) -> usize {
        let max_buffer = self.config.max_buffer_size;
        let bytes_per_amplitude = 2 * std::mem::size_of::<f32>();
        let max_dim = max_buffer / bytes_per_amplitude;
        (max_dim as f64).log2().floor() as usize
    }

    fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    fn memory_usage_bytes(&self) -> usize {
        if !self.is_initialized {
            return 0;
        }
        let dim = 1usize << self.number_of_quantum_bits;
        dim * 2 * std::mem::size_of::<f32>() * 2
    }
}

// =============================================================================
// 3. GPU Memory Management
// =============================================================================

pub struct GpuMemoryManager {
    allocated_buffers: RwLock<Vec<GpuBufferHandle>>,
    total_allocated: std::sync::atomic::AtomicUsize,
    max_memory: usize,
}

#[derive(Debug)]
pub struct GpuBufferHandle {
    id: Uuid,
    size_bytes: usize,
    buffer_type: GpuBufferType,
}

#[derive(Debug, Clone, Copy)]
pub enum GpuBufferType {
    StateVector,
    GateMatrix,
    Staging,
    Uniform,
}

impl GpuMemoryManager {
    pub fn new(max_memory: usize) -> Self {
        Self {
            allocated_buffers: RwLock::new(Vec::new()),
            total_allocated: std::sync::atomic::AtomicUsize::new(0),
            max_memory,
        }
    }

    pub fn can_allocate(&self, size_bytes: usize) -> bool {
        let current = self
            .total_allocated
            .load(std::sync::atomic::Ordering::SeqCst);
        current + size_bytes <= self.max_memory
    }

    pub fn allocate(&self, size_bytes: usize, buffer_type: GpuBufferType) -> QuantumResult<Uuid> {
        if !self.can_allocate(size_bytes) {
            return Err(QuantumRuntimeError::Backend(BackendError::GpuMemoryAlloc {
                bytes: size_bytes,
            }));
        }

        let handle = GpuBufferHandle {
            id: Uuid::new_v4(),
            size_bytes,
            buffer_type,
        };

        let id = handle.id;
        self.allocated_buffers.write().push(handle);
        self.total_allocated
            .fetch_add(size_bytes, std::sync::atomic::Ordering::SeqCst);

        Ok(id)
    }

    pub fn deallocate(&self, id: Uuid) -> bool {
        let mut buffers = self.allocated_buffers.write();
        if let Some(pos) = buffers.iter().position(|b| b.id == id) {
            let handle = buffers.remove(pos);
            self.total_allocated
                .fetch_sub(handle.size_bytes, std::sync::atomic::Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn total_allocated(&self) -> usize {
        self.total_allocated
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn available(&self) -> usize {
        self.max_memory.saturating_sub(self.total_allocated())
    }

    pub fn utilization(&self) -> f64 {
        self.total_allocated() as f64 / self.max_memory as f64
    }
}

// =============================================================================
// 4. Automatic Backend Selection
// =============================================================================

pub struct AutoBackendSelector {
    gpu_threshold_qubits: usize,
    prefer_gpu: bool,
    gpu_available: bool,
}

impl AutoBackendSelector {
    pub async fn new() -> Self {
        let gpu_available = Self::check_gpu_availability().await;

        Self {
            gpu_threshold_qubits: 10,
            prefer_gpu: true,
            gpu_available,
        }
    }

    async fn check_gpu_availability() -> bool {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .is_some()
    }

    pub fn with_threshold(mut self, qubits: usize) -> Self {
        self.gpu_threshold_qubits = qubits;
        self
    }

    pub fn prefer_gpu(mut self, prefer: bool) -> Self {
        self.prefer_gpu = prefer;
        self
    }

    pub fn should_use_gpu(&self, number_of_qubits: usize) -> bool {
        self.gpu_available
            && self.prefer_gpu
            && number_of_qubits >= self.gpu_threshold_qubits
    }

    pub fn gpu_available(&self) -> bool {
        self.gpu_available
    }

    pub fn select_backend(
        &self,
        number_of_qubits: usize,
    ) -> SelectedBackend {
        if self.should_use_gpu(number_of_qubits) {
            SelectedBackend::Gpu
        } else if number_of_qubits <= 12 {
            SelectedBackend::DenseVector
        } else {
            SelectedBackend::MatrixProductState
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedBackend {
    DenseVector,
    MatrixProductState,
    Gpu,
}

// =============================================================================
// 5. Kernel Definitions (WGSL shaders)
// =============================================================================

pub mod kernels {
    pub const HADAMARD_KERNEL: &str = r#"
        @group(0) @binding(0) var<storage, read_write> state: array<vec2<f32>>;
        @group(0) @binding(1) var<uniform> params: HadamardParams;

        struct HadamardParams {
            target_mask: u32,
            dimension: u32,
            inv_sqrt2: f32,
            _padding: f32,
        }

        @compute @workgroup_size(256)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
            let i = global_id.x;
            if (i >= params.dimension) { return; }

            let target_mask = params.target_mask;
            if ((i & target_mask) == 0u) {
                let j = i | target_mask;
                let a = state[i];
                let b = state[j];

                let inv_sqrt2 = params.inv_sqrt2;
                state[i] = (a + b) * inv_sqrt2;
                state[j] = (a - b) * inv_sqrt2;
            }
        }
    "#;

    pub const PAULI_X_KERNEL: &str = r#"
        @group(0) @binding(0) var<storage, read_write> state: array<vec2<f32>>;
        @group(0) @binding(1) var<uniform> params: PauliParams;

        struct PauliParams {
            target_mask: u32,
            dimension: u32,
            _padding: vec2<f32>,
        }

        @compute @workgroup_size(256)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
            let i = global_id.x;
            if (i >= params.dimension) { return; }

            let target_mask = params.target_mask;
            if ((i & target_mask) == 0u) {
                let j = i | target_mask;
                let temp = state[i];
                state[i] = state[j];
                state[j] = temp;
            }
        }
    "#;

    pub const ROTATION_Z_KERNEL: &str = r#"
        @group(0) @binding(0) var<storage, read_write> state: array<vec2<f32>>;
        @group(0) @binding(1) var<uniform> params: RotationParams;

        struct RotationParams {
            target_mask: u32,
            dimension: u32,
            cos_half: f32,
            sin_half: f32,
        }

        @compute @workgroup_size(256)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
            let i = global_id.x;
            if (i >= params.dimension) { return; }

            let target_mask = params.target_mask;
            let amp = state[i];

            if ((i & target_mask) == 0u) {
                // |0> state: multiply by e^(-i*theta/2)
                let cos_half = params.cos_half;
                let sin_half = -params.sin_half;
                state[i] = vec2<f32>(
                    amp.x * cos_half - amp.y * sin_half,
                    amp.x * sin_half + amp.y * cos_half
                );
            } else {
                // |1> state: multiply by e^(i*theta/2)
                let cos_half = params.cos_half;
                let sin_half = params.sin_half;
                state[i] = vec2<f32>(
                    amp.x * cos_half - amp.y * sin_half,
                    amp.x * sin_half + amp.y * cos_half
                );
            }
        }
    "#;

    pub const CNOT_KERNEL: &str = r#"
        @group(0) @binding(0) var<storage, read_write> state: array<vec2<f32>>;
        @group(0) @binding(1) var<uniform> params: CnotParams;

        struct CnotParams {
            control_mask: u32,
            target_mask: u32,
            dimension: u32,
            _padding: u32,
        }

        @compute @workgroup_size(256)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
            let i = global_id.x;
            if (i >= params.dimension) { return; }

            let control_mask = params.control_mask;
            let target_mask = params.target_mask;

            // Only swap if control is 1 and target is 0
            if ((i & control_mask) != 0u && (i & target_mask) == 0u) {
                let j = i | target_mask;
                let temp = state[i];
                state[i] = state[j];
                state[j] = temp;
            }
        }
    "#;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_memory_manager() {
        let manager = GpuMemoryManager::new(1024);

        let id1 = manager.allocate(256, GpuBufferType::StateVector).unwrap();
        assert_eq!(manager.total_allocated(), 256);

        let id2 = manager.allocate(256, GpuBufferType::Staging).unwrap();
        assert_eq!(manager.total_allocated(), 512);

        assert!(manager.deallocate(id1));
        assert_eq!(manager.total_allocated(), 256);

        assert!(!manager.can_allocate(1024));
    }

    #[test]
    fn test_backend_selection() {
        let selector = AutoBackendSelector {
            gpu_threshold_qubits: 10,
            prefer_gpu: false,
            gpu_available: true,
        };

        assert_eq!(selector.select_backend(5), SelectedBackend::DenseVector);
        assert_eq!(selector.select_backend(15), SelectedBackend::MatrixProductState);
    }
}
