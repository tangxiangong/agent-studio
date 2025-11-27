use gpui::{
    div, prelude::FluentBuilder as _, px, App, AppContext, Context, ElementId, Entity, IntoElement,
    ParentElement, Render, RenderOnce, SharedString, Styled, Window,
};

use agent_client_protocol_schema::{
    ContentBlock, EmbeddedResource, EmbeddedResourceResource, ResourceLink, SessionId,
    TextResourceContents,
};
use gpui_component::{collapsible::Collapsible, h_flex, v_flex, ActiveTheme, Icon, IconName};

/// User message data structure based on ACP's PromptRequest format
#[derive(Clone, Debug)]
pub struct UserMessageData {
    /// Session ID
    pub session_id: SessionId,
    /// Message content blocks (following ACP ContentBlock format)
    pub contents: Vec<ContentBlock>,
}

impl UserMessageData {
    pub fn new(session_id: impl Into<SessionId>) -> Self {
        Self {
            session_id: session_id.into(),
            contents: Vec::new(),
        }
    }

    pub fn with_contents(mut self, contents: Vec<ContentBlock>) -> Self {
        self.contents = contents;
        self
    }

    pub fn add_content(mut self, content: ContentBlock) -> Self {
        self.contents.push(content);
        self
    }

    /// Add a text content block
    pub fn add_text(mut self, text: impl Into<String>) -> Self {
        self.contents.push(ContentBlock::from(text.into()));
        self
    }

    /// Add a resource link content block
    pub fn add_resource_link(
        mut self,
        name: impl Into<String>,
        uri: impl Into<String>,
    ) -> Self {
        self.contents
            .push(ContentBlock::ResourceLink(ResourceLink::new(name, uri)));
        self
    }

    /// Add an embedded resource content block
    pub fn add_embedded_resource(
        mut self,
        uri: impl Into<String>,
        text: impl Into<String>,
        mime_type: Option<String>,
    ) -> Self {
        let mut resource = TextResourceContents::new(text, uri);
        if let Some(mt) = mime_type {
            resource = resource.mime_type(mt);
        }
        self.contents.push(ContentBlock::Resource(EmbeddedResource::new(
            EmbeddedResourceResource::TextResourceContents(resource),
        )));
        self
    }
}

/// Helper to extract display information from ContentBlock
fn get_resource_info(content: &ContentBlock) -> Option<ResourceInfo> {
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
                text: None, // Blob content is not displayable as text
            }),
            // Handle future variants
            _ => None,
        },
        _ => None,
    }
}

/// Extract filename from URI
fn extract_filename(uri: &str) -> String {
    uri.split('/').last().unwrap_or("unknown").to_string()
}

/// Resource information for display
struct ResourceInfo {
    uri: SharedString,
    name: SharedString,
    mime_type: Option<SharedString>,
    text: Option<SharedString>,
}

