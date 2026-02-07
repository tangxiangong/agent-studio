use std::collections::HashMap;

use agent_client_protocol::{
    ContentBlock, ContentChunk, Plan, PlanEntryStatus, SessionUpdate, ToolCall, ToolCallUpdate,
};
use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*,
};
use gpui_component::{ActiveTheme, v_flex};

use crate::agent_thought::AgentThoughtItem;
use crate::user_message::{ResourceItem, get_resource_info};
use crate::{
    AgentIconProvider, AgentMessage, AgentMessageData, AgentMessageOptions, AgentTodoList,
    DiffSummary, DiffSummaryData, DiffSummaryOptions, PermissionRequestView, ToolCallItem,
    ToolCallItemOptions, UserMessageData, UserMessageView,
};

#[derive(Clone)]
pub struct AcpMessageStreamOptions {
    pub agent_icon_provider: AgentIconProvider,
    pub tool_call_item_options: ToolCallItemOptions,
    pub diff_summary_options: DiffSummaryOptions,
}

impl Default for AcpMessageStreamOptions {
    fn default() -> Self {
        Self {
            agent_icon_provider: AgentMessageOptions::default().icon_provider,
            tool_call_item_options: ToolCallItemOptions::default(),
            diff_summary_options: DiffSummaryOptions::default(),
        }
    }
}

/// Message stream UI for ACP SessionUpdate rendering.
pub struct AcpMessageStream {
    items: Vec<RenderedItem>,
    index: UpdateStateIndex,
    next_index: usize,
    options: AcpMessageStreamOptions,
}

impl AcpMessageStream {
    pub fn new() -> Self {
        Self::with_options(AcpMessageStreamOptions::default())
    }

    pub fn with_options(options: AcpMessageStreamOptions) -> Self {
        Self {
            items: Vec::new(),
            index: UpdateStateIndex::new(),
            next_index: 0,
            options,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Return the current in-progress plan entry (if any).
    pub fn current_todo_in_progress(&self) -> Option<String> {
        self.items.iter().rev().find_map(|item| {
            if let RenderedItem::Plan(plan) = item {
                plan.entries
                    .iter()
                    .find(|entry| entry.status == PlanEntryStatus::InProgress)
                    .map(|entry| entry.content.clone())
            } else {
                None
            }
        })
    }

    /// Process a SessionUpdate and add/update items.
    pub fn process_update(
        &mut self,
        update: SessionUpdate,
        session_id: Option<&str>,
        agent_name: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let mut processor = UpdateProcessor::new(
            &mut self.items,
            &mut self.index,
            session_id,
            agent_name,
            self.next_index,
            &self.options,
        );

        processor.process_update(update, cx);
        self.next_index += 1;
        cx.notify();
    }

    pub fn add_permission_request(
        &mut self,
        request: Entity<PermissionRequestView>,
        cx: &mut Context<Self>,
    ) {
        self.items.push(RenderedItem::PermissionRequest(request));
        cx.notify();
    }

    pub fn add_info_update(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.items.push(RenderedItem::InfoUpdate(text.into()));
        cx.notify();
    }

    pub fn mark_last_complete(&mut self, cx: &mut Context<Self>) {
        if let Some(last_item) = self.items.last_mut() {
            last_item.mark_complete();
            cx.notify();
        }
    }

    /// Add DiffSummary to the message stream if there are any tool calls with diffs.
    pub fn add_diff_summary_if_needed(&mut self, cx: &mut Context<Self>) {
        let tool_calls = self.collect_tool_calls(cx);
        let summary_data = DiffSummaryData::from_tool_calls(&tool_calls);

        if summary_data.has_changes() {
            let options = self.options.diff_summary_options.clone();
            let diff_summary = cx.new(|_| DiffSummary::new(summary_data).with_options(options));
            self.items.push(RenderedItem::DiffSummary(diff_summary));
            cx.notify();
        }
    }

    fn collect_tool_calls(&self, cx: &App) -> Vec<ToolCall> {
        let mut tool_calls = Vec::new();

        for item in &self.items {
            if let RenderedItem::ToolCall(entity) = item {
                let tool_call = entity.read(cx).tool_call().clone();
                tool_calls.push(tool_call);
            }
        }

        tool_calls
    }
}

impl Render for AcpMessageStream {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut children = v_flex().gap_3().w_full();

        for item in &self.items {
            match item {
                RenderedItem::UserMessage(entity) => {
                    children = children.child(entity.clone());
                }
                RenderedItem::AgentMessage(id, data) => {
                    let msg = AgentMessage::new(get_element_id(id), data.clone())
                        .icon_provider(self.options.agent_icon_provider.clone());
                    children = children.child(msg);
                }
                RenderedItem::AgentThought(entity) => {
                    children = children.child(entity.clone());
                }
                RenderedItem::Plan(plan) => {
                    let todo_list = AgentTodoList::from_plan(plan.clone());
                    children = children.child(v_flex().pl_6().child(todo_list));
                }
                RenderedItem::ToolCall(entity) => {
                    children = children.child(v_flex().pl_6().child(entity.clone()));
                }
                RenderedItem::PermissionRequest(entity) => {
                    children = children.child(v_flex().pl_6().child(entity.clone()));
                }
                RenderedItem::DiffSummary(entity) => {
                    children = children.child(entity.clone());
                }
                RenderedItem::InfoUpdate(text) => {
                    children = children.child(
                        div().pl_6().child(
                            div()
                                .p_2()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().muted.opacity(0.5))
                                .border_1()
                                .border_color(cx.theme().border.opacity(0.3))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(text.clone()),
                                ),
                        ),
                    );
                }
            }
        }

