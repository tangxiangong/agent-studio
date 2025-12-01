// Panel-related modules
pub mod dock_panel;
pub mod code_editor;
mod chat_input;
mod conversation;
mod conversation_acp;
mod settings_window;
mod task_list;
mod welcome_panel;

// Re-export panel types
pub use chat_input::ChatInputPanel;
pub use code_editor::CodeEditorPanel;
pub use conversation::ConversationPanel;
pub use conversation_acp::ConversationPanelAcp;
pub use dock_panel::{DockPanel, DockPanelContainer, DockPanelState};
pub use settings_window::SettingsWindow;
pub use task_list::ListTaskPanel;
pub use welcome_panel::WelcomePanel;
