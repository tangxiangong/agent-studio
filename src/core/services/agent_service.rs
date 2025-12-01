//! Agent Service - Manages agents and their sessions
//!
//! This service acts as a facade for agent operations and session management.
//! It follows the Aggregate Root pattern where Agent is the aggregate root
//! and Session is a child entity.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use agent_client_protocol as acp;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};

use crate::core::agent::{AgentHandle, AgentManager};

/// Agent service - manages agents and their sessions
pub struct AgentService {
    agent_manager: Arc<AgentManager>,
    /// Stores agent -> session mapping (one active session per agent)
    sessions: Arc<RwLock<HashMap<String, AgentSessionInfo>>>,
}

/// Agent session information
#[derive(Clone, Debug)]
pub struct AgentSessionInfo {
    pub session_id: String,
    pub agent_name: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub status: SessionStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionStatus {
    Active,
    Idle,
    Closed,
}

impl AgentService {
    pub fn new(agent_manager: Arc<AgentManager>) -> Self {
        Self {
            agent_manager,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // ========== Agent Operations ==========

    /// List all available agents
    pub fn list_agents(&self) -> Vec<String> {
        self.agent_manager.list_agents()
    }

    /// Get agent handle (internal use)
    fn get_agent_handle(&self, name: &str) -> Result<Arc<AgentHandle>> {
        self.agent_manager
            .get(name)
            .ok_or_else(|| anyhow!("Agent not found: {}", name))
    }

    // ========== Session Operations ==========

    /// Create a new session for the agent
    pub async fn create_session(&self, agent_name: &str) -> Result<String> {
        let agent_handle = self.get_agent_handle(agent_name)?;

        let request = acp::NewSessionRequest {
            cwd: std::env::current_dir().unwrap_or_default(),
            mcp_servers: vec![],
            meta: None,
        };

        let session_id = agent_handle
            .new_session(request)
            .await
            .map_err(|e| anyhow!("Failed to create session: {}", e))?
            .session_id
            .to_string();

        // Store session information
        let session_info = AgentSessionInfo {
            session_id: session_id.clone(),
            agent_name: agent_name.to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            status: SessionStatus::Active,
        };

        self.sessions
            .write()
            .unwrap()
            .insert(agent_name.to_string(), session_info);

        log::info!("Created session {} for agent {}", session_id, agent_name);
        Ok(session_id)
    }

    /// Get or create an active session for the agent (recommended)
    pub async fn get_or_create_session(&self, agent_name: &str) -> Result<String> {
        // Try to get existing active session
        if let Some(session_id) = self.get_active_session(agent_name) {
            log::debug!(
                "Reusing existing session {} for agent {}",
                session_id,
                agent_name
            );
            return Ok(session_id);
        }

        // No active session, create new one
        self.create_session(agent_name).await
    }

    /// Get the active session for an agent
    pub fn get_active_session(&self, agent_name: &str) -> Option<String> {
        self.sessions
            .read()
            .unwrap()
            .get(agent_name)
            .filter(|info| info.status == SessionStatus::Active)
            .map(|info| info.session_id.clone())
    }

    /// Get session information
    pub fn get_session_info(&self, agent_name: &str) -> Option<AgentSessionInfo> {
        self.sessions.read().unwrap().get(agent_name).cloned()
    }

    /// Close an agent's session
    pub async fn close_session(&self, agent_name: &str) -> Result<()> {
        if let Some(info) = self.sessions.write().unwrap().get_mut(agent_name) {
            info.status = SessionStatus::Closed;
            log::info!(
                "Closed session {} for agent {}",
                info.session_id,
                agent_name
            );
        }
        Ok(())
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Vec<AgentSessionInfo> {
        self.sessions.read().unwrap().values().cloned().collect()
    }

    /// Update session's last active time
    pub fn update_session_activity(&self, agent_name: &str) {
        if let Some(info) = self.sessions.write().unwrap().get_mut(agent_name) {
            info.last_active = Utc::now();
        }
    }

    // ========== Prompt Operations ==========

    /// Send a prompt to an agent's session
    pub async fn send_prompt(
        &self,
        agent_name: &str,
        session_id: &str,
        prompt: Vec<String>,
    ) -> Result<()> {
        let agent_handle = self.get_agent_handle(agent_name)?;

        let request = acp::PromptRequest {
            session_id: acp::SessionId::from(session_id.to_string()),
            prompt: prompt.into_iter().map(|s| s.into()).collect(),
            meta: None,
        };

        agent_handle
            .prompt(request)
            .await
            .map_err(|e| anyhow!("Failed to send prompt: {}", e))?;

        // Update activity time
        self.update_session_activity(agent_name);

        log::debug!("Sent prompt to agent {} session {}", agent_name, session_id);
        Ok(())
    }

    // ========== Cleanup Operations ==========

    /// Clean up idle sessions
    pub async fn cleanup_idle_sessions(&self, idle_duration: Duration) {
        let now = Utc::now();
        let mut sessions = self.sessions.write().unwrap();

        sessions.retain(|agent_name, info| {
            let idle_time = now.signed_duration_since(info.last_active);
            let should_keep = idle_time.num_seconds() < idle_duration.as_secs() as i64;

            if !should_keep {
                log::info!(
                    "Cleaning up idle session {} for agent {} (idle for {}s)",
                    info.session_id,
                    agent_name,
                    idle_time.num_seconds()
                );
            }

            should_keep
        });
    }
}
