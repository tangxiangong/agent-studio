use gpui::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder as _, px,
};

use agent_client_protocol::{
    self as acp, ToolCall, ToolCallContent, ToolCallId, ToolCallStatus, ToolCallUpdateFields,
    ToolKind,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex, v_flex,
};
use similar::{ChangeTag, TextDiff};

use crate::PanelAction;
use crate::components::DiffView;
use crate::panels::conversation::types::{ToolCallStatusExt, ToolKindExt};
use crate::utils::tool_call::{extract_terminal_output, extract_xml_content, truncate_lines};

/// Diff statistics
#[derive(Debug, Clone, Default)]
struct DiffStats {
    additions: usize,
    deletions: usize,
}

/// Calculate diff statistics from old and new text
fn calculate_diff_stats(old_text: &str, new_text: &str) -> DiffStats {
    let diff = TextDiff::from_lines(old_text, new_text);
    let mut additions = 0;
    let mut deletions = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => additions += 1,
            ChangeTag::Delete => deletions += 1,
            ChangeTag::Equal => {}
        }
    }

    DiffStats {
        additions,
        deletions,
    }
}

/// Extract diff statistics from tool call content
fn extract_diff_stats_from_tool_call(tool_call: &ToolCall) -> Option<DiffStats> {
    // Find the first Diff content in the tool call
    for content in &tool_call.content {
        if let ToolCallContent::Diff(diff) = content {
            return Some(match &diff.old_text {
                Some(old_text) => calculate_diff_stats(old_text, &diff.new_text),
                None => {
                    // New file - all lines are additions
                    DiffStats {
                        additions: diff.new_text.lines().count(),
                        deletions: 0,
                    }
                }
            });
        }
    }
    None
}

/// Tool call item component based on ACP's ToolCall - stateful version
pub struct ToolCallItem {
    tool_call: ToolCall,
    open: bool,
}

impl ToolCallItem {
    pub fn new(tool_call: ToolCall) -> Self {
        Self {
            tool_call,
            open: false,
        }
    }

    pub fn new_with_open(tool_call: ToolCall, open: bool) -> Self {
        Self { tool_call, open }
    }

    pub fn tool_call(&self) -> &ToolCall {
        &self.tool_call
    }

    pub fn tool_call_id(&self) -> &ToolCallId {
        &self.tool_call.tool_call_id
    }

    /// Toggle the open state
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }

    /// Set the open state
    pub fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.open = open;
        cx.notify();
    }

    /// Update the tool call data
    pub fn update_tool_call(&mut self, tool_call: ToolCall, cx: &mut Context<Self>) {
        log::debug!("tool_call: {:?}", &tool_call);
        self.tool_call = tool_call;
        if self.has_content() {
            self.open = true;
        }
        cx.notify();
    }

    /// Update this tool call with fields from a ToolCallUpdate
    pub fn apply_update(&mut self, update_fields: ToolCallUpdateFields, cx: &mut Context<Self>) {
        log::debug!("Applying update to tool call: {:?}", update_fields);
        self.tool_call.update(update_fields);

        // Auto-open when tool call completes or fails (so user can see result)
        match self.tool_call.status {
            ToolCallStatus::Completed | ToolCallStatus::Failed => {
                if self.has_content() {
                    self.open = true;
                }
            }
            _ => {}
        }

        cx.notify();
    }

    /// Update the status
    pub fn update_status(&mut self, status: ToolCallStatus, cx: &mut Context<Self>) {
        self.tool_call.status = status;
        cx.notify();
    }

    /// Add content to the tool call
    pub fn add_content(&mut self, content: ToolCallContent, cx: &mut Context<Self>) {
        self.tool_call.content.push(content);
        cx.notify();
    }

    pub fn has_content(&self) -> bool {
        !self.tool_call.content.is_empty()
    }

    /// Get formatted display title for the tool call
    /// For Read tools, formats as: filename#L<offset>-<offset+limit>
    /// For other tools, returns the original title
    fn get_display_title(&self) -> String {
        if !matches!(self.tool_call.kind, ToolKind::Read) {
            return self.tool_call.title.clone();
        }

        if let Some(first_location) = self.tool_call.locations.first() {
            let filename = first_location
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("file");

            if let Some(raw_input) = self.tool_call.raw_input.as_ref() {
                if let Some(raw_obj) = raw_input.as_object() {
                    let offset = raw_obj.get("offset").and_then(|v| v.as_i64()).unwrap_or(1);
                    let limit = raw_obj.get("limit").and_then(|v| v.as_i64()).unwrap_or(100);

                    let end_line = offset + limit - 1;
                    return format!("Read ({}#L{}-L{})", filename, offset, end_line);
                }
            }

            return filename.to_string();
        }

        self.tool_call.title.clone()
    }

    /// Render content based on type
    fn render_content(
        &self,
        content: &ToolCallContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match content {
            ToolCallContent::Diff(diff) => {
                // Use DiffView component for diff content, limited to 10 lines
                let diff_view = DiffView::new(diff.clone())
                    .max_lines(8)
                    .context_lines(1)
                    .show_file_header(false); // Hide file header in compact view

                diff_view.render(window, cx).into_any_element()
            }
            ToolCallContent::Content(c) => match &c.content {
                acp::ContentBlock::Text(text) => {
                    let cleaned_text = extract_xml_content(&text.text, &self.tool_call.kind);
                    let display_text = if cleaned_text.lines().count() > 20 {
                        let max_lines = crate::AppState::global(cx).tool_call_preview_max_lines();
                        truncate_lines(&cleaned_text, max_lines)
                    } else {
                        cleaned_text
                    };
                    div()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .line_height(px(18.))
                        .child(display_text)
                        .into_any_element()
                }
                _ => div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child("Unsupported content type")
                    .into_any_element(),
            },
            ToolCallContent::Terminal(terminal) => {
                let max_lines = crate::AppState::global(cx).tool_call_preview_max_lines();
                let output = extract_terminal_output(terminal).and_then(|text| {
                    if text.trim().is_empty() {
                        None
                    } else {
                        Some(text)
                    }
                });
                let display_text = match output {
                    Some(text) => {
                        let truncated = truncate_lines(&text, max_lines);
                        format!("Terminal: {}\n{}", terminal.terminal_id, truncated)
                    }
                    None => format!("Terminal: {}", terminal.terminal_id),
                };
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .line_height(px(18.))
                    .child(display_text)
                    .into_any_element()
            }
            _ => div()
                .text_size(px(12.))
                .text_color(cx.theme().muted_foreground)
                .child("Unknown content type")
                .into_any_element(),
        }
    }
}

