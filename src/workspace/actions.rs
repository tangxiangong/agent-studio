use gpui::*;
use gpui_component::dock::{DockItem, DockPlacement};
use std::sync::Arc;

use crate::{
    panels::dock_panel::DockPanelContainer, utils, AddPanel, AppState, ConversationPanelAcp,
    CreateTaskFromWelcome, NewSessionConversationPanel, ShowConversationPanel, ShowWelcomePanel,
    ToggleDockToggleButton, TogglePanelVisible, WelcomePanel,
};

use super::DockWorkspace;
//   - on_action_add_panel - 添加面板到 dock 区域
//   - on_action_toggle_panel_visible - 切换面板可见性
//   - on_action_toggle_dock_toggle_button - 切换 dock 按钮显示
//   - on_action_open - 打开文件夹选择器
//   - on_action_show_welcome_panel - 显示欢迎面板
//   - on_action_show_conversation_panel - 显示对话面板
//   - on_action_create_task_from_welcome - 从欢迎面板创建任务

impl DockWorkspace {
    /// Helper method to create and add a new ConversationPanelAcp to the center
    pub fn add_conversation_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = Arc::new(DockPanelContainer::panel::<ConversationPanelAcp>(
            window, cx,
        ));

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.add_panel(panel, DockPlacement::Center, None, window, cx);
        });
    }

    /// Helper method to show ConversationPanelAcp in the current active tab
    /// This will add the panel to the current TabPanel instead of replacing the entire center
    fn show_conversation_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _conversation_panel = Arc::new(DockPanelContainer::panel::<ConversationPanelAcp>(
            window, cx,
        ));
        let conversation_panel = DockPanelContainer::panel::<ConversationPanelAcp>(window, cx);
        let conversation_item =
            DockItem::tab(conversation_panel, &self.dock_area.downgrade(), window, cx);
        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.set_center(conversation_item, window, cx);
            // Add to current center TabPanel, similar to clicking a file in an editor
            // dock_area.add_panel(conversation_panel, DockPlacement::Center, None, window, cx);
        });
    }
    /// Handle AddPanel action - randomly add a conversation panel to specified dock area
    pub(super) fn on_action_add_panel(
        &mut self,
        action: &AddPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Random pick up a panel to add
        let panel = match rand::random::<usize>() % 2 {
            0 => Arc::new(DockPanelContainer::panel::<ConversationPanelAcp>(
                window, cx,
            )),
            1 => Arc::new(DockPanelContainer::panel::<ConversationPanelAcp>(
                window, cx,
            )),
            _ => Arc::new(DockPanelContainer::panel::<ConversationPanelAcp>(
                window, cx,
            )),
        };

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.add_panel(panel, action.0, None, window, cx);
        });
    }

    /// Handle TogglePanelVisible action - show/hide panels in the UI
    pub(super) fn on_action_toggle_panel_visible(
        &mut self,
        action: &TogglePanelVisible,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panel_name = action.0.clone();
        let invisible_panels = AppState::global(cx).invisible_panels.clone();
        invisible_panels.update(cx, |names, cx| {
            if names.contains(&panel_name) {
                names.retain(|id| id != &panel_name);
            } else {
                names.push(panel_name);
            }
            cx.notify();
        });
        cx.notify();
    }

    /// Handle ToggleDockToggleButton action - show/hide dock toggle buttons
    pub(super) fn on_action_toggle_dock_toggle_button(
        &mut self,
        _: &ToggleDockToggleButton,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_button_visible = !self.toggle_button_visible;

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.set_toggle_button_visible(self.toggle_button_visible, cx);
        });
    }

    /// Handle Open action - open folder picker and print selected path
    pub(super) fn on_action_open(
        &mut self,
        _: &crate::Open,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |_this, _cx| {
            utils::pick_and_log_folder("Open Project Folder", "Menu").await;
        })
        .detach();
    }

    /// Handle ShowWelcomePanel action - display welcome panel and collapse docks
    pub(super) fn on_action_show_welcome_panel(
        &mut self,
        _: &ShowWelcomePanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Create WelcomePanel for the center
        let welcome_panel = DockPanelContainer::panel::<WelcomePanel>(window, cx);
        let welcome_item = DockItem::tab(welcome_panel, &self.dock_area.downgrade(), window, cx);

        self.dock_area.update(cx, |dock_area, cx| {
            // Replace center with WelcomePanel
            dock_area.set_center(welcome_item, window, cx);

            // Collapse right and bottom docks if they are open
            if dock_area.is_dock_open(DockPlacement::Right, cx) {
                dock_area.toggle_dock(DockPlacement::Right, window, cx);
            }
            if dock_area.is_dock_open(DockPlacement::Bottom, cx) {
                dock_area.toggle_dock(DockPlacement::Bottom, window, cx);
            }
        });
    }

    /// Handle ShowConversationPanel action - display conversation panel
    pub(super) fn on_action_show_conversation_panel(
        &mut self,
        _: &ShowConversationPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_conversation_panel(window, cx);
    }

    /// Handle NewSessionConversationPanel action - add a new conversation panel
    pub(super) fn on_action_new_session_conversation_panel(
        &mut self,
        _action: &NewSessionConversationPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        log::info!("Adding new session conversation panel");
        self.add_conversation_panel(window, cx);
    }

    /// Handle CreateTaskFromWelcome action - create a new agent task from welcome panel
    /// Uses MessageService to handle session creation, event publishing, and prompt sending
    pub(super) fn on_action_create_task_from_welcome(
        &mut self,
        action: &CreateTaskFromWelcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let agent_name = action.agent_name.clone();
        let task_input = action.task_input.clone();
        let mode = action.mode.clone();

        log::info!(
            "Creating task from welcome: agent={}, mode={}, input={}",
            agent_name,
            mode,
            task_input
        );

        // Check for existing welcome session (created by WelcomePanel)
        let welcome_session = AppState::global(cx).welcome_session().cloned();

        // Get services
        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("AgentService not initialized");
                return;
            }
        };

        let message_service = match AppState::global(cx).message_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("MessageService not initialized");
                return;
            }
        };

        let dock_area = self.dock_area.clone();

        cx.spawn_in(window, async move |_this, window| {
            // Step 1: Get or reuse session
            // IMPORTANT: Reuse welcome_session if it exists (created by WelcomePanel)
            // This ensures we use the same agent process that's already running
            let session_id = if let Some(ws) = welcome_session {
                log::info!(
                    "Reusing welcome session {} for agent {}",
                    ws.session_id,
                    ws.agent_name
                );
                ws.session_id
            } else {
                // No welcome session, create new one
                match agent_service.get_or_create_session(&agent_name).await {
                    Ok(session_id) => {
                        log::info!("Created new session {} for agent {}", session_id, agent_name);
                        session_id
                    }
                    Err(e) => {
                        log::error!("Failed to get/create session: {}", e);
                        return;
                    }
                }
            };

            // Step 2: Clear welcome session and create ConversationPanel
            // Panel will subscribe to session updates BEFORE we send the message
            let session_id_for_send = session_id.clone();
            _ = window.update(move |window, cx| {
                // Clear welcome session
                AppState::global_mut(cx).clear_welcome_session();

                // Create panel - this subscribes to the session
                let conversation_panel =
                    DockPanelContainer::panel_for_session(session_id, window, cx);

                let conversation_item =
                    DockItem::tab(conversation_panel, &dock_area.downgrade(), window, cx);

                dock_area.update(cx, |dock_area, cx| {
                    dock_area.set_center(conversation_item, window, cx);

                    // Collapse right and bottom docks
                    if dock_area.is_dock_open(DockPlacement::Right, cx) {
                        dock_area.toggle_dock(DockPlacement::Right, window, cx);
                    }
                    if dock_area.is_dock_open(DockPlacement::Bottom, cx) {
                        dock_area.toggle_dock(DockPlacement::Bottom, window, cx);
                    }
                });
            });

            // Step 3: Now send the message - panel is subscribed and will receive it
            match message_service
                .send_message_to_session(&agent_name, &session_id_for_send, task_input)
                .await
            {
                Ok(_) => {
                    log::info!(
                        "Message sent successfully to session {}",
                        session_id_for_send
                    );
                }
                Err(e) => {
                    log::error!("Failed to send message: {}", e);
                }
            }
        })
        .detach();
    }
}
