use anyhow::{Context as _, Result};
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, IconName, Root, Sizable, Size as UiSize, StyledExt as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    dock::{DockArea, DockAreaState, DockEvent, DockItem, DockPlacement},
    h_flex,
    menu::DropdownMenu,
    stepper::{Stepper, StepperItem},
    v_flex,
};
use smol::Timer;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use crate::{
    AppSettings, AppState, AppTitleBar, CodeEditorPanel, ConversationPanel, PanelAction,
    SessionManagerPanel, TaskPanel, TerminalPanel, ToggleDockToggleButton, TogglePanelVisible,
    core::{
        config::{AgentProcessConfig, Config},
        nodejs::NodeJsChecker,
    },
    panels::dock_panel::DockPanelContainer,
    title_bar::OpenSettings,
    utils,
};

// Action handlers module
pub mod actions;

const MAIN_DOCK_AREA: DockAreaTab = DockAreaTab {
    id: "main-dock",
    version: 5,
};

pub struct DockWorkspace {
    title_bar: Entity<AppTitleBar>,
    dock_area: Entity<DockArea>,
    last_layout_state: Option<DockAreaState>,
    toggle_button_visible: bool,
    _save_layout_task: Option<Task<()>>,
    startup_state: StartupState,
}

struct DockAreaTab {
    id: &'static str,
    version: usize,
}

#[derive(Clone, Debug)]
struct AgentChoice {
    name: String,
    enabled: bool,
}

#[derive(Clone, Debug)]
enum NodeJsStatus {
    Idle,
    Checking,
    Available {
        version: Option<String>,
        path: Option<PathBuf>,
    },
    Unavailable {
        message: String,
        hint: Option<String>,
    },
}

#[derive(Debug)]
struct StartupState {
    initialized: bool,
    step: usize,
    nodejs_status: NodeJsStatus,
    nodejs_skipped: bool,
    agent_choices: Vec<AgentChoice>,
    default_agent_configs: HashMap<String, AgentProcessConfig>,
    agent_apply_in_progress: bool,
    agent_apply_error: Option<String>,
    agent_load_error: Option<String>,
    agent_applied: bool,
    agent_synced: bool,
    agent_sync_in_progress: bool,
    workspace_selected: bool,
    workspace_path: Option<PathBuf>,
    workspace_loading: bool,
    workspace_error: Option<String>,
    workspace_checked: bool,
    workspace_check_in_progress: bool,
}

impl StartupState {
    fn new() -> Self {
        let (agent_choices, default_agent_configs, agent_load_error) =
            Self::load_default_agent_configs();
        let agent_applied = agent_choices.is_empty();

        Self {
            initialized: false,
            step: 0,
            nodejs_status: NodeJsStatus::Idle,
            nodejs_skipped: false,
            agent_choices,
            default_agent_configs,
            agent_apply_in_progress: false,
            agent_apply_error: None,
            agent_load_error,
            agent_applied,
            agent_synced: false,
            agent_sync_in_progress: false,
            workspace_selected: false,
            workspace_path: None,
            workspace_loading: false,
            workspace_error: None,
            workspace_checked: false,
            workspace_check_in_progress: false,
        }
    }

    fn nodejs_ready(&self) -> bool {
        self.nodejs_skipped || matches!(self.nodejs_status, NodeJsStatus::Available { .. })
    }

    fn agents_ready(&self) -> bool {
        self.agent_applied || self.agent_choices.is_empty()
    }

    fn workspace_ready(&self) -> bool {
        self.workspace_selected
    }

    fn is_complete(&self) -> bool {
        self.nodejs_ready() && self.agents_ready() && self.workspace_ready()
    }

    fn advance_step_if_needed(&mut self) {
        if self.step == 0 && self.nodejs_ready() {
            self.step = 1;
        }
        if self.step == 1 && self.agents_ready() {
            self.step = 2;
        }
        if self.step > 2 {
            self.step = 2;
        }
    }

