# V-OS Security Model

## Table of Contents

1. [Overview](#overview)
2. [Security Principles](#security-principles)
3. [Memory Safety](#memory-safety)
4. [Capability-Based Security](#capability-based-security)
5. [Process Isolation](#process-isolation)
6. [Syscall Filtering](#syscall-filtering)
7. [Driver Security](#driver-security)
8. [Hypervisor Security](#hypervisor-security)
9. [Secure Boot](#secure-boot)
10. [Threat Model](#threat-model)
11. [Security Auditing](#security-auditing)

---

## Overview

V-OS implements a defense-in-depth security model with multiple layers of protection:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Security Layers                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│ Layer 7: Secure Boot Chain                                                   │
│          Verified firmware → bootloader → kernel → drivers                  │
├─────────────────────────────────────────────────────────────────────────────┤
│ Layer 6: VM Isolation (Hypervisor)                                          │
│          Hardware-enforced guest boundaries, IOMMU, nested paging           │
├─────────────────────────────────────────────────────────────────────────────┤
│ Layer 5: Driver Isolation                                                    │
│          Userspace drivers with limited hardware access                     │
├─────────────────────────────────────────────────────────────────────────────┤
│ Layer 4: Syscall Filtering                                                   │
│          Per-process syscall whitelists, seccomp-like filtering             │
├─────────────────────────────────────────────────────────────────────────────┤
│ Layer 3: Process Isolation                                                   │
│          Separate address spaces, capability requirements                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ Layer 2: Capability-Based Access Control                                     │
│          Fine-grained permissions, principle of least privilege             │
├─────────────────────────────────────────────────────────────────────────────┤
│ Layer 1: Memory Safety (Rust)                                               │
│          No buffer overflows, use-after-free, data races                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Security Principles

### 1. Principle of Least Privilege
Every component runs with the minimum permissions required.

### 2. Defense in Depth
Multiple independent security layers prevent single-point failures.

### 3. Fail-Safe Defaults
Access is denied by default; explicit grants are required.

### 4. Complete Mediation
Every access to every resource is checked.

### 5. Minimal Attack Surface
Small kernel (~30K LoC), userspace drivers, minimal syscall interface.

---

## Memory Safety

### Rust's Guarantees

V-OS is written in Rust, providing compile-time guarantees:

| Vulnerability Class | Prevention Mechanism |
|---------------------|---------------------|
| Buffer Overflow | Bounds checking, slice types |
| Use-After-Free | Ownership system, borrow checker |
| Double Free | Single ownership |
| Null Pointer Deref | Option<T> type, no null |
| Data Races | Send/Sync traits, ownership |
| Uninitialized Memory | Initialization requirements |

### Unsafe Code Policy

```rust
/// Unsafe code guidelines for V-OS
/// 
/// 1. Minimize unsafe blocks
/// 2. Document safety invariants
/// 3. Encapsulate in safe abstractions
/// 4. Audit regularly

/// Example: Safe wrapper around unsafe hardware access
pub struct MmioRegister<T> {
    ptr: *mut T,
}

impl<T> MmioRegister<T> {
    /// # Safety
    /// - `addr` must be a valid MMIO address
    /// - The register must be properly aligned
    /// - No other code may access this address
    pub unsafe fn new(addr: usize) -> Self {
        Self { ptr: addr as *mut T }
    }
    
    /// Safe read operation
    pub fn read(&self) -> T 
    where
        T: Copy,
    {
        // SAFETY: Invariants established at construction
        unsafe { core::ptr::read_volatile(self.ptr) }
    }
    
    /// Safe write operation
    pub fn write(&mut self, value: T) {
        // SAFETY: Invariants established at construction
        unsafe { core::ptr::write_volatile(self.ptr, value) }
    }
}
```

### Kernel ASLR

Address Space Layout Randomization for kernel:

```rust
/// Kernel ASLR configuration
pub struct KernelAslr {
    /// Randomization entropy bits
    pub entropy_bits: u8,
    /// Base address offset
    pub base_offset: usize,
    /// Stack randomization
    pub stack_randomize: bool,
    /// Heap randomization
    pub heap_randomize: bool,
}

impl KernelAslr {
    pub fn randomize_base(&self, original: usize) -> usize {
        let random = self.get_random_offset();
        original.wrapping_add(random & self.mask())
    }
}
```

---

## Capability-Based Security

### Capability Model

Every resource access requires a capability token:

```rust
/// Capability token
#[derive(Clone, Debug)]
pub struct Capability {
    /// Unique capability ID
    pub id: CapabilityId,
    /// Resource this capability grants access to
    pub resource: ResourceId,
    /// Permitted operations
    pub permissions: Permissions,
    /// Optional expiration time
    pub expires: Option<Timestamp>,
    /// Can this capability be delegated?
    pub delegatable: bool,
}

/// Permission flags
bitflags! {
    pub struct Permissions: u32 {
        const READ    = 0b0001;
        const WRITE   = 0b0010;
        const EXECUTE = 0b0100;
        const GRANT   = 0b1000;  // Can delegate to others
    }
}

/// Capability operations
impl Capability {
    /// Check if operation is permitted
    pub fn allows(&self, op: Operation) -> bool {
        if let Some(exp) = self.expires {
            if Timestamp::now() > exp {
                return false;
            }
        }
        self.permissions.contains(op.required_permissions())
    }
    
    /// Delegate capability (create restricted copy)
    pub fn delegate(&self, new_perms: Permissions) -> Result<Capability, CapError> {
        if !self.delegatable {
            return Err(CapError::NotDelegatable);
        }
        if !self.permissions.contains(Permissions::GRANT) {
            return Err(CapError::NoGrantPermission);
        }
        // New capability can only have subset of permissions
        let restricted = self.permissions & new_perms;
        Ok(Capability {
            id: CapabilityId::new(),
            resource: self.resource,
            permissions: restricted,
            expires: self.expires,
            delegatable: false,  // Delegated caps can't be re-delegated
        })
    }
}
```

### Capability Flow

```
┌──────────────┐                    ┌──────────────┐
│   Process    │                    │   Kernel     │
└──────┬───────┘                    └──────┬───────┘
       │                                   │
       │ open("/scheme/file/doc.txt")      │
       ├──────────────────────────────────>│
       │                                   │
       │                          ┌────────┴────────┐
       │                          │ Check process   │
       │                          │ capabilities    │
       │                          └────────┬────────┘
       │                                   │
       │                          ┌────────┴────────┐
       │                          │ Verify access   │
       │                          │ to /scheme/file │
       │                          └────────┬────────┘
       │                                   │
       │ <capability token>                │
       │<──────────────────────────────────┤
       │                                   │
       │ read(cap, buffer, len)            │
       ├──────────────────────────────────>│
       │                                   │
       │                          ┌────────┴────────┐
       │                          │ Validate cap    │
       │                          │ has READ perm   │
       │                          └────────┬────────┘
       │                                   │
       │ <data>                            │
       │<──────────────────────────────────┤
```

---

## Process Isolation

### Address Space Isolation

Each process has its own virtual address space:

```
Process A                          Process B
┌─────────────────────┐           ┌─────────────────────┐
│ 0x0000...           │           │ 0x0000...           │
│ ┌─────────────────┐ │           │ ┌─────────────────┐ │
│ │ Code            │ │           │ │ Code            │ │
│ ├─────────────────┤ │           │ ├─────────────────┤ │
│ │ Data            │ │           │ │ Data            │ │
│ ├─────────────────┤ │           │ ├─────────────────┤ │
│ │ Heap            │ │           │ │ Heap            │ │
│ ├─────────────────┤ │           │ ├─────────────────┤ │
│ │ Stack           │ │           │ │ Stack           │ │
│ └─────────────────┘ │           │ └─────────────────┘ │
│ 0x7FFF...           │           │ 0x7FFF...           │
└─────────────────────┘           └─────────────────────┘
         │                                 │
         │ Page Tables                     │ Page Tables
         ▼                                 ▼
┌─────────────────────┐           ┌─────────────────────┐
│ Physical Memory A   │           │ Physical Memory B   │
└─────────────────────┘           └─────────────────────┘
```

### Process Sandboxing

```rust
/// Sandbox configuration
pub struct Sandbox {
    /// Allowed syscalls
    pub syscall_filter: SyscallFilter,
    /// Allowed filesystem paths
    pub fs_whitelist: Vec<PathPattern>,
    /// Allowed network access
    pub net_policy: NetworkPolicy,
    /// Resource limits
    pub limits: ResourceLimits,
}

/// Resource limits
pub struct ResourceLimits {
    pub max_memory: usize,
    pub max_cpu_time: Duration,
    pub max_file_descriptors: usize,
    pub max_processes: usize,
}

impl Sandbox {
    /// Create a restrictive sandbox for untrusted code
    pub fn untrusted() -> Self {
        Self {
            syscall_filter: SyscallFilter::minimal(),
            fs_whitelist: vec![],
            net_policy: NetworkPolicy::Deny,
            limits: ResourceLimits {
                max_memory: 64 * 1024 * 1024,  // 64 MB
                max_cpu_time: Duration::from_secs(10),
                max_file_descriptors: 16,
                max_processes: 1,
            },
        }
    }
}
```

---

## Syscall Filtering

### Filter Configuration

```rust
/// Syscall filter (seccomp-like)
pub struct SyscallFilter {
    /// Default action for unlisted syscalls
    pub default_action: FilterAction,
    /// Per-syscall rules
    pub rules: HashMap<Syscall, FilterRule>,
}

pub enum FilterAction {
    Allow,
    Deny,
    Log,
    Kill,
}

pub struct FilterRule {
    pub action: FilterAction,
    /// Optional argument constraints
    pub arg_filters: Vec<ArgFilter>,
}

pub struct ArgFilter {
    pub arg_index: usize,
    pub op: CompareOp,
    pub value: u64,
}

impl SyscallFilter {
    /// Minimal filter for sandboxed processes
    pub fn minimal() -> Self {
        let mut filter = Self {
            default_action: FilterAction::Deny,
            rules: HashMap::new(),
        };
        
        // Allow only essential syscalls
        filter.allow(Syscall::Read);
        filter.allow(Syscall::Write);
        filter.allow(Syscall::Exit);
        filter.allow(Syscall::Mmap);  // With restrictions
        filter.allow(Syscall::Munmap);
        
        filter
    }
    
    /// Check if syscall is allowed
    pub fn check(&self, syscall: Syscall, args: &[u64]) -> FilterAction {
        match self.rules.get(&syscall) {
            Some(rule) => {
                // Check argument constraints
                for arg_filter in &rule.arg_filters {
                    if !arg_filter.matches(args) {
                        return FilterAction::Deny;
                    }
                }
                rule.action
            }
            None => self.default_action,
        }
    }
}
```

---

## Driver Security

### Userspace Driver Isolation

Drivers run in userspace with limited privileges:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Userspace                                       │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │ NVMe Driver     │  │ USB Driver      │  │ Network Driver              │  │
│  │                 │  │                 │  │                             │  │
│  │ Capabilities:   │  │ Capabilities:   │  │ Capabilities:               │  │
│  │ - MMIO region   │  │ - USB ports     │  │ - NIC MMIO                  │  │
│  │ - IRQ 10        │  │ - IRQ 11        │  │ - IRQ 12                    │  │
│  │ - DMA buffer    │  │ - DMA buffer    │  │ - DMA buffer                │  │
│  └────────┬────────┘  └────────┬────────┘  └─────────────┬───────────────┘  │
├───────────┴────────────────────┴────────────────────────┴───────────────────┤
│                              Kernel                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ Capability Manager                                                   │    │
│  │ - Validates all hardware access                                      │    │
│  │ - Enforces IOMMU boundaries                                          │    │
│  │ - Mediates interrupt delivery                                        │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────┘
```

### IOMMU Protection

```rust
/// IOMMU configuration for driver isolation
pub struct IommuDomain {
    /// Domain ID
    pub id: DomainId,
    /// Allowed DMA regions
    pub dma_regions: Vec<DmaRegion>,
    /// Device assignments
    pub devices: Vec<PciDevice>,
}

pub struct DmaRegion {
    /// Guest physical address (driver sees this)
    pub iova: u64,
    /// Host physical address (actual memory)
    pub hpa: u64,
    /// Region size
    pub size: usize,
    /// Permissions
    pub permissions: DmaPermissions,
}

impl IommuDomain {
    /// Map a DMA region for a driver
    pub fn map_dma(
        &mut self,
        buffer: &DmaBuffer,
        permissions: DmaPermissions,
    ) -> Result<u64, IommuError> {
        // Allocate IOVA
        let iova = self.allocate_iova(buffer.size())?;
        
        // Create mapping in IOMMU page tables
        self.create_mapping(iova, buffer.physical_address(), buffer.size(), permissions)?;
        
        Ok(iova)
    }
}
```

---

## Hypervisor Security

### Guest Isolation

```rust
/// VM security configuration
pub struct VmSecurityConfig {
    /// Enable IOMMU for passthrough devices
    pub iommu_enabled: bool,
    /// Prevent guest from accessing host memory
    pub memory_isolation: MemoryIsolation,
    /// Restrict guest MSR access
    pub msr_filtering: MsrFilter,
    /// Limit guest I/O port access
    pub io_filtering: IoFilter,
}

pub enum MemoryIsolation {
    /// Full EPT/NPT isolation
    Full,
    /// Shared memory regions allowed
    Partial { shared_regions: Vec<MemoryRegion> },
}

/// Validate guest memory access
pub fn validate_guest_access(
    vm: &VirtualMachine,
    gpa: GuestPhysicalAddress,
    size: usize,
    access_type: AccessType,
) -> Result<(), SecurityError> {
    // Check if access is within guest memory bounds
    if !vm.memory.contains(gpa, size) {
        return Err(SecurityError::OutOfBoundsAccess);
    }
    
    // Check if region is accessible for this access type
    let region = vm.memory.find_region(gpa)?;
    if !region.permissions.allows(access_type) {
        return Err(SecurityError::PermissionDenied);
    }
    
    // Check for MMIO regions that need emulation
    if region.is_mmio() {
        return Err(SecurityError::MmioAccess);
    }
    
    Ok(())
}
```

### VM Escape Prevention

```rust
/// Security checks to prevent VM escape
pub struct VmEscapePrevention {
    /// Validate all VM-exits
    pub exit_validation: bool,
    /// Sanitize guest-provided data
    pub input_sanitization: bool,
    /// Limit hypercall rate
    pub hypercall_rate_limit: Option<RateLimit>,
    /// Audit suspicious activity
    pub audit_logging: bool,
}

impl VmEscapePrevention {
    /// Validate VM-exit before handling
    pub fn validate_exit(&self, exit: &VmExit) -> Result<(), SecurityError> {
        // Check for suspicious exit patterns
        if self.is_suspicious_pattern(exit) {
            self.log_suspicious_activity(exit);
            return Err(SecurityError::SuspiciousActivity);
        }
        
        // Validate exit information
        match exit.reason {
            VmExitReason::IoInstruction => {
                self.validate_io_exit(exit)?;
            }
            VmExitReason::EptViolation => {
                self.validate_ept_exit(exit)?;
            }
            VmExitReason::Vmcall => {
                self.validate_hypercall(exit)?;
            }
            _ => {}
        }
        
        Ok(())
    }
}
```

---

## Secure Boot

### Boot Chain Verification

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Secure Boot Chain                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐                 │
│  │   UEFI       │────>│  Bootloader  │────>│   Kernel     │                 │
│  │   Firmware   │     │  (signed)    │     │   (signed)   │                 │
│  └──────────────┘     └──────────────┘     └──────────────┘                 │
│         │                    │                    │                          │
│         │ Verify             │ Verify             │ Verify                   │
│         ▼                    ▼                    ▼                          │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐                 │
│  │  Platform    │     │  Bootloader  │     │   Kernel     │                 │
│  │  Key (PK)    │     │  Certificate │     │  Certificate │                 │
│  └──────────────┘     └──────────────┘     └──────────────┘                 │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                        Kernel Verification                            │   │
│  │                                                                       │   │
│  │  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐          │   │
│  │  │   Drivers    │────>│   Services   │────>│   Apps       │          │   │
│  │  │   (signed)   │     │   (signed)   │     │   (optional) │          │   │
│  │  └──────────────┘     └──────────────┘     └──────────────┘          │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Signature Verification

```rust
/// Secure boot verification
pub struct SecureBoot {
    /// Platform key
    pub platform_key: PublicKey,
    /// Key exchange keys
    pub kek: Vec<PublicKey>,
    /// Signature database
    pub db: SignatureDatabase,
    /// Forbidden signatures
    pub dbx: SignatureDatabase,
}

impl SecureBoot {
    /// Verify a signed binary
    pub fn verify(&self, binary: &[u8], signature: &Signature) -> Result<(), VerifyError> {
        // Check if signature is in forbidden list
        if self.dbx.contains(signature) {
            return Err(VerifyError::Forbidden);
        }
        
        // Find matching certificate in database
        let cert = self.db.find_certificate(signature)?;
        
        // Verify signature
        cert.verify(binary, signature)?;
        
        Ok(())
    }
}
```

---

## Threat Model

### Threats Addressed

| Threat | Mitigation |
|--------|------------|
| Buffer overflow | Rust memory safety |
| Use-after-free | Rust ownership |
| Privilege escalation | Capability system |
| Driver compromise | Userspace isolation, IOMMU |
| VM escape | EPT/NPT, input validation |
| Boot tampering | Secure boot chain |
| Side-channel attacks | Kernel ASLR, timing mitigations |
| DMA attacks | IOMMU enforcement |

### Trust Boundaries

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Trust Boundaries                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ UNTRUSTED: Guest VMs                                                 │    │
│  │ - Assume fully malicious                                             │    │
│  │ - All input validated                                                │    │
│  │ - Hardware isolation (EPT/IOMMU)                                     │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ UNTRUSTED: User Applications                                         │    │
│  │ - Sandboxed by default                                               │    │
│  │ - Capability-gated access                                            │    │
│  │ - Syscall filtering                                                  │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ SEMI-TRUSTED: Userspace Drivers                                      │    │
│  │ - Limited hardware access                                            │    │
│  │ - IOMMU-enforced DMA                                                 │    │
│  │ - Crash doesn't affect kernel                                        │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ TRUSTED: Kernel                                                      │    │
│  │ - Minimal codebase (~30K LoC)                                        │    │
│  │ - Rust memory safety                                                 │    │
│  │ - Audited unsafe blocks                                              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ TRUSTED: Hardware/Firmware                                           │    │
│  │ - Assumed correct (out of scope)                                     │    │
│  │ - Secure boot validates chain                                        │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Security Auditing

### Audit Tools

```bash
# Audit unsafe blocks in kernel
v-os-audit unsafe --path kernel/

# Check for known vulnerabilities
v-os-audit cve --all

# Fuzz syscall interface
v-os-fuzz syscall --duration 1h

# Verify capability usage
v-os-audit capabilities --process <pid>

# Check driver isolation
v-os-audit drivers --verify-iommu
```

### Continuous Security

1. **Static Analysis**: Clippy, cargo-audit on every PR
2. **Fuzzing**: Continuous fuzzing of syscalls, drivers, hypervisor
3. **Unsafe Auditing**: Regular review of unsafe blocks
4. **Penetration Testing**: Periodic security assessments
5. **Bug Bounty**: Community security research program

---

## References

- [Redox OS Security](https://doc.redox-os.org/book/security.html)
- [Capability-Based Security](https://en.wikipedia.org/wiki/Capability-based_security)
- [Intel VT-d Specification](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
- [AMD IOMMU Specification](https://www.amd.com/en/support/tech-docs)
