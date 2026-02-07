use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex, v_flex,
};

/// Agent thought item for streaming "thinking" output.
pub struct AgentThoughtItem {
    text: String,
    open: bool,
}

impl AgentThoughtItem {
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

    /// Toggle open/close state
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }
}

impl Render for AgentThoughtItem {
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
                                                this.toggle(cx);
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