        children
    }
}

// ============================================================================
// Rendered Item
// ============================================================================

enum RenderedItem {
    UserMessage(Entity<UserMessageView>),
    /// Agent message with unique ID and mutable data (supports chunk merging)
    AgentMessage(String, AgentMessageData),
    /// Agent thought with entity (supports chunk merging and expand/collapse)
    AgentThought(Entity<AgentThoughtItem>),
    Plan(Plan),
    ToolCall(Entity<ToolCallItem>),
    InfoUpdate(String),
    PermissionRequest(Entity<PermissionRequestView>),
    DiffSummary(Entity<DiffSummary>),
}

impl RenderedItem {
    /// Try to append an AgentMessageChunk to this item (returns true if successful)
    pub fn try_append_agent_message_chunk(&mut self, chunk: ContentChunk) -> bool {
        if let RenderedItem::AgentMessage(_id, data) = self {
            data.push_chunk(chunk);
            true
        } else {
            false
        }
    }

    /// Try to append an AgentThoughtChunk to this item (returns true if successful)
    pub fn try_append_agent_thought_chunk(
        &mut self,
        text: String,
        cx: &mut Context<AcpMessageStream>,
    ) -> bool {
        if let RenderedItem::AgentThought(entity) = self {
            entity.update(cx, |state, cx| {
                state.append_text(text, cx);
            });
            true
        } else {
            false
        }
    }

    /// Mark an AgentMessage as complete (no more chunks expected)
    pub fn mark_complete(&mut self) {
        if let RenderedItem::AgentMessage(_id, data) = self {
            data.meta.is_complete = true;
        }
    }

    pub fn can_accept_agent_message_chunk(&self) -> bool {
        matches!(self, RenderedItem::AgentMessage(..))
    }

    pub fn can_accept_agent_thought_chunk(&self) -> bool {
        matches!(self, RenderedItem::AgentThought(..))
    }

    pub fn can_accept_user_message_chunk(&self) -> bool {
        matches!(self, RenderedItem::UserMessage(..))
    }

    /// Try to append a UserMessageChunk to this item (returns true if successful)
    pub fn try_append_user_message_chunk(
        &mut self,
        chunk: ContentChunk,
        cx: &mut Context<AcpMessageStream>,
    ) -> bool {
        if let RenderedItem::UserMessage(entity) = self {
            entity.update(cx, |view, cx| {
                view.add_content(chunk.content, cx);
            });
            true
        } else {
            false
        }
    }
}

// ============================================================================
// Update State Index
// ============================================================================

