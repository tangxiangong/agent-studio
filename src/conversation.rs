use gpui::{
    px, App, AppContext, Context, ElementId, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement, Pixels, Render, Styled, Window,
};

use agent_client_protocol_schema::{
    BlobResourceContents, ContentBlock, ContentChunk, EmbeddedResource, EmbeddedResourceResource,
    ImageContent, ResourceLink, TextContent, TextResourceContents,
};
use gpui_component::{scroll::ScrollbarAxis, v_flex, ActiveTheme, StyledExt};

use crate::{
    conversation_schema::{
        AgentMessageDataSchema, ContentBlockSchema, ConversationItem, PlanEntrySchema,
        ResourceContentsSchema, ToolCallItemSchema, UserMessageDataSchema,
    },
    AgentMessage, AgentMessageData, AgentMessageMeta, AgentTodoList, PlanEntry, PlanEntryPriority,
    PlanEntryStatus, ToolCallContent, ToolCallData, ToolCallItem, ToolCallKind, ToolCallStatus,
    UserMessage, UserMessageData,
};

pub struct ConversationPanel {
    focus_handle: FocusHandle,
    items: Vec<ConversationItem>,
}

impl crate::dock_panel::DockPanel for ConversationPanel {
    fn title() -> &'static str {
        "Conversation"
    }

    fn description() -> &'static str {
        "A conversation view with agent messages, user messages, tool calls, and todos."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }

    fn paddings() -> Pixels {
        px(0.)
    }
}

impl ConversationPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(_: &mut Window, cx: &mut App) -> Self {
        let json_content = include_str!("fixtures/mock_conversation.json");
        let items: Vec<ConversationItem> =
            serde_json::from_str(json_content).expect("Failed to parse mock conversation");

        Self {
            focus_handle: cx.focus_handle(),
            items,
        }
    }

    fn get_id(id: &str) -> ElementId {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        ElementId::from(("item", hasher.finish()))
    }

    fn map_user_message(id: String, data: UserMessageDataSchema) -> UserMessage {
        let mut user_data = UserMessageData::new(data.session_id);

        // Convert content blocks from schema to ACP types
        for content_schema in data.prompt {
            let content_block = Self::map_content_block(content_schema);
            user_data.contents.push(content_block);
        }

        UserMessage::new(Self::get_id(&id), user_data)
    }

    /// Convert schema ContentBlock to ACP ContentBlock
    fn map_content_block(schema: ContentBlockSchema) -> ContentBlock {
        match schema {
            ContentBlockSchema::Text(text) => ContentBlock::Text(TextContent::new(text.text)),
            ContentBlockSchema::Image(image) => {
                ContentBlock::Image(ImageContent::new(image.data, image.mime_type))
            }
            ContentBlockSchema::ResourceLink(link) => {
                let mut resource_link = ResourceLink::new(link.name, link.uri);
                if let Some(mime) = link.mime_type {
                    resource_link = resource_link.mime_type(mime);
                }
                ContentBlock::ResourceLink(resource_link)
            }
            ContentBlockSchema::Resource(embedded) => {
                let resource = match embedded.resource {
                    ResourceContentsSchema::TextResourceContents(text_res) => {
                        let mut content = TextResourceContents::new(text_res.text, text_res.uri);
                        if let Some(mime) = text_res.mime_type {
                            content = content.mime_type(mime);
                        }
                        EmbeddedResourceResource::TextResourceContents(content)
                    }
                    ResourceContentsSchema::BlobResourceContents(blob_res) => {
                        let mut content = BlobResourceContents::new(blob_res.blob, blob_res.uri);
                        if let Some(mime) = blob_res.mime_type {
                            content = content.mime_type(mime);
                        }
                        EmbeddedResourceResource::BlobResourceContents(content)
                    }
                };
                ContentBlock::Resource(EmbeddedResource::new(resource))
            }
        }
    }

    fn map_agent_message(id: String, data: AgentMessageDataSchema) -> AgentMessage {
        let mut agent_data = AgentMessageData::new(data.session_id);

        // Set metadata from _meta field
        if let Some(meta) = data.meta {
            agent_data.meta = AgentMessageMeta {
                agent_name: meta.agent_name,
                is_complete: meta.is_complete,
            };
        }

        // Convert content chunks
        for chunk_schema in data.chunks {
            let content_block = Self::map_content_block(chunk_schema.content);
            let mut content_chunk = ContentChunk::new(content_block);
            if let Some(meta) = chunk_schema.meta {
                content_chunk = content_chunk.meta(meta);
            }
            agent_data.chunks.push(content_chunk);
        }

        AgentMessage::new(Self::get_id(&id), agent_data)
    }

    fn map_todo_list(title: String, entries: Vec<PlanEntrySchema>) -> AgentTodoList {
        let plan_entries = entries
            .into_iter()
            .map(|e| {
                let priority = match e.priority.as_str() {
                    "High" => PlanEntryPriority::High,
                    "Medium" => PlanEntryPriority::Medium,
                    "Low" => PlanEntryPriority::Low,
                    _ => PlanEntryPriority::Medium,
                };
                let status = match e.status.as_str() {
                    "Pending" => PlanEntryStatus::Pending,
                    "InProgress" => PlanEntryStatus::InProgress,
                    "Completed" => PlanEntryStatus::Completed,
                    _ => PlanEntryStatus::Pending,
                };
                PlanEntry::new(e.content)
                    .with_priority(priority)
                    .with_status(status)
            })
            .collect();

        AgentTodoList::new().title(title).entries(plan_entries)
    }

    fn map_tool_call(item: ToolCallItemSchema) -> ToolCallItem {
        let kind = ToolCallKind::from_str(&item.data.kind.to_lowercase());
        let status = match item.data.status.as_str() {
            "Pending" => ToolCallStatus::Pending,
            "InProgress" => ToolCallStatus::InProgress,
            "Completed" => ToolCallStatus::Completed,
            "Failed" => ToolCallStatus::Failed,
            _ => ToolCallStatus::Pending,
        };

        let content = item
            .data
            .content
            .into_iter()
            .map(|c| ToolCallContent::new(c.text))
            .collect();

        let data = ToolCallData::new(item.data.tool_call_id, item.data.title)
            .with_kind(kind)
            .with_status(status)
            .with_content(content);

        ToolCallItem::new(Self::get_id(&item.id), data).open(item.open)
    }
}

impl Focusable for ConversationPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ConversationPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut children = v_flex().p_4().gap_6().bg(cx.theme().background);

        for item in &self.items {
            match item {
                ConversationItem::UserMessage { id, data } => {
                    let user_msg = Self::map_user_message(id.clone(), data.clone());
                    children = children.child(user_msg);
                }
                ConversationItem::AgentMessage { id, data } => {
                    let agent_msg = Self::map_agent_message(id.clone(), data.clone());
                    children = children.child(agent_msg);
                }
                ConversationItem::AgentTodoList { title, entries } => {
                    let todo_list = Self::map_todo_list(title.clone(), entries.clone());
                    // Apply indentation for todo list
                    children = children.child(v_flex().pl_6().child(todo_list));
                }
                ConversationItem::ToolCallGroup { items } => {
                    let mut group = v_flex().pl_6().gap_2();
                    for tool_item in items {
                        let tool_call = Self::map_tool_call(tool_item.clone());
                        group = group.child(tool_call);
                    }
                    children = children.child(group);
                }
            }
        }

        children.scrollable(ScrollbarAxis::Vertical).size_full()
    }
}
