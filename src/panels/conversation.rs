use gpui::{
    div, prelude::FluentBuilder as _, px, App, AppContext, Context, ElementId, Entity, FocusHandle,
    Focusable, IntoElement, ParentElement, Pixels, Render, SharedString, Styled, Window,
};

use agent_client_protocol_schema::{
    BlobResourceContents, Content, ContentBlock, ContentChunk, EmbeddedResource,
    EmbeddedResourceResource, ImageContent, Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus,
    ResourceLink, TextContent, TextResourceContents, ToolCall, ToolCallContent, ToolCallId,
    ToolCallStatus, ToolKind,
};
use gpui_component::{
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex,
    scroll::ScrollbarAxis,
    v_flex, ActiveTheme, Icon, IconName, Sizable, StyledExt,
};

use crate::{
    conversation_schema::{
        AgentMessageDataSchema, ContentBlockSchema, ConversationItem, PlanEntrySchema, PlanSchema,
        ResourceContentsSchema, ToolCallItemSchema, UserMessageDataSchema,
    },
    AgentMessage, AgentMessageData, AgentMessageMeta, AgentTodoList, UserMessageData,
};

// ============================================================================
// Helper Traits and Functions
// ============================================================================

trait ToolKindExt {
    fn icon(&self) -> IconName;
}

impl ToolKindExt for ToolKind {
    fn icon(&self) -> IconName {
        match self {
            ToolKind::Read => IconName::File,
            ToolKind::Edit => IconName::Replace,
            ToolKind::Delete => IconName::Delete,
            ToolKind::Move => IconName::ArrowRight,
            ToolKind::Search => IconName::Search,
            ToolKind::Execute => IconName::SquareTerminal,
            ToolKind::Think => IconName::Bot,
            ToolKind::Fetch => IconName::Globe,
            ToolKind::SwitchMode => IconName::ArrowRight,
            ToolKind::Other | _ => IconName::Ellipsis,
        }
    }
}

trait ToolCallStatusExt {
    fn icon(&self) -> IconName;
}

impl ToolCallStatusExt for ToolCallStatus {
    fn icon(&self) -> IconName {
        match self {
            ToolCallStatus::Pending => IconName::Dash,
            ToolCallStatus::InProgress => IconName::LoaderCircle,
            ToolCallStatus::Completed => IconName::CircleCheck,
            ToolCallStatus::Failed => IconName::CircleX,
            _ => IconName::Dash,
        }
    }
}

fn extract_filename(uri: &str) -> String {
    uri.split('/').next_back().unwrap_or("unknown").to_string()
}

fn get_file_icon(mime_type: &Option<String>) -> IconName {
    if let Some(ref mime) = mime_type {
        if mime.contains("python")
            || mime.contains("javascript")
            || mime.contains("typescript")
            || mime.contains("rust")
            || mime.contains("json")
        {
            return IconName::File;
        }
    }
    IconName::File
}

fn extract_text_from_content(content: &ToolCallContent) -> Option<String> {
    match content {
        ToolCallContent::Content(c) => match &c.content {
            ContentBlock::Text(text) => Some(text.text.clone()),
            _ => None,
        },
        ToolCallContent::Diff(diff) => Some(format!(
            "Modified: {:?}\n{} -> {}",
            diff.path,
            diff.old_text.as_deref().unwrap_or("<new file>"),
            diff.new_text
        )),
        ToolCallContent::Terminal(terminal) => Some(format!("Terminal: {}", terminal.terminal_id)),
        _ => None,
    }
}

// ============================================================================
// Resource Info Structure
// ============================================================================

#[derive(Clone)]
struct ResourceInfo {
    uri: SharedString,
    name: SharedString,
    mime_type: Option<SharedString>,
    text: Option<SharedString>,
}

impl ResourceInfo {
    fn from_content_block(content: &ContentBlock) -> Option<Self> {
        match content {
            ContentBlock::ResourceLink(link) => Some(ResourceInfo {
                uri: link.uri.clone().into(),
                name: link.name.clone().into(),
                mime_type: link.mime_type.clone().map(|s| s.into()),
                text: None,
            }),
            ContentBlock::Resource(embedded) => match &embedded.resource {
                EmbeddedResourceResource::TextResourceContents(text_res) => Some(ResourceInfo {
                    uri: text_res.uri.clone().into(),
                    name: extract_filename(&text_res.uri).into(),
                    mime_type: text_res.mime_type.clone().map(|s| s.into()),
                    text: Some(text_res.text.clone().into()),
                }),
                EmbeddedResourceResource::BlobResourceContents(blob_res) => Some(ResourceInfo {
                    uri: blob_res.uri.clone().into(),
                    name: extract_filename(&blob_res.uri).into(),
                    mime_type: blob_res.mime_type.clone().map(|s| s.into()),
                    text: None,
                }),
                _ => None,
            },
            _ => None,
        }
    }
}

