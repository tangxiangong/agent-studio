use agent_client_protocol as acp;
use agent_client_protocol_schema as schema;
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::{
    acp_client::PermissionStore,
    session_bus::{SessionUpdateBusContainer, SessionUpdateEvent},
};

/// Convert from agent_client_protocol SessionUpdate to agent_client_protocol_schema SessionUpdate
///
/// Uses JSON serialization/deserialization as a bridge between the two incompatible versions
fn convert_session_update(update: &acp::SessionUpdate) -> schema::SessionUpdate {
    // Serialize the protocol version to JSON
    let json_value = serde_json::to_value(update)
        .expect("Failed to serialize SessionUpdate from protocol");

    // Deserialize into the schema version
    serde_json::from_value(json_value)
        .expect("Failed to deserialize SessionUpdate to schema version")
}

/// GUI Client that publishes session updates to the event bus
pub struct GuiClient {
    agent_name: String,
    permission_store: Arc<PermissionStore>,
    session_bus: SessionUpdateBusContainer,
}

impl GuiClient {
    pub fn new(
        agent_name: String,
        permission_store: Arc<PermissionStore>,
        session_bus: SessionUpdateBusContainer,
    ) -> Self {
        Self {
            agent_name,
            permission_store,
            session_bus,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl acp::Client for GuiClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        let (tx, rx) = oneshot::channel();
        let id = self
            .permission_store
            .add(self.agent_name.clone(), args.session_id.to_string(), tx)
            .await;

        println!(
            "\n[PERMISSION REQUEST] Agent '{}' session '{}'",
            self.agent_name, args.session_id
        );

        if let Some(title) = &args.tool_call.fields.title {
            println!("  Action: {}", title);
        }
        if let Some(locations) = &args.tool_call.fields.locations {
            for loc in locations {
                println!("  Location: {:?}", loc.path);
            }
        }

        println!("Options:");
        for opt in &args.options {
            println!("  [{}] {}", opt.id.0, opt.name);
        }

        println!("To select an option, type: /decide {} <option_id>", id);

        rx.await.map_err(|_| {
            acp::Error::internal_error().with_data("permission request channel closed")
        })
    }

    async fn write_text_file(
        &self,
        _args: acp::WriteTextFileRequest,
    ) -> acp::Result<acp::WriteTextFileResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn read_text_file(
        &self,
        _args: acp::ReadTextFileRequest,
    ) -> acp::Result<acp::ReadTextFileResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn create_terminal(
        &self,
        _args: acp::CreateTerminalRequest,
    ) -> Result<acp::CreateTerminalResponse, acp::Error> {
        Err(acp::Error::method_not_found())
    }

    async fn terminal_output(
        &self,
        _args: acp::TerminalOutputRequest,
    ) -> acp::Result<acp::TerminalOutputResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn release_terminal(
        &self,
        _args: acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn wait_for_terminal_exit(
        &self,
        _args: acp::WaitForTerminalExitRequest,
    ) -> acp::Result<acp::WaitForTerminalExitResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn kill_terminal_command(
        &self,
        _args: acp::KillTerminalCommandRequest,
    ) -> acp::Result<acp::KillTerminalCommandResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn session_notification(
        &self,
        args: acp::SessionNotification,
    ) -> acp::Result<(), acp::Error> {
        // Publish event to the session bus
        let event = SessionUpdateEvent {
            session_id: args.session_id.to_string(),
            update: Arc::new(convert_session_update(&args.update)),
        };

        self.session_bus.publish(event);

        // Also print to console for debugging
        match &args.update {
            acp::SessionUpdate::UserMessageChunk(chunk) => {
                println!("\n[{}] User: {:?}", self.agent_name, extract_text(&chunk.content));
            }
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                println!("\n[{}] Agent: {:?}", self.agent_name, extract_text(&chunk.content));
            }
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                println!("\n[{}] Thought: {:?}", self.agent_name, extract_text(&chunk.content));
            }
            acp::SessionUpdate::ToolCall(tool_call) => {
                println!("\n[{}] Tool: {}", self.agent_name, tool_call.title);
            }
            acp::SessionUpdate::Plan(plan) => {
                println!("\n[{}] Plan with {} entries", self.agent_name, plan.entries.len());
            }
            _ => {}
        }

        Ok(())
    }

    async fn ext_method(&self, _args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn ext_notification(&self, _args: acp::ExtNotification) -> acp::Result<()> {
        Ok(())
    }
}

fn extract_text(content: &acp::ContentBlock) -> String {
    match content {
        acp::ContentBlock::Text(text_content) => text_content.text.to_string(),
        acp::ContentBlock::Image(_) => "<image>".into(),
        acp::ContentBlock::Audio(_) => "<audio>".into(),
        acp::ContentBlock::ResourceLink(resource_link) => resource_link.uri.to_string(),
        acp::ContentBlock::Resource(_) => "<resource>".into(),
    }
}
