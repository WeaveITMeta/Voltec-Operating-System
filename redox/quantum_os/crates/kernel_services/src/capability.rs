// =============================================================================
// V-OS - Capability-Based Access Control
// =============================================================================
// Table of Contents:
//   1. AccessCapability - Base capability structure
//   2. QuantumAccessPermission - Quantum-specific permissions
//   3. CapabilitySet - Collection of capabilities for a process
// =============================================================================
// Purpose: Implements capability-based security for the V-OS. Each
//          process holds capabilities that grant access to specific resources.
//          Extended with quantum-specific permissions for device access.
// =============================================================================

use crate::handle::{HandleType, ResourceHandle};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

// =============================================================================
// 1. AccessCapability - Base capability structure
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccessCapability {
    id: Uuid,
    resource_handle_id: Uuid,
    permissions: PermissionFlags,
    granted_at: u64,
    expires_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PermissionFlags {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub delegate: bool,
}

impl PermissionFlags {
    pub const READ_ONLY: Self = Self {
        read: true,
        write: false,
        execute: false,
        delegate: false,
    };

    pub const READ_WRITE: Self = Self {
        read: true,
        write: true,
        execute: false,
        delegate: false,
    };

    pub const FULL: Self = Self {
        read: true,
        write: true,
        execute: true,
        delegate: true,
    };
}

impl AccessCapability {
    pub fn new(resource_handle: &ResourceHandle, permissions: PermissionFlags) -> Self {
        Self {
            id: Uuid::new_v4(),
            resource_handle_id: resource_handle.id(),
            permissions,
            granted_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            expires_at: None,
        }
    }

    pub fn with_expiration(mut self, expires_at: u64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    pub fn can_read(&self) -> bool {
        self.permissions.read && !self.is_expired()
    }

    pub fn can_write(&self) -> bool {
        self.permissions.write && !self.is_expired()
    }

    pub fn can_execute(&self) -> bool {
        self.permissions.execute && !self.is_expired()
    }

    pub fn can_delegate(&self) -> bool {
        self.permissions.delegate && !self.is_expired()
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            now >= expires
        } else {
            false
        }
    }
}

// =============================================================================
// 2. QuantumAccessPermission - Quantum-specific permissions
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuantumAccessPermission {
    base_capability: AccessCapability,
    quantum_permissions: QuantumPermissionFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuantumPermissionFlags {
    pub submit_circuit: bool,
    pub read_measurements: bool,
    pub access_calibration: bool,
    pub modify_device_config: bool,
    pub priority_execution: bool,
}

impl QuantumPermissionFlags {
    pub const USER_DEFAULT: Self = Self {
        submit_circuit: true,
        read_measurements: true,
        access_calibration: false,
        modify_device_config: false,
        priority_execution: false,
    };

    pub const ADMIN: Self = Self {
        submit_circuit: true,
        read_measurements: true,
        access_calibration: true,
        modify_device_config: true,
        priority_execution: true,
    };
}

impl QuantumAccessPermission {
    pub fn new(
        resource_handle: &ResourceHandle,
        base_permissions: PermissionFlags,
        quantum_permissions: QuantumPermissionFlags,
    ) -> Self {
        Self {
            base_capability: AccessCapability::new(resource_handle, base_permissions),
            quantum_permissions,
        }
    }

    pub fn can_submit_circuit(&self) -> bool {
        self.quantum_permissions.submit_circuit && self.base_capability.can_execute()
    }

    pub fn can_read_measurements(&self) -> bool {
        self.quantum_permissions.read_measurements && self.base_capability.can_read()
    }

    pub fn can_access_calibration(&self) -> bool {
        self.quantum_permissions.access_calibration && self.base_capability.can_read()
    }
}

// =============================================================================
// 3. CapabilitySet - Collection of capabilities for a process
// =============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilitySet {
    capabilities: HashSet<AccessCapability>,
    quantum_permissions: Vec<QuantumAccessPermission>,
}

impl CapabilitySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_capability(&mut self, capability: AccessCapability) {
        self.capabilities.insert(capability);
    }

    pub fn add_quantum_permission(&mut self, permission: QuantumAccessPermission) {
        self.quantum_permissions.push(permission);
    }

    pub fn has_access_to(&self, resource_id: Uuid) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap.resource_handle_id == resource_id && !cap.is_expired())
    }

    pub fn prune_expired(&mut self) {
        self.capabilities.retain(|cap| !cap.is_expired());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::HandleType;

    #[test]
    fn test_capability_permissions() {
        let handle = ResourceHandle::new(HandleType::QuantumDevice);
        let cap = AccessCapability::new(&handle, PermissionFlags::READ_ONLY);

        assert!(cap.can_read());
        assert!(!cap.can_write());
        assert!(!cap.can_execute());
    }

    #[test]
    fn test_quantum_permissions() {
        let handle = ResourceHandle::new(HandleType::QuantumDevice);
        let qperm = QuantumAccessPermission::new(
            &handle,
            PermissionFlags::FULL,
            QuantumPermissionFlags::USER_DEFAULT,
        );

        assert!(qperm.can_submit_circuit());
        assert!(qperm.can_read_measurements());
        assert!(!qperm.can_access_calibration());
    }
}