#[derive(Default)]
struct UpdateStateIndex {
    tool_call_positions: HashMap<String, usize>,
    last_message_index: Option<usize>,
    last_thought_index: Option<usize>,
    last_user_message_index: Option<usize>,
}

impl UpdateStateIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_tool_call(&mut self, tool_call_id: String, index: usize) {
        self.tool_call_positions.insert(tool_call_id, index);
    }

    pub fn update_tool_call_position(&mut self, tool_call_id: &str, new_index: usize) {
        self.tool_call_positions
            .insert(tool_call_id.to_string(), new_index);
    }

    pub fn find_tool_call(&self, tool_call_id: &str) -> Option<usize> {
        self.tool_call_positions.get(tool_call_id).copied()
    }

    pub fn remove_tool_call(&mut self, tool_call_id: &str) {
        self.tool_call_positions.remove(tool_call_id);
    }

    pub fn set_last_message(&mut self, index: usize) {
        self.last_message_index = Some(index);
    }

    pub fn last_message(&self) -> Option<usize> {
        self.last_message_index
    }

    pub fn set_last_thought(&mut self, index: usize) {
        self.last_thought_index = Some(index);
    }

    pub fn last_thought(&self) -> Option<usize> {
        self.last_thought_index
    }

    pub fn clear_streaming_state(&mut self) {
        self.last_message_index = None;
        self.last_thought_index = None;
    }

    pub fn set_last_user_message(&mut self, index: usize) {
        self.last_user_message_index = Some(index);
    }

    pub fn last_user_message(&self) -> Option<usize> {
        self.last_user_message_index
    }

    pub fn clear_user_message_state(&mut self) {
        self.last_user_message_index = None;
    }

    pub fn rebuild(&mut self, items: &[RenderedItem], cx: &App) {
        self.tool_call_positions.clear();
        self.last_message_index = None;
        self.last_thought_index = None;
        self.last_user_message_index = None;

        for (idx, item) in items.iter().enumerate() {
            match item {
                RenderedItem::ToolCall(entity) => {
                    let tool_call_id = entity.read(cx).tool_call_id().to_string();
                    self.tool_call_positions.insert(tool_call_id, idx);
                }
                RenderedItem::AgentMessage(..) => {
                    self.last_message_index = Some(idx);
                }
                RenderedItem::AgentThought(..) => {
                    self.last_thought_index = Some(idx);
                }
                _ => {}
            }
        }
    }
}

// ============================================================================
// Update Processor
// ============================================================================

struct UpdateProcessor<'a> {
    items: &'a mut Vec<RenderedItem>,
    index: &'a mut UpdateStateIndex,
    session_id: Option<&'a str>,
    agent_name: Option<&'a str>,
    next_index: usize,
    options: &'a AcpMessageStreamOptions,
}

impl<'a> UpdateProcessor<'a> {
    pub fn new(
        items: &'a mut Vec<RenderedItem>,
        index: &'a mut UpdateStateIndex,
        session_id: Option<&'a str>,
        agent_name: Option<&'a str>,
        next_index: usize,
        options: &'a AcpMessageStreamOptions,
    ) -> Self {
        Self {
            items,
            index,
            session_id,
            agent_name,
            next_index,
            options,
        }
    }

