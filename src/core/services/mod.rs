//! Service layer for business logic
//!
//! This module provides a service layer that separates business logic from UI components.
//! The services handle agent operations, session management, and message distribution.

mod agent_config_service;
mod agent_service;
mod message_service;
mod persistence_service;
mod workspace_service;

pub use agent_config_service::AgentConfigService;
pub use agent_service::{AgentService, AgentSessionInfo, SessionStatus};
pub use message_service::MessageService;
pub use persistence_service::PersistenceService;
pub use workspace_service::WorkspaceService;