// ============================================================================
// Stateful Resource Item
// ============================================================================

struct ResourceItemState {
    resource: ResourceInfo,
    open: bool,
}

impl ResourceItemState {
    fn new(resource: ResourceInfo) -> Self {
        tracing::info!("📦 Creating ResourceItemState for: {}", resource.name);
        Self {
            resource,
            open: false,
        }
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        tracing::info!(
            "🔄 ResourceItem toggle: {} -> {}",
            self.resource.name,
            self.open
        );
        cx.notify();
    }
}

impl Render for ResourceItemState {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let line_count = self
            .resource
            .text
            .as_ref()
            .map(|t| t.lines().count())
            .unwrap_or(0);

        let is_open = self.open;
        let has_content = self.resource.text.is_some();
        let resource_name = self.resource.name.clone();
        let mime_type = self.resource.mime_type.clone();

        tracing::debug!(
            "🎨 Rendering ResourceItem: {} (open: {})",
            resource_name,
            is_open
        );

        Collapsible::new()
            .open(is_open)
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().muted)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        Icon::new(get_file_icon(&mime_type.map(|s| s.to_string())))
                            .size(px(16.))
                            .text_color(cx.theme().accent),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .child(resource_name.clone()),
                    )
                    .when(line_count > 0, |this| {
                        this.child(
                            div()
                                .text_size(px(11.))
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{} lines", line_count)),
                        )
                    })
                    .when(has_content, |this| {
                        this.child(
                            Button::new(SharedString::from(format!(
                                "resource-toggle-{}",
                                resource_name
                            )))
                            .icon(if is_open {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(
                                |this, _ev, _window, cx| {
                                    tracing::info!(
                                        "🖱️ ResourceItem button clicked: {}",
                                        this.resource.name
                                    );
                                    this.toggle(cx);
                                },
                            )),
                        )
                    }),
            )
            .when(has_content, |this| {
                this.content(
                    div()
                        .w_full()
                        .p_3()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().secondary)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_family("Monaco, 'Courier New', monospace")
                                .text_color(cx.theme().foreground)
                                .line_height(px(18.))
                                .child(self.resource.text.clone().unwrap_or_default()),
                        ),
                )
            })
    }
}

// ============================================================================
// Stateful Tool Call Item
// ============================================================================

struct ToolCallItemState {
    tool_call: ToolCall,
    open: bool,
}

impl ToolCallItemState {
    fn new(tool_call: ToolCall, open: bool) -> Self {
        tracing::info!(
            "🔧 Creating ToolCallItemState: {} (open: {})",
            tool_call.title,
            open
        );
        Self { tool_call, open }
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        tracing::info!(
            "🔄 ToolCallItem toggle: {} -> {}",
            self.tool_call.title,
            self.open
        );
        cx.notify();
    }

    fn has_content(&self) -> bool {
        !self.tool_call.content.is_empty()
    }
}

impl Render for ToolCallItemState {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_content = self.has_content();
        let status_color = match self.tool_call.status {
            ToolCallStatus::Completed => cx.theme().green,
            ToolCallStatus::Failed => cx.theme().red,
            ToolCallStatus::InProgress => cx.theme().accent,
            ToolCallStatus::Pending | _ => cx.theme().muted_foreground,
        };

        let open = self.open;
        let tool_call_id = self.tool_call.tool_call_id.clone();
        let title = self.tool_call.title.clone();
        let kind_icon = self.tool_call.kind.icon();
        let status_icon = self.tool_call.status.icon();

        tracing::debug!("🎨 Rendering ToolCallItem: {} (open: {})", title, open);

        Collapsible::new()
            .open(open)
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().secondary)
                    .child(
                        Icon::new(kind_icon)
                            .size(px(16.))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .text_color(cx.theme().foreground)
                            .child(title),
                    )
                    .child(
                        Icon::new(status_icon)
                            .size(px(14.))
                            .text_color(status_color),
                    )
                    .when(has_content, |this| {
                        this.child(
                            Button::new(SharedString::from(format!(
                                "tool-call-{}-toggle",
                                tool_call_id
                            )))
                            .icon(if open {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(
                                |this, _ev, _window, cx| {
                                    tracing::info!(
                                        "🖱️ ToolCallItem button clicked: {}",
                                        this.tool_call.title
                                    );
                                    this.toggle(cx);
                                },
                            )),
                        )
                    }),
            )
            .when(has_content, |this| {
                this.content(v_flex().gap_1().p_3().pl_8().children(
                    self.tool_call.content.iter().filter_map(|content| {
                        extract_text_from_content(content).map(|text| {
                            div()
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .line_height(px(18.))
                                .child(text)
                        })
                    }),
                ))
            })
    }
}

