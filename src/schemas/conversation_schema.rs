use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ConversationItem {
    UserMessage {
        id: String,
        data: UserMessageDataSchema,
    },
    AgentMessage {
        id: String,
        data: AgentMessageDataSchema,
    },
    AgentTodoList {
        title: String,
        entries: Vec<PlanEntrySchema>,
    },
    ToolCallGroup {
        items: Vec<ToolCallItemSchema>,
    },
}

/// User message data schema aligned with ACP's PromptRequest format
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserMessageDataSchema {
    pub session_id: String,
    /// Content blocks following ACP ContentBlock structure
    pub prompt: Vec<ContentBlockSchema>,
}

/// Content block schema aligned with ACP's ContentBlock enum
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockSchema {
    Text(TextContentSchema),
    Image(ImageContentSchema),
    ResourceLink(ResourceLinkSchema),
    Resource(EmbeddedResourceSchema),
}

/// Text content schema
#[derive(Debug, Deserialize, Clone)]
pub struct TextContentSchema {
    pub text: String,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Image content schema
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImageContentSchema {
    pub data: String,
    pub mime_type: String,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Resource link schema (reference to a resource without embedding content)
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLinkSchema {
    pub name: String,
    pub uri: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Embedded resource schema (contains the actual content)
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedResourceSchema {
    pub resource: ResourceContentsSchema,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Resource contents schema (text or blob)
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResourceContentsSchema {
    TextResourceContents(TextResourceContentsSchema),
    BlobResourceContents(BlobResourceContentsSchema),
}

/// Text resource contents schema
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextResourceContentsSchema {
    pub uri: String,
    pub text: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Blob resource contents schema
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlobResourceContentsSchema {
    pub uri: String,
    pub blob: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Agent message data schema aligned with ACP's ContentChunk format
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageDataSchema {
    pub session_id: String,
    /// Content chunks following ACP ContentChunk structure
    pub chunks: Vec<ContentChunkSchema>,
    /// Extended metadata (agent_name, is_complete stored in _meta)
    #[serde(rename = "_meta")]
    pub meta: Option<AgentMessageMetaSchema>,
}

/// Content chunk schema aligned with ACP's ContentChunk
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentChunkSchema {
    /// Content block following ACP's ContentBlock structure
    pub content: ContentBlockSchema,
    /// Extension point for implementations
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Extended metadata for agent messages
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageMetaSchema {
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub is_complete: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PlanEntrySchema {
    pub content: String,
    pub priority: String,
    pub status: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ToolCallItemSchema {
    pub id: String,
    pub data: ToolCallDataSchema,
    pub open: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ToolCallDataSchema {
    pub tool_call_id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub content: Vec<ToolCallContentSchema>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ToolCallContentSchema {
    pub text: String,
}
