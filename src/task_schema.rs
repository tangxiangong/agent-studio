use gpui::SharedString;
use serde::Deserialize;

/// Task status enumeration
#[derive(Clone, Default, Debug, Deserialize)]
pub enum TaskStatus {
    /// Task is pending
    #[default]
    Pending,
    /// Task is currently running
    InProgress,
    /// Task completed successfully
    Completed,
    /// Task failed to complete
    Failed,
}

#[derive(Clone, Default, Deserialize)]
pub struct AgentTask {
    pub name: String,
    pub task_type: String,
    pub add_new_code_lines: i16,
    pub delete_code_lines: i16,
    pub status: TaskStatus,

    #[serde(skip)]
    pub change_timestamp: i16,
    #[serde(skip)]
    pub change_timestamp_str: SharedString,
    #[serde(skip)]
    pub add_new_code_lines_str: SharedString,
    #[serde(skip)]
    pub delete_code_lines_str: SharedString,
}

impl AgentTask {
    pub fn prepare(mut self) -> Self {
        self.add_new_code_lines_str = format!("+{}", self.add_new_code_lines).into();
        self.delete_code_lines_str = format!("-{}", self.delete_code_lines).into();
        self
    }
}