// ============================================================================
// User Message View
// ============================================================================

struct UserMessageView {
    data: Entity<UserMessageData>,
    resource_items: Vec<Entity<ResourceItemState>>,
}

impl Render for UserMessageView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx).clone();
        let mut resource_index = 0;

        v_flex()
            .gap_3()
            .w_full()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::User)
                            .size(px(16.))
                            .text_color(cx.theme().accent),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("You"),
                    ),
            )
            .child(
                v_flex()
                    .gap_3()
                    .pl_6()
                    .w_full()
                    .children(data.contents.into_iter().filter_map(|content| {
                        match &content {
                            ContentBlock::Text(text_content) => Some(
                                div()
                                    .text_size(px(14.))
                                    .text_color(cx.theme().foreground)
                                    .line_height(px(22.))
                                    .child(text_content.text.clone())
                                    .into_any_element(),
                            ),
                            ContentBlock::ResourceLink(_) | ContentBlock::Resource(_) => {
                                if ResourceInfo::from_content_block(&content).is_some() {
                                    let current_index = resource_index;
                                    resource_index += 1;

                                    if let Some(item) = self.resource_items.get(current_index) {
                                        Some(item.clone().into_any_element())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        }
                    })),
            )
    }
}

// ============================================================================
// Rendered Item
// ============================================================================

enum RenderedItem {
    UserMessage(Entity<UserMessageView>),
    AgentMessage(String, AgentMessageData),
    Plan(Plan),
    ToolCallGroup(Vec<Entity<ToolCallItemState>>),
}

// ============================================================================
// Conversation Panel
// ============================================================================

pub struct ConversationPanel {
    focus_handle: FocusHandle,
    rendered_items: Vec<RenderedItem>,
}

impl crate::panels::dock_panel::DockPanel for ConversationPanel {
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
        tracing::info!("🚀 Initializing ConversationPanel");

        let json_content = include_str!("../fixtures/mock_conversation.json");
        let items: Vec<ConversationItem> =
            serde_json::from_str(json_content).expect("Failed to parse mock conversation");

        let mut rendered_items = Vec::new();

        for item in items.iter() {
            match item {
                ConversationItem::UserMessage { id, data } => {
                    tracing::info!("👤 Creating UserMessage entity: {}", id);
                    let entity = Self::create_user_message(data.clone(), cx);
                    rendered_items.push(RenderedItem::UserMessage(entity));
                }
                ConversationItem::AgentMessage { id, data } => {
                    tracing::info!("🤖 Storing AgentMessage data: {}", id);
                    let agent_data = Self::create_agent_message_data(data.clone());
                    rendered_items.push(RenderedItem::AgentMessage(id.clone(), agent_data));
                }
                ConversationItem::Plan(plan_schema) => {
                    tracing::info!("📋 Storing Plan data");
                    let plan = Self::create_plan(plan_schema.clone());
                    rendered_items.push(RenderedItem::Plan(plan));
                }
                ConversationItem::ToolCallGroup { items: tool_items } => {
                    tracing::info!("🔧 Creating ToolCallGroup with {} items", tool_items.len());
                    let tool_entities: Vec<Entity<ToolCallItemState>> = tool_items
                        .iter()
                        .map(|tool_item| Self::create_tool_call(tool_item.clone(), cx))
                        .collect();
                    rendered_items.push(RenderedItem::ToolCallGroup(tool_entities));
                }
            }
        }

        tracing::info!(
            "✅ ConversationPanel initialized with {} items",
            rendered_items.len()
        );

        Self {
            focus_handle: cx.focus_handle(),
            rendered_items,
        }
    }

    fn get_id(id: &str) -> ElementId {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        ElementId::from(("item", hasher.finish()))
    }

    fn create_user_message(data: UserMessageDataSchema, cx: &mut App) -> Entity<UserMessageView> {
        let mut user_data = UserMessageData::new(data.session_id);

        for content_schema in data.prompt {
            let content_block = Self::map_content_block(content_schema);
            user_data.contents.push(content_block);
        }

        cx.new(|cx| {
            let data_entity = cx.new(|_| user_data.clone());

            let resource_items: Vec<Entity<ResourceItemState>> = user_data
                .contents
                .iter()
                .filter_map(|content| ResourceInfo::from_content_block(content))
                .map(|resource_info| cx.new(|_| ResourceItemState::new(resource_info)))
                .collect();

            tracing::info!("  └─ Created {} resource items", resource_items.len());

            UserMessageView {
                data: data_entity,
                resource_items,
            }
        })
    }

    fn create_tool_call(item: ToolCallItemSchema, cx: &mut App) -> Entity<ToolCallItemState> {
        let kind = item
            .data
            .kind
            .as_deref()
            .map(|k| match k.to_lowercase().as_str() {
                "read" => ToolKind::Read,
                "edit" => ToolKind::Edit,
                "delete" => ToolKind::Delete,
                "move" => ToolKind::Move,
                "search" => ToolKind::Search,
                "execute" => ToolKind::Execute,
                "think" => ToolKind::Think,
                "fetch" => ToolKind::Fetch,
                "switch_mode" => ToolKind::SwitchMode,
                _ => ToolKind::Other,
            })
            .unwrap_or(ToolKind::Other);

        let status = item
            .data
            .status
            .as_deref()
            .map(|s| match s.to_lowercase().as_str() {
                "pending" => ToolCallStatus::Pending,
                "in_progress" | "inprogress" => ToolCallStatus::InProgress,
                "completed" => ToolCallStatus::Completed,
                "failed" => ToolCallStatus::Failed,
                _ => ToolCallStatus::Pending,
            })
            .unwrap_or(ToolCallStatus::Pending);

        let content: Vec<ToolCallContent> = item
            .data
            .content
            .into_iter()
            .map(|c| ToolCallContent::Content(Content::new(ContentBlock::from(c.text))))
            .collect();

        let tool_call = ToolCall::new(ToolCallId::new(item.data.tool_call_id), item.data.title)
            .kind(kind)
            .status(status)
            .content(content);

        let is_open = item.open;
        cx.new(|_| ToolCallItemState::new(tool_call, is_open))
    }

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

    fn create_agent_message_data(data: AgentMessageDataSchema) -> AgentMessageData {
        let mut agent_data = AgentMessageData::new(data.session_id);

        if let Some(meta) = data.meta {
            agent_data.meta = AgentMessageMeta {
                agent_name: meta.agent_name,
                is_complete: meta.is_complete,
            };
        }

        for chunk_schema in data.chunks {
            let content_block = Self::map_content_block(chunk_schema.content);
            let mut content_chunk = ContentChunk::new(content_block);
            if let Some(meta) = chunk_schema.meta {
                content_chunk = content_chunk.meta(meta);
            }
            agent_data.chunks.push(content_chunk);
        }

        agent_data
    }

    fn create_plan(plan_schema: PlanSchema) -> Plan {
        let plan_entries: Vec<PlanEntry> = plan_schema
            .entries
            .into_iter()
            .map(|e| Self::map_plan_entry(e))
            .collect();

        let mut plan = Plan::new(plan_entries);

        if let Some(meta) = plan_schema.meta {
            plan.meta = Some(meta);
        }

        plan
    }

    fn map_plan_entry(entry: PlanEntrySchema) -> PlanEntry {
        let priority = match entry.priority.to_lowercase().as_str() {
            "high" => PlanEntryPriority::High,
            "medium" => PlanEntryPriority::Medium,
            "low" => PlanEntryPriority::Low,
            _ => PlanEntryPriority::Medium,
        };
        let status = match entry.status.to_lowercase().as_str() {
            "pending" => PlanEntryStatus::Pending,
            "in_progress" => PlanEntryStatus::InProgress,
            "completed" => PlanEntryStatus::Completed,
            _ => PlanEntryStatus::Pending,
        };

        let mut plan_entry = PlanEntry::new(entry.content, priority, status);
        if let Some(meta) = entry.meta {
            plan_entry = plan_entry.meta(meta);
        }
        plan_entry
    }
}

impl Focusable for ConversationPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ConversationPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        tracing::debug!("🎨 Rendering ConversationPanel");

        let mut children = v_flex().p_4().gap_6().bg(cx.theme().background);

        for item in &self.rendered_items {
            match item {
                RenderedItem::UserMessage(entity) => {
                    children = children.child(entity.clone());
                }
                RenderedItem::AgentMessage(id, data) => {
                    let msg = AgentMessage::new(Self::get_id(id), data.clone());
                    children = children.child(msg);
                }
                RenderedItem::Plan(plan) => {
                    let todo_list = AgentTodoList::from_plan(plan.clone());
                    children = children.child(v_flex().pl_6().child(todo_list));
                }
                RenderedItem::ToolCallGroup(entities) => {
                    let mut group = v_flex().pl_6().gap_2();
                    for entity in entities {
                        group = group.child(entity.clone());
                    }
                    children = children.child(group);
                }
            }
        }

        children.size_full()
    }
}
