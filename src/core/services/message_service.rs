//! Message Service - Handles message sending and event bus interaction
//!
//! This service provides a high-level API for sending messages and subscribing
//! to session updates. It orchestrates between AgentService and SessionBus.

use std::sync::Arc;

use agent_client_protocol_schema as schema;
use agent_client_protocol_schema::SessionUpdate;
use anyhow::{anyhow, Result};

use crate::core::event_bus::session_bus::{SessionUpdateBusContainer, SessionUpdateEvent};

use super::agent_service::AgentService;

/// Message service - handles message sending and event bus interaction
pub struct MessageService {
    session_bus: SessionUpdateBusContainer,
    agent_service: Arc<AgentService>,
}

impl MessageService {
    pub fn new(
        session_bus: SessionUpdateBusContainer,
        agent_service: Arc<AgentService>,
    ) -> Self {
        Self {
            session_bus,
            agent_service,
        }
    }

    /// Send a user message (complete flow)
    ///
    /// This method performs the following steps:
    /// 1. Get or create a session for the agent
    /// 2. Publish the user message to the event bus (immediate UI feedback)
    /// 3. Send the prompt to the agent
    ///
    /// Returns the session ID on success.
    pub async fn send_user_message(
        &self,
        agent_name: &str,
        message: String,
    ) -> Result<String> {
        // 1. Get or create session
        let session_id = self
            .agent_service
            .get_or_create_session(agent_name)
            .await
            .map_err(|e| anyhow!("Agent error: {}", e))?;

        // 2. Publish user message to event bus (immediate UI feedback)
        self.publish_user_message(&session_id, &message);

        // 3. Send prompt to agent
        self.agent_service
            .send_prompt(agent_name, &session_id, vec![message])
            .await
            .map_err(|e| anyhow!("Failed to send message: {}", e))?;

        Ok(session_id)
    }

    /// Send a user message to an existing session
    ///
    /// This method performs the following steps:
    /// 1. Publish the user message to the event bus (immediate UI feedback)
    /// 2. Send the prompt to the agent
    ///
    /// Use this when you already have a session ID and want to ensure
    /// the UI panel has subscribed before the message is sent.
    pub async fn send_message_to_session(
        &self,
        agent_name: &str,
        session_id: &str,
        message: String,
    ) -> Result<()> {
        // 1. Publish user message to event bus (immediate UI feedback)
        self.publish_user_message(session_id, &message);

        // 2. Send prompt to agent
        self.agent_service
            .send_prompt(agent_name, session_id, vec![message])
            .await
            .map_err(|e| anyhow!("Failed to send message: {}", e))?;

        Ok(())
    }

    /// Publish a user message to the event bus (immediate UI feedback)
    pub fn publish_user_message(&self, session_id: &str, message: &str) {
        let content_block = schema::ContentBlock::from(message.to_string());
        let content_chunk = schema::ContentChunk::new(content_block);

        let user_event = SessionUpdateEvent {
            session_id: session_id.to_string(),
            update: Arc::new(schema::SessionUpdate::UserMessageChunk(content_chunk)),
        };

        self.session_bus.publish(user_event);
        log::debug!("Published user message to session bus: {}", session_id);
    }

    /// Subscribe to session updates
    ///
    /// Returns a channel receiver for session updates. If session_id is provided,
    /// only updates for that session will be received.
    pub fn subscribe_session_updates(
        &self,
        session_id: Option<String>,
    ) -> tokio::sync::mpsc::UnboundedReceiver<SessionUpdate> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        self.session_bus.subscribe(move |event| {
            // Filter by session_id if specified
            if let Some(ref filter_id) = session_id {
                if &event.session_id != filter_id {
                    return;
                }
            }

            let _ = tx.send((*event.update).clone());
        });

        rx
    }
}
