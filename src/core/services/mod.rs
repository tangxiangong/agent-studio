//! Service layer for business logic
//!
//! This module provides a service layer that separates business logic from UI components.
//! The services handle agent operations, session management, and message distribution.

mod agent_service;
mod message_service;

pub use agent_service::AgentService;
pub use message_service::MessageService;
