// Conversation panel module - modularized for better maintainability

mod helpers;
mod types;
mod components;
mod rendered_item;
mod panel;

// Re-export public API
pub use panel::ConversationPanel;
