# V-OS Driver Development Guide

## Table of Contents

1. [Overview](#overview)
2. [Driver Architecture](#driver-architecture)
3. [Creating a Driver](#creating-a-driver)
4. [Hardware Access](#hardware-access)
5. [Interrupt Handling](#interrupt-handling)
6. [DMA Operations](#dma-operations)
7. [Testing Drivers](#testing-drivers)
8. [Driver Examples](#driver-examples)

---

## Overview

V-OS uses a **userspace driver model** inherited from Redox OS. All drivers run as regular userspace processes, communicating with the kernel through schemes.

### Benefits

- **Isolation**: Driver crashes don't crash the kernel
- **Security**: Limited hardware access via capabilities
- **Debugging**: Standard debugging tools work
- **Development**: Faster iteration, no kernel rebuilds

### Driver Types

| Type | Description | Examples |
|------|-------------|----------|
| Block | Storage devices | NVMe, AHCI, VirtIO-block |
| Network | Network interfaces | e1000, VirtIO-net |
| Input | Human interface devices | USB HID, PS/2 |
| Graphics | Display adapters | Framebuffer, Intel, VirtIO-GPU |
| Audio | Sound devices | Intel HDA |

---

## Driver Architecture

### Scheme-Based Communication

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Driver Architecture                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐         ┌──────────────┐         ┌──────────────┐         │
│  │ Application  │         │ Application  │         │ Application  │         │
│  └──────┬───────┘         └──────┬───────┘         └──────┬───────┘         │
│         │                        │                        │                  │
│         │ open("disk:/0")        │ open("net:/eth0")      │                  │
│         ▼                        ▼                        ▼                  │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                         Scheme Router                                │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│         │                        │                        │                  │
│         ▼                        ▼                        ▼                  │
│  ┌──────────────┐         ┌──────────────┐         ┌──────────────┐         │
│  │ NVMe Driver  │         │ e1000 Driver │         │ USB Driver   │         │
│  │              │         │              │         │              │         │
│  │ Scheme:      │         │ Scheme:      │         │ Scheme:      │         │
│  │ "disk"       │         │ "net"        │         │ "usb"        │         │
│  └──────┬───────┘         └──────┬───────┘         └──────┬───────┘         │
│         │                        │                        │                  │
├─────────┴────────────────────────┴────────────────────────┴─────────────────┤
│                              Kernel                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ Hardware Access Layer (Capabilities, IOMMU, Interrupts)              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────────────────────────┤
│                              Hardware                                        │
│  ┌──────────────┐         ┌──────────────┐         ┌──────────────┐         │
│  │ NVMe SSD     │         │ Intel NIC    │         │ USB Host     │         │
│  └──────────────┘         └──────────────┘         └──────────────┘         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Driver Lifecycle

```rust
/// Driver lifecycle states
pub enum DriverState {
    /// Driver loaded but not initialized
    Loaded,
    /// Driver initialized and ready
    Ready,
    /// Driver actively handling requests
    Running,
    /// Driver suspended (power management)
    Suspended,
    /// Driver shutting down
    Stopping,
    /// Driver stopped
    Stopped,
}

/// Driver lifecycle trait
pub trait Driver {
    /// Initialize the driver
    fn init(&mut self) -> Result<(), DriverError>;
    
    /// Start handling requests
    fn start(&mut self) -> Result<(), DriverError>;
    
    /// Suspend the driver (power management)
    fn suspend(&mut self) -> Result<(), DriverError>;
    
    /// Resume from suspend
    fn resume(&mut self) -> Result<(), DriverError>;
    
    /// Stop the driver
    fn stop(&mut self) -> Result<(), DriverError>;
}
```

---

## Creating a Driver

### Project Structure

```
my-driver/
├── Cargo.toml
├── src/
│   ├── main.rs          # Entry point
│   ├── lib.rs           # Driver library
│   ├── device.rs        # Device abstraction
│   ├── scheme.rs        # Scheme handler
│   └── registers.rs     # Hardware registers
└── recipe.toml          # V-OS recipe
```

### Cargo.toml

```toml
[package]
name = "my-driver"
version = "0.1.0"
edition = "2021"

[dependencies]
# Redox system libraries
redox_syscall = "0.4"
libredox = "0.1"

# Hardware access
pcid_interface = { git = "https://gitlab.redox-os.org/redox-os/drivers.git" }
common = { git = "https://gitlab.redox-os.org/redox-os/drivers.git" }

# Utilities
log = "0.4"
bitflags = "2.0"
```

### Main Entry Point

```rust
//! My Driver - Example V-OS driver
//! 
//! # Table of Contents
//! 1. Initialization
//! 2. Scheme Handler
//! 3. Device Operations
//! 4. Interrupt Handling

use std::fs::File;
use std::io::{Read, Write};
use libredox::flag;
use redox_syscall::SchemeMut;

mod device;
mod scheme;
mod registers;

use device::MyDevice;
use scheme::MyScheme;

fn main() {
    // Initialize logging
    env_logger::init();
    log::info!("My Driver starting...");
    
    // Parse PCI device info from environment
    let pci_config = pcid_interface::PcidServerHandle::connect_default()
        .expect("Failed to connect to pcid");
    
    // Initialize device
    let device = MyDevice::new(&pci_config)
        .expect("Failed to initialize device");
    
    // Create scheme
    let mut scheme = MyScheme::new(device);
    
    // Register scheme with kernel
    let socket = File::create(":mydevice")
        .expect("Failed to create scheme");
    
    // Event loop
    let mut event_queue = File::open("event:")
        .expect("Failed to open event queue");
    
    loop {
        // Wait for events
        let mut events = [0u8; 16];
        if event_queue.read(&mut events).is_err() {
            break;
        }
        
        // Handle scheme requests
        let mut buf = [0u8; 4096];
        while let Ok(count) = socket.read(&mut buf) {
            if count == 0 {
                break;
            }
            
            // Process request
            let response = scheme.handle(&buf[..count]);
            socket.write_all(&response).ok();
        }
    }
    
    log::info!("My Driver stopped");
}
```

### Scheme Handler

```rust
//! Scheme handler for device access

use redox_syscall::{SchemeMut, Error, ENOENT, EINVAL};
use std::collections::BTreeMap;

pub struct MyScheme {
    device: MyDevice,
    handles: BTreeMap<usize, Handle>,
    next_id: usize,
}

struct Handle {
    path: String,
    flags: usize,
    offset: usize,
}

impl SchemeMut for MyScheme {
    fn open(&mut self, path: &str, flags: usize, _uid: u32, _gid: u32) -> Result<usize, Error> {
        // Validate path
        if !self.device.is_valid_path(path) {
            return Err(Error::new(ENOENT));
        }
        
        // Create handle
        let id = self.next_id;
        self.next_id += 1;
        
        self.handles.insert(id, Handle {
            path: path.to_string(),
            flags,
            offset: 0,
        });
        
        Ok(id)
    }
    
    fn read(&mut self, id: usize, buf: &mut [u8]) -> Result<usize, Error> {
        let handle = self.handles.get_mut(&id).ok_or(Error::new(ENOENT))?;
        
        // Read from device
        let count = self.device.read(handle.offset, buf)?;
        handle.offset += count;
        
        Ok(count)
    }
    
    fn write(&mut self, id: usize, buf: &[u8]) -> Result<usize, Error> {
        let handle = self.handles.get_mut(&id).ok_or(Error::new(ENOENT))?;
        
        // Write to device
        let count = self.device.write(handle.offset, buf)?;
        handle.offset += count;
        
        Ok(count)
    }
    
    fn close(&mut self, id: usize) -> Result<usize, Error> {
        self.handles.remove(&id).ok_or(Error::new(ENOENT))?;
        Ok(0)
    }
    
    fn seek(&mut self, id: usize, pos: isize, whence: usize) -> Result<isize, Error> {
        let handle = self.handles.get_mut(&id).ok_or(Error::new(ENOENT))?;
        
        let new_offset = match whence {
            0 => pos as usize,                              // SEEK_SET
            1 => (handle.offset as isize + pos) as usize,   // SEEK_CUR
            2 => (self.device.size() as isize + pos) as usize, // SEEK_END
            _ => return Err(Error::new(EINVAL)),
        };
        
        handle.offset = new_offset;
        Ok(new_offset as isize)
    }
}
```

---

## Hardware Access

### MMIO Access

```rust
//! Memory-mapped I/O access

use std::ptr::{read_volatile, write_volatile};

/// MMIO region wrapper
pub struct MmioRegion {
    base: *mut u8,
    size: usize,
}

impl MmioRegion {
    /// Create from physical address (requires capability)
    pub unsafe fn new(phys_addr: usize, size: usize) -> Result<Self, Error> {
        // Map physical memory
        let base = libredox::call::physmap(phys_addr, size, libredox::flag::PROT_READ | libredox::flag::PROT_WRITE)?;
        
        Ok(Self {
            base: base as *mut u8,
            size,
        })
    }
    
    /// Read 32-bit register
    pub fn read32(&self, offset: usize) -> u32 {
        assert!(offset + 4 <= self.size);
        unsafe {
            read_volatile((self.base.add(offset)) as *const u32)
        }
    }
    
    /// Write 32-bit register
    pub fn write32(&mut self, offset: usize, value: u32) {
        assert!(offset + 4 <= self.size);
        unsafe {
            write_volatile((self.base.add(offset)) as *mut u32, value)
        }
    }
    
    /// Read 64-bit register
    pub fn read64(&self, offset: usize) -> u64 {
        assert!(offset + 8 <= self.size);
        unsafe {
            read_volatile((self.base.add(offset)) as *const u64)
        }
    }
    
    /// Write 64-bit register
    pub fn write64(&mut self, offset: usize, value: u64) {
        assert!(offset + 8 <= self.size);
        unsafe {
            write_volatile((self.base.add(offset)) as *mut u64, value)
        }
    }
}

impl Drop for MmioRegion {
    fn drop(&mut self) {
        unsafe {
            libredox::call::physunmap(self.base as usize, self.size).ok();
        }
    }
}
```

### PCI Configuration

```rust
//! PCI configuration space access

use pcid_interface::{PcidServerHandle, PciBar, PciFunction};

pub struct PciDevice {
    handle: PcidServerHandle,
    function: PciFunction,
    bars: Vec<PciBar>,
}

impl PciDevice {
    pub fn new(handle: PcidServerHandle) -> Result<Self, Error> {
        let function = handle.fetch_function()?;
        let bars = handle.fetch_bars()?;
        
        Ok(Self {
            handle,
            function,
            bars,
        })
    }
    
    /// Get BAR address
    pub fn bar(&self, index: usize) -> Option<&PciBar> {
        self.bars.get(index)
    }
    
    /// Read config space
    pub fn config_read(&self, offset: u8) -> u32 {
        self.handle.config_read(offset)
    }
    
    /// Write config space
    pub fn config_write(&mut self, offset: u8, value: u32) {
        self.handle.config_write(offset, value)
    }
    
    /// Enable bus mastering (for DMA)
    pub fn enable_bus_master(&mut self) {
        let command = self.config_read(0x04);
        self.config_write(0x04, command | 0x04);
    }
    
    /// Enable memory space access
    pub fn enable_memory_space(&mut self) {
        let command = self.config_read(0x04);
        self.config_write(0x04, command | 0x02);
    }
}
```

---

## Interrupt Handling

### IRQ Registration

```rust
//! Interrupt handling

use std::fs::File;
use std::io::Read;

pub struct IrqHandler {
    irq_file: File,
    irq_number: u8,
}

impl IrqHandler {
    /// Register for IRQ notifications
    pub fn new(irq: u8) -> Result<Self, Error> {
        let irq_file = File::open(format!("irq:{}", irq))?;
        
        Ok(Self {
            irq_file,
            irq_number: irq,
        })
    }
    
    /// Wait for interrupt
    pub fn wait(&mut self) -> Result<(), Error> {
        let mut buf = [0u8; 8];
        self.irq_file.read(&mut buf)?;
        Ok(())
    }
    
    /// Acknowledge interrupt
    pub fn ack(&mut self) -> Result<(), Error> {
        // Write to IRQ file to acknowledge
        use std::io::Write;
        self.irq_file.write_all(&[0])?;
        Ok(())
    }
}

/// Interrupt handler thread
pub fn irq_thread(irq: u8, device: Arc<Mutex<MyDevice>>) {
    let mut handler = IrqHandler::new(irq).expect("Failed to register IRQ");
    
    loop {
        // Wait for interrupt
        if handler.wait().is_err() {
            break;
        }
        
        // Handle interrupt
        {
            let mut dev = device.lock().unwrap();
            dev.handle_interrupt();
        }
        
        // Acknowledge
        handler.ack().ok();
    }
}
```

### MSI/MSI-X

```rust
//! MSI/MSI-X interrupt support

pub struct MsiHandler {
    vector: u16,
    address: u64,
    data: u32,
}

impl MsiHandler {
    /// Configure MSI for device
    pub fn configure_msi(pci: &mut PciDevice, vector: u16) -> Result<Self, Error> {
        // Find MSI capability
        let msi_cap = pci.find_capability(0x05)?;
        
        // Configure MSI address and data
        let address = 0xFEE00000u64; // Local APIC address
        let data = vector as u32;
        
        // Write to MSI capability
        pci.config_write(msi_cap + 4, address as u32);
        pci.config_write(msi_cap + 8, (address >> 32) as u32);
        pci.config_write(msi_cap + 12, data);
        
        // Enable MSI
        let control = pci.config_read(msi_cap);
        pci.config_write(msi_cap, control | 0x10000);
        
        Ok(Self {
            vector,
            address,
            data,
        })
    }
}
```

---

## DMA Operations

### DMA Buffer Allocation

```rust
//! DMA buffer management

use std::alloc::{alloc, dealloc, Layout};

/// DMA-capable buffer
pub struct DmaBuffer {
    /// Virtual address
    virt: *mut u8,
    /// Physical address
    phys: usize,
    /// Buffer size
    size: usize,
    /// Alignment
    align: usize,
}

impl DmaBuffer {
    /// Allocate a DMA buffer
    pub fn new(size: usize) -> Result<Self, Error> {
        // Allocate page-aligned memory
        let align = 4096;
        let layout = Layout::from_size_align(size, align)
            .map_err(|_| Error::new(ENOMEM))?;
        
        let virt = unsafe { alloc(layout) };
        if virt.is_null() {
            return Err(Error::new(ENOMEM));
        }
        
        // Get physical address
        let phys = libredox::call::virttophys(virt as usize)?;
        
        Ok(Self {
            virt,
            phys,
            size,
            align,
        })
    }
    
    /// Get virtual address
    pub fn virt(&self) -> *mut u8 {
        self.virt
    }
    
    /// Get physical address (for hardware)
    pub fn phys(&self) -> usize {
        self.phys
    }
    
    /// Get buffer size
    pub fn size(&self) -> usize {
        self.size
    }
    
    /// Get as slice
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.virt, self.size) }
    }
    
    /// Get as mutable slice
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.virt, self.size) }
    }
}

impl Drop for DmaBuffer {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.size, self.align).unwrap();
        unsafe { dealloc(self.virt, layout) };
    }
}
```

### Scatter-Gather Lists

```rust
//! Scatter-gather DMA support

/// Scatter-gather entry
#[repr(C)]
pub struct SgEntry {
    pub addr: u64,
    pub len: u32,
    pub flags: u32,
}

/// Scatter-gather list
pub struct SgList {
    entries: Vec<SgEntry>,
    buffers: Vec<DmaBuffer>,
}

impl SgList {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            buffers: Vec::new(),
        }
    }
    
    /// Add a buffer to the scatter-gather list
    pub fn add(&mut self, buffer: DmaBuffer) {
        self.entries.push(SgEntry {
            addr: buffer.phys() as u64,
            len: buffer.size() as u32,
            flags: 0,
        });
        self.buffers.push(buffer);
    }
    
    /// Get entries for hardware
    pub fn entries(&self) -> &[SgEntry] {
        &self.entries
    }
}
```

---

## Testing Drivers

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_register_read_write() {
        // Create mock MMIO region
        let mut mock = MockMmioRegion::new(0x1000);
        
        // Write and read back
        mock.write32(0x00, 0xDEADBEEF);
        assert_eq!(mock.read32(0x00), 0xDEADBEEF);
    }
    
    #[test]
    fn test_dma_buffer_allocation() {
        let buffer = DmaBuffer::new(4096).unwrap();
        
        assert!(!buffer.virt().is_null());
        assert!(buffer.phys() != 0);
        assert_eq!(buffer.size(), 4096);
    }
}
```

### Integration Tests

```rust
//! Integration tests for driver

#[test]
fn test_device_initialization() {
    // This test requires actual hardware or QEMU
    if !cfg!(feature = "integration-tests") {
        return;
    }
    
    let device = MyDevice::probe().expect("Device not found");
    device.init().expect("Initialization failed");
    
    assert!(device.is_ready());
}

#[test]
fn test_read_write_cycle() {
    if !cfg!(feature = "integration-tests") {
        return;
    }
    
    let mut device = MyDevice::probe().unwrap();
    device.init().unwrap();
    
    // Write test data
    let write_data = [0x55u8; 512];
    device.write(0, &write_data).unwrap();
    
    // Read back
    let mut read_data = [0u8; 512];
    device.read(0, &mut read_data).unwrap();
    
    assert_eq!(write_data, read_data);
}
```

### Fuzzing

```rust
//! Fuzz testing for driver

#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz the scheme handler
    let mut scheme = MyScheme::new_mock();
    
    // Try to parse as scheme request
    if let Ok(request) = parse_request(data) {
        let _ = scheme.handle(&request);
    }
});
```

---

## Driver Examples

### Simple Block Driver

```rust
//! Simple block device driver example

use redox_syscall::SchemeMut;

pub struct BlockDriver {
    /// Device storage
    storage: Vec<u8>,
    /// Block size
    block_size: usize,
}

impl BlockDriver {
    pub fn new(size: usize, block_size: usize) -> Self {
        Self {
            storage: vec![0; size],
            block_size,
        }
    }
    
    /// Read blocks
    pub fn read_blocks(&self, start: u64, count: u64, buf: &mut [u8]) -> Result<usize, Error> {
        let offset = (start as usize) * self.block_size;
        let len = (count as usize) * self.block_size;
        
        if offset + len > self.storage.len() {
            return Err(Error::new(EIO));
        }
        
        buf[..len].copy_from_slice(&self.storage[offset..offset + len]);
        Ok(len)
    }
    
    /// Write blocks
    pub fn write_blocks(&mut self, start: u64, count: u64, buf: &[u8]) -> Result<usize, Error> {
        let offset = (start as usize) * self.block_size;
        let len = (count as usize) * self.block_size;
        
        if offset + len > self.storage.len() {
            return Err(Error::new(EIO));
        }
        
        self.storage[offset..offset + len].copy_from_slice(&buf[..len]);
        Ok(len)
    }
}
```

### Network Driver Skeleton

```rust
//! Network driver skeleton

pub struct NetworkDriver {
    mac_address: [u8; 6],
    mmio: MmioRegion,
    rx_ring: RxRing,
    tx_ring: TxRing,
}

impl NetworkDriver {
    pub fn new(pci: &PciDevice) -> Result<Self, Error> {
        // Map BAR0
        let bar = pci.bar(0).ok_or(Error::new(ENODEV))?;
        let mmio = unsafe { MmioRegion::new(bar.address, bar.size)? };
        
        // Read MAC address from EEPROM
        let mac_address = Self::read_mac_address(&mmio);
        
        // Initialize rings
        let rx_ring = RxRing::new(256)?;
        let tx_ring = TxRing::new(256)?;
        
        Ok(Self {
            mac_address,
            mmio,
            rx_ring,
            tx_ring,
        })
    }
    
    /// Transmit a packet
    pub fn transmit(&mut self, packet: &[u8]) -> Result<(), Error> {
        // Get next TX descriptor
        let desc = self.tx_ring.next_descriptor()?;
        
        // Copy packet to DMA buffer
        desc.buffer.as_mut_slice()[..packet.len()].copy_from_slice(packet);
        desc.length = packet.len() as u16;
        
        // Submit to hardware
        self.tx_ring.submit(desc);
        self.mmio.write32(TX_TAIL, self.tx_ring.tail() as u32);
        
        Ok(())
    }
    
    /// Receive a packet
    pub fn receive(&mut self) -> Option<Vec<u8>> {
        // Check for received packets
        if let Some(desc) = self.rx_ring.pop_completed() {
            let packet = desc.buffer.as_slice()[..desc.length as usize].to_vec();
            self.rx_ring.recycle(desc);
            Some(packet)
        } else {
            None
        }
    }
}
```

---

## Recipe Template

```toml
# recipe.toml for V-OS driver

[source]
git = "https://github.com/v-os-os/my-driver.git"
branch = "main"

[build]
template = "cargo"

[package]
name = "my-driver"
version = "0.1.0"
description = "My V-OS driver"
authors = ["Your Name"]
license = "MIT"

dependencies = [
    "relibc",
]

[package.files]
"/usr/bin/my-driver" = "target/release/my-driver"
```

---

## References

- [Redox OS Drivers](https://gitlab.redox-os.org/redox-os/drivers)
- [PCI Local Bus Specification](https://pcisig.com/)
- [NVMe Specification](https://nvmexpress.org/)
- [VirtIO Specification](https://docs.oasis-open.org/virtio/virtio/v1.1/virtio-v1.1.html)