    pub fn process_update(&mut self, update: SessionUpdate, cx: &mut Context<AcpMessageStream>) {
        let update_type = session_update_type_name(&update);
        log::debug!(
            "Processing SessionUpdate[{}]: {}",
            self.next_index,
            update_type
        );

        match update {
            SessionUpdate::UserMessageChunk(chunk) => {
                self.process_user_message_chunk(chunk, cx);
            }
            SessionUpdate::AgentMessageChunk(chunk) => {
                self.process_agent_message_chunk(chunk, cx);
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                self.process_agent_thought_chunk(chunk, cx);
            }
            SessionUpdate::ToolCall(tool_call) => {
                self.process_tool_call(tool_call, cx);
            }
            SessionUpdate::ToolCallUpdate(tool_call_update) => {
                self.process_tool_call_update(tool_call_update, cx);
            }
            SessionUpdate::Plan(plan) => {
                self.process_plan(plan);
            }
            SessionUpdate::AvailableCommandsUpdate(commands_update) => {
                self.complete_last_item();
                self.index.clear_streaming_state();
                log::debug!(
                    "  └─ Commands update: {} available",
                    commands_update.available_commands.len()
                );
                self.items.push(RenderedItem::InfoUpdate(format!(
                    "📋 Available Commands: {} commands",
                    commands_update.available_commands.len()
                )));
            }
            SessionUpdate::CurrentModeUpdate(mode_update) => {
                self.complete_last_item();
                self.index.clear_streaming_state();
                log::debug!("  └─ Mode changed to: {}", mode_update.current_mode_id);
                self.items.push(RenderedItem::InfoUpdate(format!(
                    "🔄 Mode: {}",
                    mode_update.current_mode_id
                )));
            }
            _ => {
                log::warn!(
                    "⚠️  UNHANDLED SessionUpdate type: {}\n\
                     This update will be ignored. Consider implementing support for this type.\n\
                     Update details: {:?}",
                    update_type,
                    update
                );
            }
        }
    }

    fn process_user_message_chunk(
        &mut self,
        chunk: ContentChunk,
        cx: &mut Context<AcpMessageStream>,
    ) {
        if let Some(last_idx) = self.index.last_user_message() {
            if last_idx < self.items.len() {
                if let Some(last_item) = self.items.get_mut(last_idx) {
                    if last_item.can_accept_user_message_chunk() {
                        if last_item.try_append_user_message_chunk(chunk.clone(), cx) {
                            log::debug!(
                                "  └─ Merged UserMessageChunk into existing message (fast path)"
                            );
                            return;
                        }
                    }
                }
            }
        }

        if self.index.last_message().is_some() || self.index.last_thought().is_some() {
            self.complete_last_item();
            self.index.clear_streaming_state();
        }

        log::debug!("  └─ Creating UserMessage");
        let item = create_user_message(chunk, self.session_id, cx);
        let new_index = self.items.len();
        self.items.push(item);
        self.index.set_last_user_message(new_index);
    }

    fn process_agent_message_chunk(
        &mut self,
        chunk: ContentChunk,
        cx: &mut Context<AcpMessageStream>,
    ) {
        self.index.clear_user_message_state();
        let resolved_agent_name = self.agent_name.map(str::to_string);

        if let Some(last_idx) = self.index.last_message() {
            if last_idx < self.items.len() {
                if let Some(last_item) = self.items.get_mut(last_idx) {
                    if last_item.can_accept_agent_message_chunk() {
                        if last_item.try_append_agent_message_chunk(chunk.clone()) {
                            if let (Some(name), RenderedItem::AgentMessage(_, data)) =
                                (resolved_agent_name.as_deref(), last_item)
                            {
                                if data.meta.agent_name.is_none() {
                                    data.meta.agent_name = Some(name.to_string());
                                }
                            }
                            log::debug!(
                                "  └─ Merged AgentMessageChunk into existing message (fast path)"
                            );
                            return;
                        }
                    }
                }
            }
        }

        if self.index.last_thought().is_some() {
            self.complete_last_item();
        }

        log::debug!("  └─ Creating new AgentMessage");
        let data =
            create_agent_message_data(chunk, self.session_id, resolved_agent_name.as_deref());
        let new_index = self.items.len();
        self.items.push(RenderedItem::AgentMessage(
            format!("agent-msg-{}", self.next_index),
            data,
        ));
        self.index.set_last_message(new_index);
        self.index.set_last_thought(new_index);
    }

    fn process_agent_thought_chunk(
        &mut self,
        chunk: ContentChunk,
        cx: &mut Context<AcpMessageStream>,
    ) {
        self.index.clear_user_message_state();
        let text = extract_text_from_content(&chunk.content);

        if let Some(last_idx) = self.index.last_thought() {
            if last_idx < self.items.len() {
                if let Some(last_item) = self.items.get_mut(last_idx) {
                    if last_item.can_accept_agent_thought_chunk() {
                        if last_item.try_append_agent_thought_chunk(text.clone(), cx) {
                            log::debug!(
                                "  └─ Merged AgentThoughtChunk into existing thought (fast path)"
                            );
                            return;
                        }
                    }
                }
            }
        }

        if self.index.last_message().is_some() {
            self.complete_last_item();
        }

        log::debug!("  └─ Creating new AgentThought");
        let entity = cx.new(|_| AgentThoughtItem::new(text));
        let new_index = self.items.len();
        self.items.push(RenderedItem::AgentThought(entity));
        self.index.set_last_thought(new_index);
        self.index.set_last_message(new_index);
    }