    fn load_default_agent_configs() -> (
        Vec<AgentChoice>,
        HashMap<String, AgentProcessConfig>,
        Option<String>,
    ) {
        let raw = match crate::assets::get_default_config() {
            Some(raw) => raw,
            None => {
                return (
                    Vec::new(),
                    HashMap::new(),
                    Some("Embedded config.json not found.".to_string()),
                );
            }
        };

        let config: Config = match serde_json::from_str(&raw) {
            Ok(config) => config,
            Err(err) => {
                log::error!("Failed to parse embedded config.json: {}", err);
                return (
                    Vec::new(),
                    HashMap::new(),
                    Some(format!("Failed to parse embedded config.json: {}", err)),
                );
            }
        };

        let mut agent_entries: Vec<_> = config.agent_servers.into_iter().collect();
        agent_entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut agent_choices = Vec::new();
        let mut default_agent_configs = HashMap::new();

        for (name, config) in agent_entries {
            default_agent_configs.insert(name.clone(), config.clone());
            agent_choices.push(AgentChoice {
                name,
                enabled: true,
            });
        }

        (agent_choices, default_agent_configs, None)
    }
}

impl DockWorkspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dock_area =
            cx.new(|cx| DockArea::new(MAIN_DOCK_AREA.id, Some(MAIN_DOCK_AREA.version), window, cx));
        let weak_dock_area = dock_area.downgrade();

        match Self::load_layout(dock_area.clone(), window, cx) {
            Ok(_) => {
                println!("load layout success");
            }
            Err(err) => {
                eprintln!("load layout error: {:?}", err);
                Self::reset_default_layout(weak_dock_area, window, cx);
            }
        };

        cx.subscribe_in(
            &dock_area,
            window,
            |this, dock_area, ev: &DockEvent, window, cx| match ev {
                DockEvent::LayoutChanged => this.save_layout(dock_area, window, cx),
                _ => {}
            },
        )
        .detach();

        cx.on_app_quit({
            let dock_area = dock_area.clone();
            move |_, cx| {
                let state = dock_area.read(cx).dump(cx);
                cx.background_executor().spawn(async move {
                    // Save layout before quitting
                    if let Err(e) = Self::save_state(&state) {
                        log::warn!("Failed to save layout state: {}", e);
                    }
                })
            }
        })
        .detach();

        cx.on_release(|this, cx| {
            this.flush_layout_state(cx);
            crate::themes::save_state(cx);
        })
        .detach();

        let title_bar = cx.new(|cx| {
            AppTitleBar::new("Agent Studio", window, cx)
            // .child({
            //     move |_, cx| {
            //         Button::new("add-panel")
            //             .icon(IconName::LayoutDashboard)
            //             .small()
            //             .ghost()
            //             .dropdown_menu({
            //                 let invisible_panels = AppState::global(cx).invisible_panels.clone();

            //                 move |menu, _, cx| {
            //                     menu.menu(
            //                         "Add Panel to Center",
            //                         Box::new(PanelAction::add_conversation(DockPlacement::Center)),
            //                     )
            //                     .separator()
            //                     .menu(
            //                         "Add Panel to Left",
            //                         Box::new(PanelAction::add_conversation(DockPlacement::Left)),
            //                     )
            //                     .menu(
            //                         "Add Panel to Right",
            //                         Box::new(PanelAction::add_conversation(DockPlacement::Right)),
            //                     )
            //                     .menu(
            //                         "Add Panel to Bottom",
            //                         Box::new(PanelAction::add_conversation(DockPlacement::Bottom)),
            //                     )
            //                     .separator()
            //                     .menu(
            //                         "Show / Hide Dock Toggle Button",
            //                         Box::new(ToggleDockToggleButton),
            //                     )
            //                     .separator()
            //                     .menu_with_check(
            //                         "Sidebar",
            //                         !invisible_panels
            //                             .read(cx)
            //                             .contains(&SharedString::from("Sidebar")),
            //                         Box::new(TogglePanelVisible(SharedString::from("Sidebar"))),
            //                     )
            //                     .menu_with_check(
            //                         "Dialog",
            //                         !invisible_panels
            //                             .read(cx)
            //                             .contains(&SharedString::from("Dialog")),
            //                         Box::new(TogglePanelVisible(SharedString::from("Dialog"))),
            //                     )
            //                     .menu_with_check(
            //                         "Accordion",
            //                         !invisible_panels
            //                             .read(cx)
            //                             .contains(&SharedString::from("Accordion")),
            //                         Box::new(TogglePanelVisible(SharedString::from("Accordion"))),
            //                     )
            //                     .menu_with_check(
            //                         "List",
            //                         !invisible_panels
            //                             .read(cx)
            //                             .contains(&SharedString::from("List")),
            //                         Box::new(TogglePanelVisible(SharedString::from("List"))),
            //                     )
            //                 }
            //             })
            //             .anchor(Corner::TopRight)
            //     }
            // })
        });

        Self {
            dock_area,
            title_bar,
            last_layout_state: None,
            toggle_button_visible: true,
            _save_layout_task: None,
            startup_state: StartupState::new(),
        }
    }

    fn ensure_startup_initialized(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.startup_state.initialized {
            self.startup_state.initialized = true;
            self.start_nodejs_check(window, cx);
        }

        self.maybe_sync_agents(window, cx);
        self.maybe_check_workspace(window, cx);
    }

    fn start_nodejs_check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.startup_state.nodejs_status, NodeJsStatus::Checking) {
            return;
        }

        let custom_path = AppSettings::global(cx).nodejs_path.clone();
        let custom_path = if custom_path.is_empty() {
            None
        } else {
            Some(PathBuf::from(custom_path.to_string()))
        };

        self.startup_state.nodejs_status = NodeJsStatus::Checking;
        self.startup_state.nodejs_skipped = false;
        cx.notify();

        cx.spawn_in(window, async move |this, window| {
            let checker = NodeJsChecker::new(custom_path);
            let result = checker.check_nodejs_available_blocking();

            _ = this.update_in(window, |this, _, cx| {
                match result {
                    Ok(result) => {
                        if result.available {
                            this.startup_state.nodejs_status = NodeJsStatus::Available {
                                version: result.version,
                                path: result.path,
                            };
                        } else {
                            this.startup_state.nodejs_status = NodeJsStatus::Unavailable {
                                message: result
                                    .error_message
                                    .unwrap_or_else(|| "Node.js not found".to_string()),
                                hint: result.install_hint,
                            };
                        }
                    }
                    Err(err) => {
                        this.startup_state.nodejs_status = NodeJsStatus::Unavailable {
                            message: err.to_string(),
                            hint: None,
                        };
                    }
                }

                this.startup_state.advance_step_if_needed();
                cx.notify();
            });
        })
        .detach();
    }

    fn maybe_sync_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_state.agent_synced || self.startup_state.agent_sync_in_progress {
            return;
        }

        let agent_config_service = match AppState::global(cx).agent_config_service() {
            Some(service) => service.clone(),
            None => return,
        };

        self.startup_state.agent_sync_in_progress = true;

        cx.spawn_in(window, async move |this, window| {
            let current_agents = agent_config_service.list_agents().await;
            let current_names: HashSet<String> =
                current_agents.into_iter().map(|(name, _)| name).collect();

            _ = this.update_in(window, |this, _, cx| {
                for choice in &mut this.startup_state.agent_choices {
                    choice.enabled = current_names.contains(&choice.name);
                }

                this.startup_state.agent_synced = true;
                this.startup_state.agent_sync_in_progress = false;

                if this.startup_state.agent_choices.is_empty() {
                    this.startup_state.agent_applied = true;
                }

                this.startup_state.advance_step_if_needed();
                cx.notify();
            });
        })
        .detach();
    }

    fn maybe_check_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_state.workspace_checked || self.startup_state.workspace_check_in_progress {
            return;
        }

        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                self.startup_state.workspace_checked = true;
                return;
            }
        };

        self.startup_state.workspace_check_in_progress = true;

        cx.spawn_in(window, async move |this, window| {
            let active_workspace = workspace_service.get_active_workspace().await;
            let fallback_workspace = if active_workspace.is_none() {
                workspace_service.list_workspaces().await.into_iter().next()
            } else {
                None
            };

            let selected_path = active_workspace
                .map(|ws| ws.path)
                .or_else(|| fallback_workspace.map(|ws| ws.path));

            _ = this.update_in(window, |this, _, cx| {
                if let Some(path) = selected_path {
                    this.startup_state.workspace_selected = true;
                    this.startup_state.workspace_path = Some(path.clone());
                    AppState::global_mut(cx).set_current_working_dir(path);
                }

                this.startup_state.workspace_checked = true;
                this.startup_state.workspace_check_in_progress = false;
                this.startup_state.advance_step_if_needed();
                cx.notify();
            });
        })
        .detach();
    }

    fn apply_agent_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_state.agent_apply_in_progress {
            return;
        }

        let agent_config_service = match AppState::global(cx).agent_config_service() {
            Some(service) => service.clone(),
            None => {
                self.startup_state.agent_apply_error =
                    Some("Agent service is not initialized yet.".to_string());
                cx.notify();
                return;
            }
        };

        let selections = self.startup_state.agent_choices.clone();
        let default_configs = self.startup_state.default_agent_configs.clone();

        self.startup_state.agent_apply_in_progress = true;
        self.startup_state.agent_apply_error = None;
        cx.notify();

        cx.spawn_in(window, async move |this, window| {
            let current_agents = agent_config_service.list_agents().await;
            let current_names: HashSet<String> =
                current_agents.into_iter().map(|(name, _)| name).collect();
            let mut errors = Vec::new();

            for choice in selections {
                if choice.enabled && !current_names.contains(&choice.name) {
                    match default_configs.get(&choice.name) {
                        Some(config) => {
                            if let Err(err) = agent_config_service
                                .add_agent(choice.name.clone(), config.clone())
                                .await
                            {
                                errors.push(format!("Failed to enable {}: {}", choice.name, err));
                            }
                        }
                        None => {
                            errors.push(format!(
                                "Missing config for selected agent: {}",
                                choice.name
                            ));
                        }
                    }
                } else if !choice.enabled && current_names.contains(&choice.name) {
                    if let Err(err) = agent_config_service.remove_agent(&choice.name).await {
                        errors.push(format!("Failed to disable {}: {}", choice.name, err));
                    }
                }
            }

            _ = this.update_in(window, |this, _, cx| {
                this.startup_state.agent_apply_in_progress = false;

                if errors.is_empty() {
                    this.startup_state.agent_applied = true;
                    this.startup_state.agent_apply_error = None;
                } else {
                    this.startup_state.agent_apply_error = Some(errors.join("\n"));
                }

                this.startup_state.advance_step_if_needed();
                cx.notify();
            });
        })
        .detach();
    }

    fn open_workspace_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_state.workspace_loading {
            return;
        }

        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                self.startup_state.workspace_error =
                    Some("Workspace service is not available.".to_string());
                cx.notify();
                return;
            }
        };

        self.startup_state.workspace_loading = true;
        self.startup_state.workspace_error = None;
        cx.notify();

        let dialog_title = "Open Project Folder";

        cx.spawn_in(window, async move |this, window| {
            let selection = utils::pick_folder(dialog_title).await;
            let cancelled = selection.is_none();

            if cancelled {
                _ = this.update_in(window, |this, _, cx| {
                    this.startup_state.workspace_loading = false;
                    cx.notify();
                });
                return;
            }

            let Some(folder_path) = selection else {
                return;
            };

            let add_result = workspace_service.add_workspace(folder_path.clone()).await;
            let mut selected_path = None;
            let mut error_message = None;

            match add_result {
                Ok(workspace) => {
                    selected_path = Some(workspace.path);
                }
                Err(err) => {
                    let message = err.to_string();
                    if message.contains("Workspace already exists") {
                        selected_path = Some(folder_path.clone());
                    } else {
                        error_message = Some(message);
                    }
                }
            }

            _ = this.update_in(window, |this, _, cx| {
                this.startup_state.workspace_loading = false;

                if let Some(path) = selected_path {
                    this.startup_state.workspace_selected = true;
                    this.startup_state.workspace_path = Some(path.clone());
                    this.startup_state.workspace_error = None;
                    this.startup_state.workspace_checked = true;
                    AppState::global_mut(cx).set_current_working_dir(path);
                    this.startup_state.advance_step_if_needed();
                } else {
                    this.startup_state.workspace_error = error_message;
                }

                cx.notify();
            });
        })
        .detach();
    }

    fn render_startup(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let node_icon = match self.startup_state.nodejs_status {
            NodeJsStatus::Available { .. } => IconName::CircleCheck,
            NodeJsStatus::Unavailable { .. } => IconName::TriangleAlert,
            NodeJsStatus::Checking => IconName::LoaderCircle,
            NodeJsStatus::Idle => IconName::SquareTerminal,
        };

        let agent_icon = if self.startup_state.agents_ready() {
            IconName::CircleCheck
        } else if self.startup_state.agent_apply_error.is_some() {
            IconName::TriangleAlert
        } else {
            IconName::Bot
        };

        let workspace_icon = if self.startup_state.workspace_ready() {
            IconName::CircleCheck
        } else {
            IconName::Folder
        };

        let stepper = Stepper::new("startup-stepper")
            .vertical()
            .with_size(UiSize::Medium)
            .selected_index(self.startup_state.step)
            .items_center()
            .items([
                StepperItem::new()
                    .icon(node_icon)
                    .child(v_flex().child("Node.js 环境").child("检测系统依赖")),
                StepperItem::new()
                    .icon(agent_icon)
                    .child(v_flex().child("启用 Agent").child("选择默认配置")),
                StepperItem::new()
                    .icon(workspace_icon)
                    .child(v_flex().child("打开文件夹").child("设置工作区")),
            ])
            .on_click(cx.listener(|this, step, _, cx| {
                this.startup_state.step = *step;
                cx.notify();
            }));

        let content = match self.startup_state.step {
            0 => self.render_nodejs_step(cx),
            1 => self.render_agents_step(cx),
            _ => self.render_workspace_step(cx),
        };

        let theme = cx.theme();

        div()
            .flex_1()
            .bg(theme.background)
            .p_6()
            .child(
                h_flex()
                    .gap_6()
                    .items_start()
                    .child(div().w(px(260.)).child(stepper))
                    .child(
                        div()
                            .flex_1()
                            .rounded(theme.radius_lg)
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.secondary)
                            .p_4()
                            .child(content),
                    ),
            )
            .into_any_element()
    }

    fn render_nodejs_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let mut content = v_flex()
            .gap_3()
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("1. Node.js 环境检查"),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("用于启动内置 agent，可在设置中自定义 Node.js 路径。"),
            );

        match &self.startup_state.nodejs_status {
            NodeJsStatus::Idle => {
                content = content.child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child("尚未开始检测。"),
                );
            }
            NodeJsStatus::Checking => {
                content = content.child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child("正在检测 Node.js 环境..."),
                );
            }
            NodeJsStatus::Available { version, path } => {
                let detail = match (version, path) {
                    (Some(version), Some(path)) => {
                        format!("已检测到 Node.js {} — {}", version, path.display())
                    }
                    (Some(version), None) => format!("已检测到 Node.js {}", version),
                    (None, Some(path)) => format!("已检测到 Node.js — {}", path.display()),
                    (None, None) => "已检测到 Node.js".to_string(),
                };

                content = content.child(div().text_color(theme.success_foreground).child(detail));
            }
            NodeJsStatus::Unavailable { message, hint } => {
                content = content.child(
                    div()
                        .text_color(theme.colors.danger_foreground)
                        .child(message.clone()),
                );

                if let Some(hint) = hint {
                    content =
                        content.child(div().text_color(theme.muted_foreground).child(hint.clone()));
                }
            }
        }

        let mut actions = h_flex().gap_2().pt_2();

        actions = actions
            .child(
                Button::new("startup-nodejs-recheck")
                    .label("重新检测")
                    .outline()
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        this.start_nodejs_check(window, cx);
                    })),
            )
            .child(
                Button::new("startup-nodejs-settings")
                    .label("打开设置")
                    .ghost()
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        this.on_action_open_setting_panel(&OpenSettings, window, cx);
                    })),
            );

        if self.startup_state.nodejs_ready() {
            actions = actions.child(
                Button::new("startup-nodejs-next")
                    .label("继续")
                    .primary()
                    .on_click(cx.listener(|this, _ev, _, cx| {
                        this.startup_state.step = 1;
                        cx.notify();
                    })),
            );
        } else {
            actions = actions.child(
                Button::new("startup-nodejs-skip")
                    .label("暂时跳过")
                    .ghost()
                    .on_click(cx.listener(|this, _ev, _, cx| {
                        this.startup_state.nodejs_skipped = true;
                        this.startup_state.advance_step_if_needed();
                        cx.notify();
                    })),
            );
        }

        content.child(actions).into_any_element()
    }

    fn render_agents_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let mut content = v_flex()
            .gap_3()
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("2. 选择启用的 Agent"),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("基于内置 config.json 选择默认启动的 agent。"),
            );

        if let Some(error) = &self.startup_state.agent_load_error {
            content = content.child(
                div()
                    .text_color(theme.colors.danger_foreground)
                    .child(error.clone()),
            );
        }

        if self.startup_state.agent_choices.is_empty() {
            content = content.child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("未找到内置 agent 配置。"),
            );
        } else {
            let mut list = v_flex().gap_2();
            let disabled = self.startup_state.agent_apply_in_progress;

            for (idx, choice) in self.startup_state.agent_choices.iter().enumerate() {
                let name = choice.name.clone();
                let checked = choice.enabled;
                list = list.child(
                    Checkbox::new(("startup-agent", idx))
                        .label(name.clone())
                        .checked(checked)
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, checked, _, cx| {
                            if let Some(choice) = this.startup_state.agent_choices.get_mut(idx) {
                                choice.enabled = *checked;
                                cx.notify();
                            }
                        })),
                );
            }

            content = content.child(list);
        }

        if AppState::global(cx).agent_config_service().is_none() {
            content = content.child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("Agent 服务初始化中，请稍后再试。"),
            );
        }

        if let Some(error) = &self.startup_state.agent_apply_error {
            content = content.child(
                div()
                    .text_color(theme.colors.danger_foreground)
                    .child(error.clone()),
            );
        }

        let service_ready = AppState::global(cx).agent_config_service().is_some();
        let apply_label = if self.startup_state.agent_apply_in_progress {
            "应用中..."
        } else {
            "应用并继续"
        };

        let mut actions = h_flex().gap_2().pt_2();
        actions = actions
            .child(
                Button::new("startup-agent-apply")
                    .label(apply_label)
                    .primary()
                    .disabled(!service_ready || self.startup_state.agent_apply_in_progress)
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        this.apply_agent_selection(window, cx);
                    })),
            )
            .child(
                Button::new("startup-agent-skip")
                    .label("稍后设置")
                    .ghost()
                    .on_click(cx.listener(|this, _ev, _, cx| {
                        this.startup_state.agent_applied = true;
                        this.startup_state.advance_step_if_needed();
                        cx.notify();
                    })),
            );

        content.child(actions).into_any_element()
    }

    fn render_workspace_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let mut content = v_flex()
            .gap_3()
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("3. 打开一个文件夹"),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("选择一个本地项目作为工作区。"),
            );

        if let Some(path) = &self.startup_state.workspace_path {
            content = content.child(
                div()
                    .text_color(theme.success_foreground)
                    .child(format!("当前工作区：{}", path.display())),
            );
        } else {
            content = content.child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("尚未选择工作区。"),
            );
        }

        if let Some(error) = &self.startup_state.workspace_error {
            content = content.child(
                div()
                    .text_color(theme.colors.danger_foreground)
                    .child(error.clone()),
            );
        }

        if self.startup_state.workspace_loading {
            content = content.child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("正在打开文件夹..."),
            );
        }

        let pick_label = if self.startup_state.workspace_selected {
            "重新选择文件夹"
        } else {
            "选择文件夹"
        };

        let mut actions = h_flex().gap_2().pt_2();
        actions = actions.child(
            Button::new("startup-workspace-pick")
                .label(pick_label)
                .primary()
                .disabled(self.startup_state.workspace_loading)
                .on_click(cx.listener(|this, _ev, window, cx| {
                    this.open_workspace_folder(window, cx);
                })),
        );

        content.child(actions).into_any_element()
    }

    fn save_layout(
        &mut self,
        dock_area: &Entity<DockArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dock_area = dock_area.clone();
        self._save_layout_task = Some(cx.spawn_in(window, async move |agent_studio, window| {
            Timer::after(Duration::from_secs(10)).await;

            _ = agent_studio.update_in(window, move |this, _, cx| {
                let dock_area = dock_area.read(cx);
                let state = dock_area.dump(cx);

                let last_layout_state = this.last_layout_state.clone();
                if Some(&state) == last_layout_state.as_ref() {
                    return;
                }

                if let Err(e) = Self::save_state(&state) {
                    log::warn!("Failed to save layout state: {}", e);
                }
                this.last_layout_state = Some(state);
            });
        }));
    }

    fn flush_layout_state(&mut self, cx: &mut App) {
        let state = self.dock_area.read(cx).dump(cx);
        if Some(&state) == self.last_layout_state.as_ref() {
            return;
        }
        if let Err(e) = Self::save_state(&state) {
            log::warn!("Failed to save layout state: {}", e);
        }
        self.last_layout_state = Some(state);
    }

    fn save_state(state: &DockAreaState) -> Result<()> {
        println!("Save Docks layout...");
        let json = serde_json::to_string_pretty(state)?;
        let state_file = crate::core::config_manager::get_docks_layout_path();
        if let Some(parent) = state_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(state_file, json)?;
        Ok(())
    }

    fn load_layout(
        dock_area: Entity<DockArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        println!("Load Docks layout...");
        let state_file = crate::core::config_manager::get_docks_layout_path();
        let json = std::fs::read_to_string(state_file)?;
        let state = serde_json::from_str::<DockAreaState>(&json)?;

        // Check if the saved layout version is different from the current version
        // Notify the user and ask if they want to reset the layout to default.
        if state.version != Some(MAIN_DOCK_AREA.version) {
            let answer = window.prompt(
                PromptLevel::Info,
                "The default main layout has been updated.\n\nDo you want to reset the layout to default?",
                None,
                &["Yes", "No"],
                cx,
            );

            let weak_dock_area = dock_area.downgrade();
            cx.spawn_in(window, async move |this, window| {
                if answer.await == Ok(0) {
                    _ = this.update_in(window, |_, window, cx| {
                        Self::reset_default_layout(weak_dock_area, window, cx);
                    });
                }
            })
            .detach();
        }

        dock_area.update(cx, |dock_area, cx| {
            dock_area.load(state, window, cx).context("load layout")?;
            dock_area.set_dock_collapsible(
                Edges {
                    left: true,
                    bottom: true,
                    right: true,
                    ..Default::default()
                },
                window,
                cx,
            );

            Ok::<(), anyhow::Error>(())
        })
    }

    fn reset_default_layout(dock_area: WeakEntity<DockArea>, window: &mut Window, cx: &mut App) {
        let dock_item = Self::init_default_layout(&dock_area, window, cx);

        let left_panels = DockItem::split_with_sizes(
            Axis::Vertical,
            vec![DockItem::tab(
                DockPanelContainer::panel::<TaskPanel>(window, cx),
                &dock_area,
                window,
                cx,
            )],
            vec![None, Some(px(360.))],
            &dock_area,
            window,
            cx,
        );

        let bottom_panels = DockItem::split_with_sizes(
            Axis::Vertical,
            vec![DockItem::tabs(
                vec![
                    Arc::new(DockPanelContainer::panel::<TerminalPanel>(window, cx)),
                    Arc::new(DockPanelContainer::panel::<SessionManagerPanel>(window, cx)),
                ],
                &dock_area,
                window,
                cx,
            )],
            vec![None],
            &dock_area,
            window,
            cx,
        );

        let right_panels = DockItem::split_with_sizes(
            Axis::Vertical,
            vec![DockItem::tabs(
                vec![
                    Arc::new(DockPanelContainer::panel::<CodeEditorPanel>(window, cx)),
                    // Arc::new(DockPanelContainer::panel::<TerminalPanel>(window, cx)),
                ],
                &dock_area,
                window,
                cx,
            )],
            vec![None],
            &dock_area,
            window,
            cx,
        );

        _ = dock_area.update(cx, |view, cx| {
            view.set_version(MAIN_DOCK_AREA.version, window, cx);
            view.set_center(dock_item, window, cx);
            view.set_left_dock(left_panels, Some(px(350.)), true, window, cx);
            view.set_bottom_dock(bottom_panels, Some(px(200.)), true, window, cx);
            view.set_right_dock(right_panels, Some(px(480.)), true, window, cx);

            if let Err(e) = Self::save_state(&view.dump(cx)) {
                log::warn!("Failed to save layout state: {}", e);
            }
        });
    }

    fn init_default_layout(
        dock_area: &WeakEntity<DockArea>,
        window: &mut Window,
        cx: &mut App,
    ) -> DockItem {
        // Main layout: Left (CodeEditorPanel) and Right (Conversation + Input)
        DockItem::split_with_sizes(
            Axis::Horizontal,
            vec![
                // Left panel: ConversationPanel (ACP-enabled conversation)
                DockItem::tabs(
                    vec![Arc::new(DockPanelContainer::panel::<ConversationPanel>(
                        window, cx,
                    ))],
                    &dock_area,
                    window,
                    cx,
                ),
                // Right panel: Combined conversation and input
                // right_side,
            ],
            vec![None, None],
            &dock_area,
            window,
            cx,
        )
    }

    pub fn new_local(cx: &mut App) -> Task<anyhow::Result<WindowHandle<Root>>> {
        let mut window_size = size(px(1600.0), px(1200.0));
        if let Some(display) = cx.primary_display() {
            let display_size = display.bounds().size;
            window_size.width = window_size.width.min(display_size.width * 0.85);
            window_size.height = window_size.height.min(display_size.height * 0.85);
        }

        let window_bounds = Bounds::centered(None, window_size, cx);

        cx.spawn(async move |cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                #[cfg(not(target_os = "linux"))]
                titlebar: Some(gpui_component::TitleBar::title_bar_options()),
                window_min_size: Some(gpui::Size {
                    width: px(640.0),
                    height: px(480.0),
                }),
                #[cfg(target_os = "linux")]
                window_background: gpui::WindowBackgroundAppearance::Transparent,
                #[cfg(target_os = "linux")]
                window_decorations: Some(gpui::WindowDecorations::Client),
                kind: WindowKind::Normal,
                ..Default::default()
            };

            let window = cx.open_window(options, |window, cx| {
                let agent_studio_view = cx.new(|cx| DockWorkspace::new(window, cx));
                cx.new(|cx| Root::new(agent_studio_view, window, cx))
            })?;

            window
                .update(cx, |_, window, cx| {
                    window.activate_window();
                    window.set_window_title("Agent Studio");
                    cx.on_release(|_, cx| {
                        // exit app
                        cx.quit();
                    })
                    .detach();
                })
                .expect("failed to update window");

            Ok(window)
        })
    }
}

