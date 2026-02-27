# V-OS Architecture

## Table of Contents

1. [Overview](#overview)
2. [Microkernel Design](#microkernel-design)
3. [Memory Management](#memory-management)
4. [Process Model](#process-model)
5. [IPC Mechanisms](#ipc-mechanisms)
6. [Filesystem Architecture](#filesystem-architecture)
7. [Driver Model](#driver-model)
8. [Hypervisor Architecture](#hypervisor-architecture)
9. [Security Model](#security-model)
10. [Networking Stack](#networking-stack)

---

## Overview

V-OS is built on a **Rust microkernel architecture** forked from Redox OS. The design prioritizes:

- **Memory Safety**: Rust's ownership model eliminates entire classes of vulnerabilities
- **Modularity**: Userspace drivers and services for reliability and isolation
- **Security**: Capability-based access control, sandboxing, and minimal kernel attack surface
- **Performance**: Efficient IPC, SMP support, and hardware acceleration

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              User Applications                               │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  │
│  │ Desktop  │ │ Terminal │ │ Browser  │ │ Editor   │ │ Windows Guest VM │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ └────────┬─────────┘  │
├───────┴────────────┴────────────┴────────────┴────────────────┴─────────────┤
│                              System Services                                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  │
│  │ Orbital  │ │ Ion      │ │ Network  │ │ Audio    │ │ Hypervisor       │  │
│  │ (GUI)    │ │ (Shell)  │ │ Stack    │ │ Daemon   │ │ Service          │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ └────────┬─────────┘  │
├───────┴────────────┴────────────┴────────────┴────────────────┴─────────────┤
│                              Userspace Drivers                               │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  │
│  │ NVMe     │ │ AHCI     │ │ USB      │ │ Network  │ │ Graphics         │  │
│  │ Driver   │ │ Driver   │ │ Stack    │ │ Drivers  │ │ Drivers          │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ └────────┬─────────┘  │
├───────┴────────────┴────────────┴────────────┴────────────────┴─────────────┤
│                              Core Libraries                                  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  │
│  │ relibc   │ │ RedoxFS  │ │ Scheme   │ │ Syscall  │ │ Capability       │  │
│  │ (libc)   │ │          │ │ Handler  │ │ Wrapper  │ │ Manager          │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ └────────┬─────────┘  │
├───────┴────────────┴────────────┴────────────┴────────────────┴─────────────┤
│                              Microkernel (~30K LoC)                          │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  │
│  │ Memory   │ │ Process  │ │ IPC      │ │ Syscall  │ │ Interrupt        │  │
│  │ Manager  │ │ Scheduler│ │ (Scheme) │ │ Handler  │ │ Handler          │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ └────────┬─────────┘  │
├───────┴────────────┴────────────┴────────────┴────────────────┴─────────────┤
│                              Hardware Abstraction                            │
│  UEFI │ ACPI │ APIC │ PCIe │ IOMMU │ Timers │ Serial │ Framebuffer         │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Microkernel Design

### Kernel Responsibilities (Minimal)

The kernel handles only essential operations:

1. **Memory Management**: Physical/virtual memory allocation, paging
2. **Process Management**: Context switching, scheduling
3. **IPC**: Message passing via schemes
4. **Interrupt Handling**: Hardware interrupt dispatch
5. **Syscall Interface**: Minimal syscall set

### Kernel Size

- **Target**: ~30,000 lines of Rust code
- **Unsafe Blocks**: Minimized and audited
- **Dependencies**: Minimal external crates

### Boot Sequence

```
1. UEFI Firmware
   └─> 2. Bootloader (UEFI app)
       └─> 3. Kernel initialization
           ├─> Memory manager init
           ├─> Interrupt handlers
           ├─> Scheduler init
           └─> 4. Init process (userspace)
               ├─> Driver spawner
               ├─> Filesystem mount
               ├─> Network stack
               └─> Desktop/Shell
```

---

## Memory Management

### Virtual Memory Layout (x86_64)

```
0x0000_0000_0000_0000 ┌─────────────────────────┐
                      │ NULL guard page         │
0x0000_0000_0000_1000 ├─────────────────────────┤
                      │ User code/data          │
                      │ (grows up)              │
                      ├─────────────────────────┤
                      │ User heap               │
                      │ (grows up)              │
                      ├─────────────────────────┤
                      │ Memory-mapped regions   │
                      ├─────────────────────────┤
                      │ User stack              │
                      │ (grows down)            │
0x0000_7FFF_FFFF_FFFF ├─────────────────────────┤
                      │ Non-canonical hole      │
0xFFFF_8000_0000_0000 ├─────────────────────────┤
                      │ Kernel space            │
                      │ - Kernel code/data      │
                      │ - Kernel heap           │
                      │ - Physical memory map   │
                      │ - Per-CPU data          │
0xFFFF_FFFF_FFFF_FFFF └─────────────────────────┘
```

### Features

- **Paging**: 4-level page tables (PML4)
- **ASLR**: Kernel and userspace address randomization
- **Guard Pages**: Stack overflow protection
- **Copy-on-Write**: Efficient fork()
- **Demand Paging**: Lazy allocation

---

## Process Model

### Process States

```
         ┌─────────┐
         │ Created │
         └────┬────┘
              │ start()
              ▼
         ┌─────────┐
    ┌───>│ Ready   │<───┐
    │    └────┬────┘    │
    │         │ schedule│
    │         ▼         │
    │    ┌─────────┐    │
    │    │ Running │────┤ preempt/yield
    │    └────┬────┘    │
    │         │         │
    │    ┌────┴────┐    │
    │    │         │    │
    │    ▼         ▼    │
    │ ┌──────┐ ┌──────┐ │
    │ │Blocked│ │Zombie│ │
    │ └──┬───┘ └──────┘ │
    │    │ event        │
    └────┘              │
                        │
```

### Scheduling

- **Algorithm**: Preemptive priority-based with time slices
- **SMP Support**: Per-CPU run queues, load balancing
- **Real-time**: Priority levels for time-sensitive tasks

### Context Structure

```rust
pub struct Context {
    /// Kernel stack pointer
    pub kstack: Option<VirtualAddress>,
    /// User stack pointer
    pub ustack: Option<VirtualAddress>,
    /// Page table
    pub page_table: PageTable,
    /// CPU registers
    pub regs: Registers,
    /// Floating point state
    pub fpu: FpuState,
    /// Capability tokens
    pub capabilities: CapabilitySet,
}
```

---

## IPC Mechanisms

### Scheme-Based IPC

V-OS uses **schemes** for all IPC—a URL-like namespace for resources:

```
scheme://path/to/resource
```

Examples:
- `file:/home/user/document.txt` - Filesystem
- `tcp:192.168.1.1:80` - Network socket
- `display:` - Graphics output
- `hypervisor:vm0` - Virtual machine control

### Message Flow

```
┌──────────────┐                    ┌──────────────┐
│   Client     │                    │   Server     │
│   Process    │                    │   (Driver)   │
└──────┬───────┘                    └──────┬───────┘
       │                                   │
       │ open("scheme:/path")              │
       ├──────────────────────────────────>│
       │                                   │
       │ <handle>                          │
       │<──────────────────────────────────┤
       │                                   │
       │ read(handle, buf, len)            │
       ├──────────────────────────────────>│
       │                                   │
       │ <data>                            │
       │<──────────────────────────────────┤
       │                                   │
       │ close(handle)                     │
       ├──────────────────────────────────>│
       │                                   │
```

### Syscall Interface

Minimal syscall set (~50 syscalls):

| Category | Syscalls |
|----------|----------|
| Process | `clone`, `exit`, `waitpid`, `exec` |
| Memory | `mmap`, `munmap`, `mprotect` |
| IPC | `open`, `read`, `write`, `close`, `dup` |
| Sync | `futex`, `yield` |
| Time | `clock_gettime`, `nanosleep` |

---

## Filesystem Architecture

### RedoxFS

Native filesystem with:

- **Journaling**: Crash recovery
- **Copy-on-Write**: Data integrity
- **Encryption**: Optional disk encryption
- **Compression**: Transparent compression

### Virtual Filesystem Layer

```
┌─────────────────────────────────────────────────────────┐
│                    VFS Abstraction                       │
├─────────────────────────────────────────────────────────┤
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐ │
│  │ RedoxFS  │ │ FAT32    │ │ ext4     │ │ 9P/VirtIO  │ │
│  │          │ │          │ │ (compat) │ │ (guest)    │ │
│  └──────────┘ └──────────┘ └──────────┘ └────────────┘ │
├─────────────────────────────────────────────────────────┤
│                    Block Device Layer                    │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐ │
│  │ NVMe     │ │ AHCI     │ │ VirtIO   │ │ Ramdisk    │ │
│  └──────────┘ └──────────┘ └──────────┘ └────────────┘ │
└─────────────────────────────────────────────────────────┘
```

---

## Driver Model

### Userspace Drivers

All drivers run in userspace for:

- **Isolation**: Driver crash doesn't crash kernel
- **Security**: Limited privileges via capabilities
- **Debugging**: Standard debugging tools work

### Driver Structure

```rust
pub trait Driver {
    /// Initialize the driver
    fn init(&mut self) -> Result<(), DriverError>;
    
    /// Handle a scheme request
    fn handle(&mut self, request: &Request) -> Result<Response, DriverError>;
    
    /// Handle an interrupt
    fn interrupt(&mut self, irq: u8) -> Result<(), DriverError>;
    
    /// Cleanup on shutdown
    fn shutdown(&mut self) -> Result<(), DriverError>;
}
```

### PCI Device Discovery

```
1. ACPI tables parsed for PCI root
2. PCIe enumeration walks bus/device/function
3. Device IDs matched against driver database
4. Matching driver spawned with device access
```

---

## Hypervisor Architecture

### Design Goals

- **Type-1.5 Hypervisor**: Kernel-integrated but isolated
- **Hardware Virtualization**: VT-x (Intel), AMD-V (AMD)
- **Nested Paging**: EPT/NPT for memory virtualization
- **VirtIO**: Paravirtualized devices for performance

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Guest VMs                               │
│  ┌─────────────────┐  ┌─────────────────┐                   │
│  │ Windows Guest   │  │ Linux Guest     │                   │
│  │ ┌─────────────┐ │  │ ┌─────────────┐ │                   │
│  │ │ Guest Agent │ │  │ │ Guest Agent │ │                   │
│  │ └─────────────┘ │  │ └─────────────┘ │                   │
│  └────────┬────────┘  └────────┬────────┘                   │
├───────────┴────────────────────┴────────────────────────────┤
│                    Hypervisor Service                        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐  │
│  │ VM       │ │ vCPU     │ │ Memory   │ │ Device         │  │
│  │ Manager  │ │ Scheduler│ │ Manager  │ │ Emulation      │  │
│  └──────────┘ └──────────┘ └──────────┘ └────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                    Virtual Devices                           │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐  │
│  │ VirtIO   │ │ VirtIO   │ │ Virtual  │ │ Virtual        │  │
│  │ Block    │ │ Net      │ │ GPU      │ │ APIC           │  │
│  └──────────┘ └──────────┘ └──────────┘ └────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                    Kernel VMX/SVM Support                    │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐  │
│  │ VMCS/    │ │ EPT/NPT  │ │ VM-Exit  │ │ Interrupt      │  │
│  │ VMCB     │ │ Manager  │ │ Handlers │ │ Virtualization │  │
│  └──────────┘ └──────────┘ └──────────┘ └────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### VM Lifecycle

```
1. Create VM
   └─> Allocate VMCS/VMCB
   └─> Set up EPT/NPT
   └─> Initialize vCPUs

2. Load Guest
   └─> Map guest memory
   └─> Load guest kernel/OS
   └─> Set initial registers

3. Run Guest
   └─> VM-entry
   └─> Guest execution
   └─> VM-exit on events

4. Handle VM-Exits
   └─> I/O port access
   └─> MMIO access
   └─> Interrupts
   └─> Hypercalls

5. Shutdown
   └─> Stop vCPUs
   └─> Free resources
   └─> Cleanup
```

---

## Security Model

### Capability-Based Security

Every resource access requires a capability token:

```rust
pub struct Capability {
    /// Resource identifier
    pub resource: ResourceId,
    /// Permitted operations
    pub permissions: Permissions,
    /// Expiration (optional)
    pub expires: Option<Timestamp>,
}

pub struct Permissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub grant: bool,  // Can delegate to others
}
```

### Security Layers

```
┌─────────────────────────────────────────────────────────────┐
│ Layer 1: Memory Safety (Rust)                                │
│ - No buffer overflows, use-after-free, data races          │
├─────────────────────────────────────────────────────────────┤
│ Layer 2: Process Isolation                                   │
│ - Separate address spaces, capability requirements          │
├─────────────────────────────────────────────────────────────┤
│ Layer 3: Syscall Filtering                                   │
│ - Whitelist allowed syscalls per process                    │
├─────────────────────────────────────────────────────────────┤
│ Layer 4: Driver Isolation                                    │
│ - Userspace drivers, limited hardware access                │
├─────────────────────────────────────────────────────────────┤
│ Layer 5: VM Isolation                                        │
│ - Hardware-enforced guest boundaries, IOMMU                 │
├─────────────────────────────────────────────────────────────┤
│ Layer 6: Secure Boot                                         │
│ - Verified boot chain, signed kernel/drivers                │
└─────────────────────────────────────────────────────────────┘
```

---

## Networking Stack

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Applications                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐  │
│  │ Browser  │ │ SSH      │ │ HTTP     │ │ DNS Client     │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────────┬───────┘  │
├───────┴────────────┴────────────┴────────────────┴──────────┤
│                    Socket API                                │
├─────────────────────────────────────────────────────────────┤
│                    Transport Layer                           │
│  ┌──────────────────────┐ ┌──────────────────────────────┐  │
│  │ TCP                  │ │ UDP                          │  │
│  └──────────────────────┘ └──────────────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                    Network Layer                             │
│  ┌──────────────────────┐ ┌──────────────────────────────┐  │
│  │ IPv4                 │ │ IPv6                         │  │
│  └──────────────────────┘ └──────────────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                    Link Layer                                │
│  ┌──────────────────────┐ ┌──────────────────────────────┐  │
│  │ Ethernet             │ │ Virtual Bridge               │  │
│  └──────────────────────┘ └──────────────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                    NIC Drivers                               │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐                     │
│  │ e1000    │ │ VirtIO   │ │ RTL8139  │                     │
│  └──────────┘ └──────────┘ └──────────┘                     │
└─────────────────────────────────────────────────────────────┘
```

---

## Future Considerations

### Performance Optimizations
- Lock-free data structures
- Zero-copy I/O paths
- NUMA awareness
- CPU affinity

### Hardware Support
- GPU compute (Vulkan/OpenCL)
- TPM integration
- Secure enclaves (SGX/SEV)
- Thunderbolt/USB4

### Compatibility
- Linux syscall compatibility layer
- Wine integration for Windows apps
- Container runtime support

---

## References

- [Redox OS Book](https://doc.redox-os.org/book/)
- [Redox Kernel Source](https://gitlab.redox-os.org/redox-os/kernel)
- [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
- [AMD APM](https://www.amd.com/en/support/tech-docs)
- [VirtIO Specification](https://docs.oasis-open.org/virtio/virtio/v1.1/virtio-v1.1.html)