    fn process_tool_call(&mut self, tool_call: ToolCall, cx: &mut Context<AcpMessageStream>) {
        self.index.clear_user_message_state();
        if let Some(idx) = self
            .index
            .find_tool_call(&tool_call.tool_call_id.to_string())
        {
            if idx < self.items.len() {
                if let Some(RenderedItem::ToolCall(entity)) = self.items.get_mut(idx) {
                    entity.update(cx, |state, cx| {
                        log::debug!(
                            "  └─ Updating existing ToolCall: {} (title: {:?} -> {:?}) [fast O(1) lookup]",
                            tool_call.tool_call_id,
                            state.tool_call().title,
                            tool_call.title
                        );
                        state.update_tool_call(tool_call.clone(), cx);
                    });
                    return;
                }
            }
        }

        self.complete_last_item();
        self.index.clear_streaming_state();

        log::debug!("  └─ Creating new ToolCall: {}", tool_call.tool_call_id);
        let tool_call_id = tool_call.tool_call_id.to_string();
        let options = self.options.tool_call_item_options.clone();
        let entity = cx.new(|_| ToolCallItem::with_options(tool_call, options));
        let new_index = self.items.len();
        self.items.push(RenderedItem::ToolCall(entity));
        self.index.register_tool_call(tool_call_id, new_index);
    }

    fn process_tool_call_update(
        &mut self,
        tool_call_update: ToolCallUpdate,
        cx: &mut Context<AcpMessageStream>,
    ) {
        log::debug!("  └─ Updating ToolCall: {}", tool_call_update.tool_call_id);

        if let Some(idx) = self
            .index
            .find_tool_call(&tool_call_update.tool_call_id.to_string())
        {
            if idx < self.items.len() {
                if let Some(RenderedItem::ToolCall(entity)) = self.items.get_mut(idx) {
                    entity.update(cx, |state, cx| {
                        log::debug!(
                            "     ✓ Found and updating ToolCall {} (status: {:?}) [fast O(1) lookup]",
                            tool_call_update.tool_call_id,
                            tool_call_update.fields.status
                        );
                        state.apply_update(tool_call_update.fields.clone(), cx);
                    });
                    return;
                }
            }
        }

        log::warn!(
            "     ⚠ ToolCallUpdate for non-existent ID: {}. Attempting to create.",
            tool_call_update.tool_call_id
        );

        match ToolCall::try_from(tool_call_update) {
            Ok(tool_call) => {
                log::debug!("     ✓ Successfully created ToolCall from update");
                let tool_call_id = tool_call.tool_call_id.to_string();
                let options = self.options.tool_call_item_options.clone();
                let entity = cx.new(|_| ToolCallItem::with_options(tool_call, options));
                let new_index = self.items.len();
                self.items.push(RenderedItem::ToolCall(entity));
                self.index.register_tool_call(tool_call_id, new_index);
            }
            Err(e) => {
                log::error!("     ✗ Failed to create ToolCall from update: {:?}", e);
            }
        }
    }

    fn process_plan(&mut self, plan: Plan) {
        self.index.clear_user_message_state();
        self.complete_last_item();
        self.index.clear_streaming_state();
        log::debug!("  └─ Creating Plan with {} entries", plan.entries.len());
        self.items.push(RenderedItem::Plan(plan));
    }

