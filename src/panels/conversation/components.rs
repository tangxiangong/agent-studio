/// UI Components for ConversationPanel
use gpui::{
    Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex, v_flex,
};

use agent_client_protocol::ContentBlock;

use super::types::{ResourceInfo, get_file_icon};
use crate::UserMessageData;

// ============================================================================
// Stateful Resource Item
// ============================================================================

pub struct ResourceItemState {
    resource: ResourceInfo,
    open: bool,
}

impl ResourceItemState {
    pub fn new(resource: ResourceInfo) -> Self {
        Self {
            resource,
            open: false,
        }
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
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
// Code Selection Detection
// ============================================================================

/// Parsed code selection info from a formatted text block
struct CodeSelectionChip {
    file_path: String,
    line_range: String,
}

/// Try to parse a text block as a code selection.
///
/// Code selection text blocks follow the format produced by
/// `format_code_selection_as_context()` in session_actions.rs.
fn parse_code_selection_text(text: &str) -> Option<CodeSelectionChip> {
    let trimmed = text.trim();
    if !trimmed.starts_with("```\n// File: ") || !trimmed.ends_with("\n```") {
        return None;
    }

    // Extract the "// File: path (Line range)" line
    let first_line = trimmed
        .strip_prefix("```\n")?
        .lines()
        .next()?;

    // Parse: "// File: /path/to/file.rs (Lines 10-20)" or "// File: /path/to/file.rs (Line 10)"
    let after_prefix = first_line.strip_prefix("// File: ")?;
    let paren_pos = after_prefix.rfind('(')?;
    let file_path = after_prefix[..paren_pos].trim().to_string();
    let line_range = after_prefix[paren_pos + 1..]
        .trim_end_matches(')')
        .trim()
        .to_string();

    // Extract just the filename for display
    let filename = std::path::Path::new(&file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&file_path)
        .to_string();

    // Format line range for chip display
    let display_range = if line_range.starts_with("Line ") {
        line_range.strip_prefix("Line ").unwrap_or(&line_range).to_string()
    } else if line_range.starts_with("Lines ") {
        line_range
            .strip_prefix("Lines ")
            .unwrap_or(&line_range)
            .replace('-', "~")
    } else {
        line_range
    };

    Some(CodeSelectionChip {
        file_path: filename,
        line_range: display_range,
    })
}

// ============================================================================
// Stateful Agent Thought Item
// ============================================================================

pub struct AgentThoughtItemState {
    text: String,
    open: bool,
}

impl AgentThoughtItemState {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            open: false,
        }
    }

    /// Append more text to the thought (for streaming updates)
    pub fn append_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.text.push_str(&text.into());
        cx.notify();
    }
}

impl Render for AgentThoughtItemState {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_content = !self.text.is_empty();

        div().pl_6().child(
            Collapsible::new()
                .open(self.open)
                .w_full()
                .gap_2()
                .child(
                    div()
                        .p_3()
                        .rounded_lg()
                        .bg(cx.theme().muted.opacity(0.3))
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Icon::new(IconName::Bot)
                                        .size(px(14.))
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Thinking..."),
                                )
                                .when(has_content, |this| {
                                    this.child(
                                        Button::new("agent-thought-toggle")
                                            .icon(if self.open {
                                                IconName::ChevronUp
                                            } else {
                                                IconName::ChevronDown
                                            })
                                            .ghost()
                                            .xsmall()
                                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                                this.open = !this.open;
                                                cx.notify();
                                            })),
                                    )
                                }),
                        ),
                )
                .when(has_content, |this| {
                    this.content(
                        div()
                            .mt_2()
                            .p_3()
                            .pl_6()
                            .text_sm()
                            .italic()
                            .text_color(cx.theme().foreground.opacity(0.8))
                            .child(self.text.clone()),
                    )
                }),
        )
    }
}

// ============================================================================
// User Message View
// ============================================================================

pub struct UserMessageView {
    pub data: Entity<UserMessageData>,
    pub resource_items: Vec<Entity<ResourceItemState>>,
}

impl UserMessageView {
    /// Add a content block to this user message (for merging consecutive chunks)
    pub fn add_content(&mut self, content: ContentBlock, cx: &mut Context<Self>) {
        // If it's a resource, create a new ResourceItemState entity
        let is_resource = matches!(
            content,
            ContentBlock::ResourceLink(_) | ContentBlock::Resource(_)
        );

        self.data.update(cx, |d, cx| {
            d.contents.push(content.clone());
            cx.notify();
        });

        if is_resource {
            if let Some(resource_info) = ResourceInfo::from_content_block(&content) {
                let item = cx.new(|_| ResourceItemState::new(resource_info));
                self.resource_items.push(item);
            }
        }

        cx.notify();
    }
}

impl Render for UserMessageView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx).clone();
        let mut resource_index = 0;
        let theme = cx.theme().clone();

        // Separate code selection blocks from other content
        let mut code_chips: Vec<CodeSelectionChip> = Vec::new();
        let mut other_contents: Vec<ContentBlock> = Vec::new();

        for content in data.contents.into_iter() {
            if let ContentBlock::Text(ref text_content) = content {
                if let Some(chip) = parse_code_selection_text(&text_content.text) {
                    code_chips.push(chip);
                    continue;
                }
            }
            other_contents.push(content);
        }

        let has_chips = !code_chips.is_empty();

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
                            .text_color(theme.accent),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child("You"),
                    ),
            )
            .child(
                v_flex()
                    .gap_3()
                    .pl_6()
                    .w_full()
                    // Render text and resource blocks
                    .children(other_contents.into_iter().filter_map(|content| {
                        match &content {
                            ContentBlock::Text(text_content) => Some(
                                div()
                                    .text_size(px(14.))
                                    .text_color(theme.foreground)
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
                    }))
                    // Render code selection chips
                    .when(has_chips, |this| {
                        this.child(
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .flex_wrap()
                                .children(code_chips.into_iter().map(|chip| {
                                    let display_text =
                                        format!("{}:{}", chip.file_path, chip.line_range);

                                    h_flex()
                                        .gap_1()
                                        .items_center()
                                        .py_0p5()
                                        .px_1p5()
                                        .rounded(px(6.))
                                        .bg(theme.primary.opacity(0.1))
                                        .border_1()
                                        .border_color(theme.primary.opacity(0.3))
                                        .child(
                                            Icon::new(IconName::Frame)
                                                .size(px(13.))
                                                .text_color(theme.primary),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.5))
                                                .text_color(theme.foreground.opacity(0.85))
                                                .child(display_text),
                                        )
                                })),
                        )
                    }),
            )
    }
}