impl ResourceInfo {
    /// Get icon based on MIME type
    fn icon(&self) -> IconName {
        if let Some(ref mime) = self.mime_type {
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
}

/// Resource item component (collapsible)
#[derive(IntoElement)]
struct ResourceItem {
    resource: ResourceInfo,
    open: bool,
}

impl ResourceItem {
    pub fn new(resource: ResourceInfo, open: bool) -> Self {
        Self { resource, open }
    }
}

impl RenderOnce for ResourceItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let line_count = self
            .resource
            .text
            .as_ref()
            .map(|t| t.lines().count())
            .unwrap_or(0);

        Collapsible::new()
            .w_full()
            .gap_2()
            // Header
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
                        Icon::new(self.resource.icon())
                            .size(px(16.))
                            .text_color(cx.theme().accent),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .child(self.resource.name.clone()),
                    )
                    .when(line_count > 0, |this| {
                        this.child(
                            div()
                                .text_size(px(11.))
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{} lines", line_count)),
                        )
                    })
                    .child(
                        Icon::new(if self.open {
                            IconName::ChevronUp
                        } else {
                            IconName::ChevronDown
                        })
                        .size(px(14.))
                        .text_color(cx.theme().muted_foreground),
                    ),
            )
            // Content - code display (only if we have text)
            .when(self.open && self.resource.text.is_some(), |this| {
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

/// User message component
#[derive(IntoElement)]
pub struct UserMessage {
    id: ElementId,
    data: UserMessageData,
    resource_states: Vec<bool>, // Track open/close state for each resource
}

impl UserMessage {
    pub fn new(id: impl Into<ElementId>, data: UserMessageData) -> Self {
        let resource_count = data
            .contents
            .iter()
            .filter(|c| matches!(c, ContentBlock::ResourceLink(_) | ContentBlock::Resource(_)))
            .count();

        Self {
            id: id.into(),
            data,
            resource_states: vec![false; resource_count],
        }
    }

    pub fn with_resource_state(mut self, index: usize, open: bool) -> Self {
        if index < self.resource_states.len() {
            self.resource_states[index] = open;
        }
        self
    }
}

impl RenderOnce for UserMessage {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut resource_index = 0;

        v_flex()
            .gap_3()
            .w_full()
            // User icon and label
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
            // Message content
            .child(
                v_flex()
                    .gap_3()
                    .pl_6()
                    .w_full()
                    .children(self.data.contents.into_iter().filter_map(|content| {
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
                                if let Some(resource_info) = get_resource_info(&content) {
                                    let current_index = resource_index;
                                    resource_index += 1;
                                    let open = self
                                        .resource_states
                                        .get(current_index)
                                        .copied()
                                        .unwrap_or(false);

                                    Some(ResourceItem::new(resource_info, open).into_any_element())
                                } else {
                                    None
                                }
                            }
                            // Skip other content types for now (Image, Audio)
                            _ => None,
                        }
                    })),
            )
    }
}

/// A stateful wrapper for UserMessage that can be used as a GPUI view
pub struct UserMessageView {
    data: Entity<UserMessageData>,
    resource_states: Entity<Vec<bool>>,
}

impl UserMessageView {
    pub fn new(data: UserMessageData, _window: &mut Window, cx: &mut App) -> Entity<Self> {
        let resource_count = data
            .contents
            .iter()
            .filter(|c| matches!(c, ContentBlock::ResourceLink(_) | ContentBlock::Resource(_)))
            .count();

        cx.new(|cx| {
            let data_entity = cx.new(|_| data);
            let states_entity = cx.new(|_| vec![false; resource_count]);

            Self {
                data: data_entity,
                resource_states: states_entity,
            }
        })
    }

    /// Update the message data
    pub fn update_data(&mut self, data: UserMessageData, cx: &mut Context<Self>) {
        let resource_count = data
            .contents
            .iter()
            .filter(|c| matches!(c, ContentBlock::ResourceLink(_) | ContentBlock::Resource(_)))
            .count();

        self.data.update(cx, |d, cx| {
            *d = data;
            cx.notify();
        });

        self.resource_states.update(cx, |states, cx| {
            *states = vec![false; resource_count];
            cx.notify();
        });

        cx.notify();
    }

    /// Add content to the message
    pub fn add_content(&mut self, content: ContentBlock, cx: &mut Context<Self>) {
        let is_resource =
            matches!(content, ContentBlock::ResourceLink(_) | ContentBlock::Resource(_));

        self.data.update(cx, |d, cx| {
            d.contents.push(content);
            cx.notify();
        });

        if is_resource {
            self.resource_states.update(cx, |states, cx| {
                states.push(false);
                cx.notify();
            });
        }

        cx.notify();
    }

    /// Toggle resource open state
    pub fn toggle_resource(&mut self, index: usize, cx: &mut Context<Self>) {
        self.resource_states.update(cx, |states, cx| {
            if let Some(state) = states.get_mut(index) {
                *state = !*state;
                cx.notify();
            }
        });
        cx.notify();
    }

    /// Set resource open state
    pub fn set_resource_open(&mut self, index: usize, open: bool, cx: &mut Context<Self>) {
        self.resource_states.update(cx, |states, cx| {
            if let Some(state) = states.get_mut(index) {
                *state = open;
                cx.notify();
            }
        });
        cx.notify();
    }
}

impl Render for UserMessageView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx).clone();
        let resource_states = self.resource_states.read(cx).clone();

        let mut msg = UserMessage::new("user-message", data);
        for (index, open) in resource_states.iter().enumerate() {
            msg = msg.with_resource_state(index, *open);
        }
        msg
    }
}