    fn complete_last_item(&mut self) {
        if let Some(last_item) = self.items.last_mut() {
            last_item.mark_complete();
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn create_user_message<T>(
    chunk: ContentChunk,
    session_id: Option<&str>,
    cx: &mut Context<T>,
) -> RenderedItem {
    let content_vec = vec![chunk.content.clone()];
    let session_id = session_id
        .map(str::to_string)
        .unwrap_or_else(|| "default-session".to_string());
    let user_data = UserMessageData::new(session_id).with_contents(content_vec.clone());

    let entity = cx.new(|cx| {
        let data_entity = cx.new(|_| user_data);

        let resource_items: Vec<Entity<ResourceItem>> = content_vec
            .iter()
            .filter_map(|content| get_resource_info(content))
            .map(|resource_info| cx.new(|_| ResourceItem::new(resource_info)))
            .collect();

        UserMessageView {
            data: data_entity,
            resource_items,
        }
    });

    RenderedItem::UserMessage(entity)
}

fn create_agent_message_data(
    chunk: ContentChunk,
    session_id: Option<&str>,
    agent_name: Option<&str>,
) -> AgentMessageData {
    let session_id = session_id.unwrap_or("default-session").to_string();
    let mut data = AgentMessageData::new(session_id).add_chunk(chunk);
    if let Some(agent_name) = agent_name {
        data = data.with_agent_name(agent_name);
    }
    data
}

fn extract_text_from_content(content: &ContentBlock) -> String {
    match content {
        ContentBlock::Text(text_content) => text_content.text.clone(),
        ContentBlock::Image(img) => format!("[Image: {}]", img.mime_type),
        ContentBlock::Audio(audio) => format!("[Audio: {}]", audio.mime_type),
        ContentBlock::ResourceLink(link) => format!("[Resource: {}]", link.name),
        ContentBlock::Resource(resource) => match &resource.resource {
            agent_client_protocol::EmbeddedResourceResource::TextResourceContents(text_res) => {
                format!(
                    "[Resource: {}]\n{}",
                    text_res.uri,
                    &text_res.text[..text_res.text.len().min(200)]
                )
            }
            agent_client_protocol::EmbeddedResourceResource::BlobResourceContents(blob_res) => {
                format!("[Binary Resource: {}]", blob_res.uri)
            }
            _ => "[Unknown Resource]".to_string(),
        },
        _ => "[Unknown Content]".to_string(),
    }
}

fn session_update_type_name(update: &SessionUpdate) -> &'static str {
    match update {
        SessionUpdate::UserMessageChunk(_) => "UserMessageChunk",
        SessionUpdate::AgentMessageChunk(_) => "AgentMessageChunk",
        SessionUpdate::AgentThoughtChunk(_) => "AgentThoughtChunk",
        SessionUpdate::ToolCall(_) => "ToolCall",
        SessionUpdate::ToolCallUpdate(_) => "ToolCallUpdate",
        SessionUpdate::Plan(_) => "Plan",
        SessionUpdate::AvailableCommandsUpdate(_) => "AvailableCommandsUpdate",
        SessionUpdate::CurrentModeUpdate(_) => "CurrentModeUpdate",
        _ => "Unknown/Future SessionUpdate Type",
    }
}

fn get_element_id(id: &str) -> gpui::ElementId {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    gpui::ElementId::from(("item", hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_tool_call_operations() {
        let mut index = UpdateStateIndex::new();

        index.register_tool_call("tc-1".to_string(), 0);
        index.register_tool_call("tc-2".to_string(), 5);

        assert_eq!(index.find_tool_call("tc-1"), Some(0));
        assert_eq!(index.find_tool_call("tc-2"), Some(5));
        assert_eq!(index.find_tool_call("tc-3"), None);

        index.update_tool_call_position("tc-1", 10);
        assert_eq!(index.find_tool_call("tc-1"), Some(10));

        index.remove_tool_call("tc-1");
        assert_eq!(index.find_tool_call("tc-1"), None);
    }

    #[test]
    fn test_index_streaming_state() {
        let mut index = UpdateStateIndex::new();

        index.set_last_message(5);
        assert_eq!(index.last_message(), Some(5));

        index.set_last_thought(8);
        assert_eq!(index.last_thought(), Some(8));

        index.clear_streaming_state();
        assert_eq!(index.last_message(), None);
        assert_eq!(index.last_thought(), None);
    }
}