impl Render for ToolCallItem {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_content = self.has_content();
        let status_color = match self.tool_call.status {
            ToolCallStatus::Completed => cx.theme().green,
            ToolCallStatus::Failed => cx.theme().red,
            ToolCallStatus::InProgress => cx.theme().accent,
            ToolCallStatus::Pending | _ => cx.theme().muted_foreground,
        };

        let open = self.open;
        let tool_call_id = self.tool_call.tool_call_id.to_string();
        let title = self.get_display_title();
        let kind_icon = self.tool_call.kind.icon();
        let status_icon = self.tool_call.status.icon();

        // Extract diff stats if this is a diff tool call
        let diff_stats = extract_diff_stats_from_tool_call(&self.tool_call);

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
                        kind_icon
                            .size(px(16.))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_size(px(13.))
                            .text_color(cx.theme().foreground)
                            .line_height(px(18.))
                            .whitespace_normal()
                            .child(title),
                    )
                    // Show diff stats if available
                    .when_some(diff_stats, |this, stats| {
                        this.child(
                            h_flex()
                                .gap_1()
                                .items_center()
                                .child(
                                    // Additions
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(cx.theme().green)
                                        .child(format!("+{}", stats.additions)),
                                )
                                .child(
                                    // Deletions
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(cx.theme().red)
                                        .child(format!("-{}", stats.deletions)),
                                ),
                        )
                    })
                    .child(status_icon.size(px(14.)).text_color(status_color))
                    .when(has_content, |this| {
                        let tool_call_clone_for_detail = self.tool_call.clone();
                        this.child(
                            h_flex()
                                .gap_2()
                                .child(
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
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.toggle(cx);
                                    })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "tool-call-{}-detail",
                                        tool_call_id
                                    )))
                                    .icon(IconName::Info)
                                    .ghost()
                                    .xsmall()
                                    .on_click(cx.listener(move |_, _ev, window, cx| {
                                        let action = PanelAction::show_tool_call_detail(
                                            tool_call_id.clone(),
                                            tool_call_clone_for_detail.clone(),
                                        );
                                        log::debug!("Dispatching PanelAction from ToolCallItem");
                                        window.dispatch_action(Box::new(action), cx);
                                    })),
                                ),
                        )
                    }),
            )
            // Content - only visible when open and has content
            .when(has_content, |this| {
                this.content(
                    v_flex().gap_2().pl_8().children(
                        self.tool_call
                            .content
                            .iter()
                            .map(|content| self.render_content(content, window, cx)),
                    ),
                )
                .max_h(px(300.))
                .overflow_hidden()
            })
    }
}

/// A stateful wrapper for ToolCallItem that can be used as a GPUI view
pub struct ToolCallItemView {
    item: Entity<ToolCallItem>,
}

impl ToolCallItemView {
    pub fn new(tool_call: ToolCall, _window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let item = cx.new(|_| ToolCallItem::new(tool_call));
            Self { item }
        })
    }

    /// Update the tool call data
    pub fn update_tool_call(&mut self, tool_call: ToolCall, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.update_tool_call(tool_call, cx);
        });
        cx.notify();
    }

    /// Update this tool call with fields from a ToolCallUpdate
    pub fn apply_update(&mut self, update_fields: ToolCallUpdateFields, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.apply_update(update_fields, cx);
        });
        cx.notify();
    }

    /// Update the status
    pub fn update_status(&mut self, status: ToolCallStatus, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.update_status(status, cx);
        });
        cx.notify();
    }

    /// Add content to the tool call
    pub fn add_content(&mut self, content: ToolCallContent, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.add_content(content, cx);
        });
        cx.notify();
    }

    /// Set content for the tool call
    pub fn set_content(&mut self, content: Vec<ToolCallContent>, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.tool_call.content = content;
            cx.notify();
        });
        cx.notify();
    }

    /// Toggle the open state
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.toggle(cx);
        });
        cx.notify();
    }

    /// Set the open state
    pub fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.set_open(open, cx);
        });
        cx.notify();
    }
}

impl Render for ToolCallItemView {
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.item.clone()
    }
}
