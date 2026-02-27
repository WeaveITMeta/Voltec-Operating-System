// =============================================================================
// V-OS - Kernel Services
// =============================================================================
// Table of Contents:
//   1. Module Declarations
//   2. Re-exports
//   3. Prelude Module
// =============================================================================
// Purpose: Microkernel-style services for the V-OS. Provides capability-
//          based access control, message passing IPC, task scheduling, and
//          resource handle management extended for quantum device access.
// =============================================================================

pub mod capability;
pub mod channel;
pub mod handle;
pub mod message;
pub mod task;

pub mod prelude {
    pub use crate::capability::*;
    pub use crate::channel::*;
    pub use crate::handle::*;
    pub use crate::message::*;
    pub use crate::task::*;
}
