// Event bus modules
pub mod permission_bus;
pub mod session_bus;

// Re-export event bus types
pub use permission_bus::{PermissionBusContainer, PermissionRequestEvent};
pub use session_bus::{SessionUpdateBusContainer, SessionUpdateEvent};
