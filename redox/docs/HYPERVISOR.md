# V-OS Hypervisor Design Document

## Table of Contents

1. [Overview](#overview)
2. [Hardware Requirements](#hardware-requirements)
3. [Intel VT-x Implementation](#intel-vt-x-implementation)
4. [AMD-V Implementation](#amd-v-implementation)
5. [Memory Virtualization](#memory-virtualization)
6. [Virtual Devices](#virtual-devices)
7. [Guest Management](#guest-management)
8. [Windows Integration](#windows-integration)
9. [Security Considerations](#security-considerations)
10. [API Reference](#api-reference)

---

## Overview

The V-OS Hypervisor is a Type-1.5 hypervisor integrated into the OS, providing:

- **Native Performance**: Hardware-assisted virtualization (VT-x/AMD-V)
- **Seamless Integration**: Windows/Linux guests appear as native apps
- **Security**: Strong isolation with IOMMU and nested paging
- **Efficiency**: VirtIO paravirtualization for I/O

### Architecture Goals

```
┌─────────────────────────────────────────────────────────────┐
│                    Design Principles                         │
├─────────────────────────────────────────────────────────────┤
│ 1. Minimal TCB      - Small, auditable codebase             │
│ 2. Memory Safety    - Rust implementation                   │
│ 3. Hardware Accel   - VT-x/AMD-V, EPT/NPT, IOMMU           │
│ 4. Paravirt I/O     - VirtIO for performance               │
│ 5. Seamless UX      - Guests integrate with host desktop   │
└─────────────────────────────────────────────────────────────┘
```

---

## Hardware Requirements

### Intel Platforms

| Feature | CPUID | Required |
|---------|-------|----------|
| VMX | CPUID.1:ECX.VMX[bit 5] | Yes |
| EPT | IA32_VMX_PROCBASED_CTLS2[bit 33] | Yes |
| VPID | IA32_VMX_PROCBASED_CTLS2[bit 37] | Recommended |
| Unrestricted Guest | IA32_VMX_PROCBASED_CTLS2[bit 39] | Recommended |

### AMD Platforms

| Feature | CPUID | Required |
|---------|-------|----------|
| SVM | CPUID.80000001H:ECX.SVM[bit 2] | Yes |
| NPT | CPUID.8000000AH:EDX.NP[bit 0] | Yes |
| NRIP Save | CPUID.8000000AH:EDX.NRIPS[bit 3] | Recommended |
| Decode Assists | CPUID.8000000AH:EDX.DecodeAssists[bit 7] | Recommended |

### IOMMU (Optional but Recommended)

- Intel VT-d
- AMD-Vi

---

## Intel VT-x Implementation

### VMX Lifecycle

```rust
/// VMX operation states
pub enum VmxState {
    /// VMX not enabled
    Disabled,
    /// VMX enabled, in root operation
    RootOperation,
    /// VMX enabled, in non-root operation (guest running)
    NonRootOperation,
}

/// Enable VMX operation
pub fn vmx_enable() -> Result<(), VmxError> {
    // 1. Check CPUID for VMX support
    // 2. Enable VMX in CR4
    // 3. Allocate VMXON region (4KB aligned)
    // 4. Execute VMXON instruction
}

/// Disable VMX operation
pub fn vmx_disable() -> Result<(), VmxError> {
    // 1. Execute VMXOFF instruction
    // 2. Clear VMX enable bit in CR4
}
```

### VMCS Structure

```rust
/// Virtual Machine Control Structure
pub struct Vmcs {
    /// Guest state area
    pub guest_state: GuestState,
    /// Host state area
    pub host_state: HostState,
    /// VM-execution control fields
    pub exec_controls: ExecutionControls,
    /// VM-exit control fields
    pub exit_controls: ExitControls,
    /// VM-entry control fields
    pub entry_controls: EntryControls,
    /// VM-exit information fields
    pub exit_info: ExitInfo,
}

/// Guest state fields
pub struct GuestState {
    // Control registers
    pub cr0: u64,
    pub cr3: u64,
    pub cr4: u64,
    
    // Segment registers
    pub cs: SegmentRegister,
    pub ss: SegmentRegister,
    pub ds: SegmentRegister,
    pub es: SegmentRegister,
    pub fs: SegmentRegister,
    pub gs: SegmentRegister,
    
    // Descriptor tables
    pub gdtr: DescriptorTable,
    pub idtr: DescriptorTable,
    
    // General purpose registers
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    
    // Instruction pointer
    pub rip: u64,
    pub rflags: u64,
}
```

### VM-Exit Handling

```rust
/// VM-exit reasons
pub enum VmExitReason {
    ExceptionOrNmi = 0,
    ExternalInterrupt = 1,
    TripleFault = 2,
    InitSignal = 3,
    Sipi = 4,
    IoSmi = 5,
    OtherSmi = 6,
    InterruptWindow = 7,
    NmiWindow = 8,
    TaskSwitch = 9,
    Cpuid = 10,
    Getsec = 11,
    Hlt = 12,
    Invd = 13,
    Invlpg = 14,
    Rdpmc = 15,
    Rdtsc = 16,
    Rsm = 17,
    Vmcall = 18,
    Vmclear = 19,
    Vmlaunch = 20,
    Vmptrld = 21,
    Vmptrst = 22,
    Vmread = 23,
    Vmresume = 24,
    Vmwrite = 25,
    Vmxoff = 26,
    Vmxon = 27,
    ControlRegisterAccess = 28,
    MovDr = 29,
    IoInstruction = 30,
    Rdmsr = 31,
    Wrmsr = 32,
    EntryFailInvalidGuestState = 33,
    EntryFailMsrLoading = 34,
    Mwait = 36,
    MonitorTrapFlag = 37,
    Monitor = 39,
    Pause = 40,
    EntryFailMachineCheckEvent = 41,
    TprBelowThreshold = 43,
    ApicAccess = 44,
    VirtualizedEoi = 45,
    AccessGdtrOrIdtr = 46,
    AccessLdtrOrTr = 47,
    EptViolation = 48,
    EptMisconfiguration = 49,
    Invept = 50,
    Rdtscp = 51,
    VmxPreemptionTimerExpired = 52,
    Invvpid = 53,
    Wbinvd = 54,
    Xsetbv = 55,
    ApicWrite = 56,
    Rdrand = 57,
    Invpcid = 58,
    Vmfunc = 59,
    Encls = 60,
    Rdseed = 61,
    PageModificationLogFull = 62,
    Xsaves = 63,
    Xrstors = 64,
}

/// Handle VM-exit
pub fn handle_vm_exit(vcpu: &mut VCpu) -> Result<VmAction, VmError> {
    let reason = vcpu.vmcs.exit_info.reason;
    
    match reason {
        VmExitReason::Cpuid => handle_cpuid(vcpu),
        VmExitReason::IoInstruction => handle_io(vcpu),
        VmExitReason::EptViolation => handle_ept_violation(vcpu),
        VmExitReason::Vmcall => handle_hypercall(vcpu),
        VmExitReason::Hlt => handle_halt(vcpu),
        VmExitReason::ExternalInterrupt => handle_interrupt(vcpu),
        _ => Err(VmError::UnhandledExit(reason)),
    }
}
```

---

## AMD-V Implementation

### SVM Lifecycle

```rust
/// Enable SVM
pub fn svm_enable() -> Result<(), SvmError> {
    // 1. Check CPUID for SVM support
    // 2. Check MSR_VM_CR for SVM lock
    // 3. Enable SVM in EFER MSR
    // 4. Allocate host save area
}

/// VMCB structure
pub struct Vmcb {
    /// Control area (offset 0x000)
    pub control: VmcbControl,
    /// State save area (offset 0x400)
    pub state: VmcbState,
}

pub struct VmcbControl {
    pub intercept_cr: u32,
    pub intercept_dr: u32,
    pub intercept_exceptions: u32,
    pub intercept_misc1: u32,
    pub intercept_misc2: u32,
    pub pause_filter_threshold: u16,
    pub pause_filter_count: u16,
    pub iopm_base_pa: u64,
    pub msrpm_base_pa: u64,
    pub tsc_offset: u64,
    pub guest_asid: u32,
    pub tlb_control: u8,
    pub virtual_intr: VirtualInterrupt,
    pub interrupt_shadow: u8,
    pub exit_code: u64,
    pub exit_info1: u64,
    pub exit_info2: u64,
    pub exit_int_info: u64,
    pub np_enable: u64,
    pub avic_apic_bar: u64,
    pub ghcb_gpa: u64,
    pub event_inject: u64,
    pub n_cr3: u64,
    pub lbr_virtualization_enable: u64,
    pub vmcb_clean: u32,
    pub next_rip: u64,
    pub bytes_fetched: u8,
    pub guest_instruction_bytes: [u8; 15],
}
```

---

## Memory Virtualization

### Extended Page Tables (EPT) - Intel

```rust
/// EPT PML4 entry
pub struct EptPml4Entry {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub accessed: bool,
    pub pdpt_address: PhysicalAddress,
}

/// EPT page table structure
pub struct EptPageTable {
    pub pml4: [EptPml4Entry; 512],
}

/// Map guest physical to host physical
pub fn ept_map(
    ept: &mut EptPageTable,
    guest_phys: GuestPhysicalAddress,
    host_phys: HostPhysicalAddress,
    size: PageSize,
    permissions: EptPermissions,
) -> Result<(), EptError> {
    // Walk EPT and create mapping
}
```

### Nested Page Tables (NPT) - AMD

```rust
/// NPT configuration
pub struct NptConfig {
    pub n_cr3: PhysicalAddress,
    pub guest_pat: u64,
}

/// NPT page table (same format as regular page tables)
pub type NptPageTable = PageTable;
```

---

## Virtual Devices

### VirtIO Block Device

```rust
/// VirtIO block device
pub struct VirtioBlock {
    pub common: VirtioCommon,
    pub config: VirtioBlockConfig,
    pub queues: [Virtqueue; 1],
    pub backend: Box<dyn BlockBackend>,
}

pub struct VirtioBlockConfig {
    pub capacity: u64,
    pub size_max: u32,
    pub seg_max: u32,
    pub geometry: VirtioBlockGeometry,
    pub blk_size: u32,
}

impl VirtioDevice for VirtioBlock {
    fn handle_queue(&mut self, queue_idx: u16) -> Result<(), VirtioError> {
        let queue = &mut self.queues[queue_idx as usize];
        
        while let Some(desc_chain) = queue.pop_descriptor_chain() {
            let request = VirtioBlockRequest::parse(&desc_chain)?;
            
            match request.request_type {
                VIRTIO_BLK_T_IN => {
                    self.backend.read(request.sector, &mut request.data)?;
                }
                VIRTIO_BLK_T_OUT => {
                    self.backend.write(request.sector, &request.data)?;
                }
                VIRTIO_BLK_T_FLUSH => {
                    self.backend.flush()?;
                }
                _ => return Err(VirtioError::UnsupportedRequest),
            }
            
            queue.push_used(desc_chain.head_index(), request.data.len() as u32)?;
        }
        
        Ok(())
    }
}
```

### VirtIO Network Device

```rust
/// VirtIO network device
pub struct VirtioNet {
    pub common: VirtioCommon,
    pub config: VirtioNetConfig,
    pub rx_queue: Virtqueue,
    pub tx_queue: Virtqueue,
    pub backend: Box<dyn NetBackend>,
}

pub struct VirtioNetConfig {
    pub mac: [u8; 6],
    pub status: u16,
    pub max_virtqueue_pairs: u16,
    pub mtu: u16,
}
```

### Virtual GPU (Future)

```rust
/// Virtual GPU for Windows guest
pub struct VirtualGpu {
    pub framebuffer: FrameBuffer,
    pub cursor: CursorState,
    pub display_info: DisplayInfo,
}

pub trait VirtualGpuBackend {
    fn create_resource(&mut self, id: u32, format: Format, width: u32, height: u32);
    fn transfer_to_host(&mut self, id: u32, rect: Rect);
    fn flush(&mut self, id: u32, rect: Rect);
}
```

---

## Guest Management

### VM Lifecycle API

```rust
/// Virtual machine
pub struct VirtualMachine {
    pub id: VmId,
    pub config: VmConfig,
    pub state: VmState,
    pub vcpus: Vec<VCpu>,
    pub memory: GuestMemory,
    pub devices: Vec<Box<dyn VirtioDevice>>,
}

pub struct VmConfig {
    pub name: String,
    pub memory_mb: u64,
    pub vcpu_count: u32,
    pub boot_source: BootSource,
    pub devices: Vec<DeviceConfig>,
}

pub enum VmState {
    Created,
    Running,
    Paused,
    Stopped,
}

impl VirtualMachine {
    /// Create a new VM
    pub fn create(config: VmConfig) -> Result<Self, VmError>;
    
    /// Start the VM
    pub fn start(&mut self) -> Result<(), VmError>;
    
    /// Pause the VM
    pub fn pause(&mut self) -> Result<(), VmError>;
    
    /// Resume the VM
    pub fn resume(&mut self) -> Result<(), VmError>;
    
    /// Stop the VM
    pub fn stop(&mut self) -> Result<(), VmError>;
    
    /// Destroy the VM
    pub fn destroy(self) -> Result<(), VmError>;
}
```

---

## Windows Integration

### Seamless Mode

```rust
/// Windows seamless mode integration
pub struct SeamlessMode {
    pub enabled: bool,
    pub windows: Vec<GuestWindow>,
    pub compositor: Compositor,
}

pub struct GuestWindow {
    pub hwnd: u64,
    pub title: String,
    pub rect: Rect,
    pub visible: bool,
    pub minimized: bool,
}

impl SeamlessMode {
    /// Sync guest windows with host compositor
    pub fn sync_windows(&mut self, guest_agent: &GuestAgent) -> Result<(), Error> {
        let guest_windows = guest_agent.enumerate_windows()?;
        
        for window in guest_windows {
            self.compositor.update_window(window)?;
        }
        
        Ok(())
    }
}
```

### Shared Folders (9P/VirtIO-fs)

```rust
/// 9P filesystem for host-guest sharing
pub struct Virtio9p {
    pub common: VirtioCommon,
    pub tag: String,
    pub root_path: PathBuf,
}

impl Virtio9p {
    pub fn handle_tattach(&mut self, msg: &Tattach) -> Result<Rattach, Error>;
    pub fn handle_twalk(&mut self, msg: &Twalk) -> Result<Rwalk, Error>;
    pub fn handle_topen(&mut self, msg: &Topen) -> Result<Ropen, Error>;
    pub fn handle_tread(&mut self, msg: &Tread) -> Result<Rread, Error>;
    pub fn handle_twrite(&mut self, msg: &Twrite) -> Result<Rwrite, Error>;
    pub fn handle_tclunk(&mut self, msg: &Tclunk) -> Result<Rclunk, Error>;
}
```

### Clipboard Sync

```rust
/// Clipboard synchronization
pub struct ClipboardSync {
    pub host_clipboard: HostClipboard,
    pub guest_agent: GuestAgentConnection,
}

impl ClipboardSync {
    /// Sync clipboard from host to guest
    pub fn host_to_guest(&mut self) -> Result<(), Error> {
        let content = self.host_clipboard.get()?;
        self.guest_agent.set_clipboard(content)?;
        Ok(())
    }
    
    /// Sync clipboard from guest to host
    pub fn guest_to_host(&mut self) -> Result<(), Error> {
        let content = self.guest_agent.get_clipboard()?;
        self.host_clipboard.set(content)?;
        Ok(())
    }
}
```

---

## Security Considerations

### VM Isolation

1. **Memory Isolation**: EPT/NPT prevents guest from accessing host memory
2. **IOMMU**: Prevents DMA attacks from passthrough devices
3. **Interrupt Isolation**: Virtual APIC prevents interrupt injection
4. **Time Isolation**: TSC offsetting prevents timing attacks

### VM Escape Prevention

```rust
/// Security checks for VM operations
pub fn validate_guest_access(
    vcpu: &VCpu,
    access: &GuestAccess,
) -> Result<(), SecurityError> {
    // Validate memory access is within guest bounds
    if !vcpu.memory.contains(access.address, access.size) {
        return Err(SecurityError::OutOfBoundsAccess);
    }
    
    // Validate I/O port access is permitted
    if let GuestAccess::IoPort { port, .. } = access {
        if !vcpu.io_bitmap.is_allowed(*port) {
            return Err(SecurityError::UnauthorizedIoAccess);
        }
    }
    
    // Validate MSR access
    if let GuestAccess::Msr { msr, .. } = access {
        if !is_safe_msr(*msr) {
            return Err(SecurityError::DangerousMsrAccess);
        }
    }
    
    Ok(())
}
```

---

## API Reference

### Hypercall Interface

| Number | Name | Description |
|--------|------|-------------|
| 0x0001 | HC_VERSION | Get hypervisor version |
| 0x0002 | HC_FEATURES | Query supported features |
| 0x0010 | HC_MEMORY_MAP | Get guest memory map |
| 0x0020 | HC_TIMER | Set virtual timer |
| 0x0030 | HC_IPC | Host-guest IPC |
| 0x0040 | HC_CLIPBOARD | Clipboard operations |
| 0x0050 | HC_DISPLAY | Display operations |

### Management Commands

```bash
# Create a VM
v-os-hypervisor create --name "Windows 11" --memory 4096 --cpus 2 --disk win11.qcow2

# Start a VM
v-os-hypervisor start "Windows 11"

# List VMs
v-os-hypervisor list

# Stop a VM
v-os-hypervisor stop "Windows 11"

# Enable seamless mode
v-os-hypervisor seamless --enable "Windows 11"
```

---

## References

- Intel SDM Volume 3C: VMX
- AMD APM Volume 2: SVM
- VirtIO Specification 1.1
- 9P Protocol Specification