impl Render for DockWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_startup_initialized(window, cx);

        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        let content = if self.startup_state.is_complete() {
            self.dock_area.clone().into_any_element()
        } else {
            self.render_startup(cx)
        };

        div()
            .id("agent_studio-workspace")
            .on_action(cx.listener(Self::on_action_panel_action))
            .on_action(cx.listener(Self::on_action_toggle_panel_visible))
            .on_action(cx.listener(Self::on_action_toggle_dock_toggle_button))
            .on_action(cx.listener(Self::on_action_open_setting_panel))
            .on_action(cx.listener(Self::on_action_new_session_conversation_panel))
            .on_action(cx.listener(Self::on_action_create_task_from_welcome))
            .on_action(cx.listener(Self::on_action_send_message_to_session))
            .on_action(cx.listener(Self::on_action_cancel_session))
            .on_action(cx.listener(Self::on_action_open))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .child(self.title_bar.clone())
            .child(content)
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

pub fn open_new(
    cx: &mut App,
    init: impl FnOnce(&mut Root, &mut Window, &mut Context<Root>) + 'static + Send,
) -> Task<()> {
    let task: Task<std::result::Result<WindowHandle<Root>, anyhow::Error>> =
        DockWorkspace::new_local(cx);
    cx.spawn(async move |cx| {
        if let Some(root) = task.await.ok() {
            root.update(cx, |workspace, window, cx| init(workspace, window, cx))
                .expect("failed to init workspace");
        }
    })
}
