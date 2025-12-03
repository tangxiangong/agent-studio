//! Task Panel - Workspace sidebar for agentx
//!
//! This panel uses the Workspace Sidebar UI design to display tasks organized by workspace.
//! Features:
//! - Workspace groups that can be expanded/collapsed
//! - Task items with status and last message preview
//! - View toggle between tree view and timeline view (by date)
//! - Random data generation for demonstration

use gpui::{
    div, px, prelude::FluentBuilder, App, AppContext, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window,
};
use gpui_component::{
    button::{Button, ButtonGroup, ButtonVariants},
    h_flex, v_flex, ActiveTheme, Icon, IconName, Selectable, Sizable, StyledExt,
    scroll::ScrollableElement as _,
};
use rand::Rng;
use std::rc::Rc;

use crate::core::event_bus::WorkspaceUpdateEvent;
use crate::core::services::WorkspaceService;
use crate::panels::dock_panel::DockPanel;
use crate::schemas::workspace_schema::{TaskStatus, WorkspaceTask};
use crate::{utils, AppState, ShowConversationPanel, ShowWelcomePanel};

// ============================================================================
// Data Models
// ============================================================================

#[derive(Clone, Debug)]
pub struct WorkspaceGroup {
    pub id: String,
    pub name: String,
    pub tasks: Vec<Rc<WorkspaceTask>>,
    pub is_expanded: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewMode {
    Tree,       // Group by workspace
    Timeline,   // Group by date
}

// ============================================================================
// Main Panel Component
// ============================================================================

pub struct TaskPanel {
    focus_handle: FocusHandle,
    workspaces: Vec<WorkspaceGroup>,
    selected_task_id: Option<String>,
    view_mode: ViewMode,
    _subscriptions: Vec<Subscription>,
    use_real_data: bool, // Flag to distinguish between random data and real data
}

impl DockPanel for TaskPanel {
    fn title() -> &'static str {
        "任务"
    }

    fn description() -> &'static str {
        "Task list grouped by workspace"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }

    fn paddings() -> Pixels {
        px(0.)
    }
}

impl TaskPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let entity = cx.new(|cx| Self::new(window, cx));

        // Try to load real workspace data first
        if let Some(workspace_service) = AppState::global(cx).workspace_service() {
            Self::load_workspace_data(&entity, workspace_service.clone(), cx);
            Self::subscribe_to_workspace_updates(&entity, cx);
        } else {
            // Fallback to random data if no workspace service
            Self::load_random_data(&entity, cx);
        }

        entity
    }

    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            workspaces: Vec::new(),
            selected_task_id: None,
            view_mode: ViewMode::Tree,
            _subscriptions: Vec::new(),
            use_real_data: false,
        }
    }

    /// Load workspace data from WorkspaceService
    fn load_workspace_data(
        entity: &Entity<Self>,
        workspace_service: std::sync::Arc<WorkspaceService>,
        cx: &mut App,
    ) {
        let entity_clone = entity.clone();
        cx.spawn(async move |cx| {
            let workspaces_list = workspace_service.list_workspaces().await;
            let config = workspace_service.get_config().await;

            cx.update(|cx| {
                entity_clone.update(cx, |this, cx| {
                    this.use_real_data = true;
                    this.workspaces = workspaces_list
                        .into_iter()
                        .map(|ws| {
                            let tasks = config
                                .tasks
                                .iter()
                                .filter(|t| t.workspace_id == ws.id)
                                .map(|t| Rc::new(t.clone()))
                                .collect();

                            WorkspaceGroup {
                                id: ws.id.clone(),
                                name: ws.name.clone(),
                                tasks,
                                is_expanded: true,
                            }
                        })
                        .collect();

                    // Select the first task if available
                    if let Some(first_workspace) = this.workspaces.first() {
                        if let Some(first_task) = first_workspace.tasks.first() {
                            this.selected_task_id = Some(first_task.id.clone());
                        }
                    }

                    cx.notify();
                });
            })
            .ok();
        })
        .detach();
    }

    /// Subscribe to workspace update events
    fn subscribe_to_workspace_updates(_entity: &Entity<Self>, cx: &mut App) {
        let workspace_bus = AppState::global(cx).workspace_bus.clone();

        workspace_bus.lock().unwrap().subscribe(move |event| {
            match event {
                WorkspaceUpdateEvent::WorkspaceAdded { workspace_id } => {
                    log::debug!("TaskPanel received WorkspaceAdded: {}", workspace_id);
                    // Note: Cannot reload here due to async/sync boundary
                    // The add_workspace method will trigger a reload manually
                }
                WorkspaceUpdateEvent::WorkspaceRemoved { workspace_id } => {
                    log::debug!("TaskPanel received WorkspaceRemoved: {}", workspace_id);
                }
                WorkspaceUpdateEvent::TaskCreated { workspace_id, task_id } => {
                    log::debug!("TaskPanel received TaskCreated: {} in {}", task_id, workspace_id);
                }
                WorkspaceUpdateEvent::TaskUpdated { task_id } => {
                    log::debug!("TaskPanel received TaskUpdated: {}", task_id);
                }
            }
        });
    }

    /// Load random workspace and task data
    fn load_random_data(entity: &Entity<Self>, cx: &mut App) {
        let workspaces = Self::generate_random_workspaces();
        entity.update(cx, |this, cx| {
            this.use_real_data = false;
            this.workspaces = workspaces;
            // Select the first task if available
            if let Some(first_workspace) = this.workspaces.first() {
                if let Some(first_task) = first_workspace.tasks.first() {
                    this.selected_task_id = Some(first_task.id.clone());
                }
            }
            cx.notify();
        });
    }

    /// Generate random workspace data for demonstration
    fn generate_random_workspaces() -> Vec<WorkspaceGroup> {
        let mut rng = rand::thread_rng();

        let workspace_names = vec![
            "conductor",
            "melty_home",
            "swipe",
            "conductor-docs",
            "conductor_api",
            "chorus",
            "api",
            "metarquiz-2",
        ];

        let task_modes = vec!["Auto", "Ask", "Plan", "Code", "Explain"];
        let agent_names = vec!["claude", "gpt-4", "gemini", "copilot"];

        let sample_messages = vec![
            "Implement user authentication",
            "Fix layout issue on mobile",
            "Add dark mode support",
            "Refactor database queries",
            "Write unit tests",
            "Update documentation",
            "Optimize performance",
            "Add error handling",
        ];

        workspace_names.iter().enumerate().map(|(idx, name)| {
            let task_count = if idx < 2 { rng.gen_range(2..4) } else { 0 };
            let tasks: Vec<_> = (0..task_count).map(|i| {
                let workspace_id = format!("ws-{}", idx);
                let task_name = sample_messages[rng.gen_range(0..sample_messages.len())].to_string();
                let agent_name = agent_names[rng.gen_range(0..agent_names.len())].to_string();
                let mode = task_modes[rng.gen_range(0..task_modes.len())].to_string();

                let mut task = WorkspaceTask::new(workspace_id.clone(), task_name, agent_name, mode);

                // Randomly assign status
                let status_rand = rng.gen_range(0..4);
                task.status = match status_rand {
                    0 => TaskStatus::Pending,
                    1 => TaskStatus::InProgress,
                    2 => TaskStatus::Completed,
                    _ => TaskStatus::Failed,
                };

                // Randomly add session ID for in-progress tasks
                if task.status == TaskStatus::InProgress {
                    task.session_id = Some(format!("session-{}-{}", idx, i));
                }

                // Add last message for some tasks
                if rng.gen_bool(0.7) {
                    let messages = vec![
                        "Working on it...",
                        "Almost done",
                        "Need more information",
                        "Completed successfully",
                        "Encountered an error",
                    ];
                    task.last_message = Some(SharedString::from(messages[rng.gen_range(0..messages.len())]));
                }

                // Randomize created_at to test timeline view
                let days_ago = rng.gen_range(0..30);
                task.created_at = chrono::Utc::now() - chrono::Duration::days(days_ago);

                Rc::new(task)
            }).collect();

            WorkspaceGroup {
                id: format!("ws-{}", idx),
                name: name.to_string(),
                is_expanded: idx < 2, // Expand first two workspaces
                tasks,
            }
        }).collect()
    }

    fn toggle_workspace(&mut self, workspace_id: String, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspaces.iter_mut().find(|w| w.id == workspace_id) {
            workspace.is_expanded = !workspace.is_expanded;
            cx.notify();
        }
    }

    fn add_workspace(&mut self, cx: &mut Context<Self>) {
        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                log::warn!("WorkspaceService not available");
                return;
            }
        };

        cx.spawn(async move |entity, cx| {
            // Open folder picker
            if let Some(folder_path) = utils::pick_folder("选择工作区文件夹").await {
                log::info!("Selected folder: {:?}", folder_path);

                // Add workspace via service
                match workspace_service.add_workspace(folder_path.clone()).await {
                    Ok(workspace) => {
                        log::info!("Successfully added workspace: {}", workspace.name);

                        // Reload workspace data
                        cx.update(|cx| {
                            if let Some(entity_strong) = entity.upgrade() {
                                Self::load_workspace_data(&entity_strong, workspace_service.clone(), cx);
                            }
                        }).ok();
                    }
                    Err(e) => {
                        log::error!("Failed to add workspace: {}", e);
                    }
                }
            } else {
                log::info!("Folder selection cancelled");
            }
        })
        .detach();
    }

    fn select_task(&mut self, task_id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_task_id = Some(task_id.clone());

        // Dispatch action to show conversation panel
        window.dispatch_action(Box::new(ShowConversationPanel), cx);
        cx.notify();
    }

    fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        self.view_mode = mode;
        cx.notify();
    }

    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let view_mode = self.view_mode;

        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .px_3()
            .py_3()
            .border_b_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(IconName::Inbox).size_4().text_color(theme.muted_foreground))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.foreground)
                            .child("工作区"),
                    ),
            )
            .child(
                ButtonGroup::new("view-toggle")
                    .small()
                    .child(
                        Button::new("tree-view")
                            .icon(IconName::LayoutDashboard)
                            .ghost()
                            .xsmall()
                            .selected(view_mode == ViewMode::Tree)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_view_mode(ViewMode::Tree, cx);
                            })),
                    )
                    .child(
                        Button::new("timeline-view")
                            .icon(IconName::Menu)
                            .ghost()
                            .xsmall()
                            .selected(view_mode == ViewMode::Timeline)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_view_mode(ViewMode::Timeline, cx);
                            })),
                    ),
            )
    }

    fn render_footer(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .px_3()
            .py_2()
            .border_t_1()
            .border_color(theme.border)
            .child(
                Button::new("add-workspace")
                    .ghost()
                    .small()
                    .icon(IconName::FolderOpen)
                    .label("添加工作区")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.add_workspace(cx);
                    })),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(Button::new("refresh").ghost().small().icon(IconName::Delete))
                    .child(Button::new("monitor").ghost().small().icon(IconName::SquareTerminal))
                    .child(Button::new("settings").ghost().small().icon(IconName::Settings)),
            )
    }

    fn render_tree_view(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .min_h_0()
            .py_1()
            .overflow_y_scrollbar()
            .children(self.workspaces.iter().map(|workspace| {
                self.render_workspace_group(workspace, cx)
            }))
    }

    fn render_workspace_group(&self, workspace: &WorkspaceGroup, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let workspace_id = workspace.id.clone();
        let workspace_id_for_toggle = workspace_id.clone();
        let is_expanded = workspace.is_expanded;
        let has_tasks = !workspace.tasks.is_empty();
        let workspace_name = workspace.name.clone();
        let task_count = workspace.tasks.len();

        v_flex()
            .w_full()
            .child(
                h_flex()
                    .id(SharedString::from(format!("workspace-{}", workspace_id)))
                    .w_full()
                    .justify_between()
                    .items_center()
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.accent.opacity(0.3)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_workspace(workspace_id_for_toggle.clone(), cx);
                    }))
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .child(if is_expanded {
                                Icon::new(IconName::ChevronDown)
                                    .size_4()
                                    .text_color(theme.muted_foreground)
                                    .into_any_element()
                            } else {
                                Icon::new(IconName::ChevronRight)
                                    .size_4()
                                    .text_color(theme.muted_foreground)
                                    .into_any_element()
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .font_medium()
                                    .text_color(theme.foreground)
                                    .child(workspace_name),
                            ),
                    )
                    .child(if has_tasks {
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{}", task_count))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
            .when(is_expanded, |this| {
                this.child(self.render_new_task_button(cx))
                    .children(workspace.tasks.iter().map(|task| {
                        self.render_task_item(task, cx)
                    }))
            })
    }

    fn render_new_task_button(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .id("new-task-button")
            .w_full()
            .justify_between()
            .items_center()
            .px_3()
            .py_1p5()
            .cursor_pointer()
            .hover(|s| s.bg(theme.accent.opacity(0.3)))
            .on_click(cx.listener(|_this, _, window, cx| {
                // Dispatch action to show welcome panel for creating new task
                window.dispatch_action(Box::new(ShowWelcomePanel), cx);
            }))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .pl_5()
                    .child(
                        Icon::new(IconName::Plus)
                            .size_3p5()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("新建任务"),
                    ),
            )
            .child(
                Icon::new(IconName::Ellipsis)
                    .size_4()
                    .text_color(theme.muted_foreground)
                    .opacity(0.),
            )
    }

    fn render_task_item(&self, task: &Rc<WorkspaceTask>, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let task_id = task.id.clone();
        let task_id_for_click = task_id.clone();
        let is_selected = self.selected_task_id.as_ref() == Some(&task_id);

        v_flex()
            .id(SharedString::from(format!("task-{}", task_id)))
            .w_full()
            .gap_0p5()
            .px_3()
            .py_2()
            .cursor_pointer()
            .when(is_selected, |s| s.bg(theme.accent))
            .when(!is_selected, |s| s.hover(|s| s.bg(theme.accent.opacity(0.5))))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_task(task_id_for_click.clone(), window, cx);
            }))
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .min_w_0()
                            .flex_1()
                            .child(
                                Icon::new(self.status_icon(&task.status))
                                    .size_4()
                                    .text_color(self.status_color(&task.status)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .overflow_x_hidden()
                                    .text_ellipsis()
                                    .child(task.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(task.mode.clone()),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap_2()
                    .pl_6()
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .min_w_0()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(
                                div()
                                    .overflow_x_hidden()
                                    .text_ellipsis()
                                    .child(task.agent_name.clone()),
                            )
                            .child(div().child("·"))
                            .when_some(task.last_message.clone(), |this, msg| {
                                this.child(
                                    div()
                                        .overflow_x_hidden()
                                        .text_ellipsis()
                                        .child(msg),
                                )
                            }),
                    )
                    .child(self.render_status_badge(&task.status, cx)),
            )
    }

    fn render_timeline_view(&self, cx: &Context<Self>) -> impl IntoElement {
        use chrono::{Local, Duration};

        // Flatten all tasks and categorize by date
        let mut all_tasks: Vec<Rc<WorkspaceTask>> = Vec::new();
        for workspace in &self.workspaces {
            for task in &workspace.tasks {
                all_tasks.push(task.clone());
            }
        }

        // Sort by created_at descending
        all_tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let now = Local::now().date_naive();

        let today: Vec<_> = all_tasks
            .iter()
            .filter(|t| {
                let task_date = t.created_at.with_timezone(&Local).date_naive();
                task_date == now
            })
            .collect();

        let yesterday: Vec<_> = all_tasks
            .iter()
            .filter(|t| {
                let task_date = t.created_at.with_timezone(&Local).date_naive();
                task_date == now - Duration::days(1)
            })
            .collect();

        let older: Vec<_> = all_tasks
            .iter()
            .filter(|t| {
                let task_date = t.created_at.with_timezone(&Local).date_naive();
                task_date < now - Duration::days(1)
            })
            .collect();

        v_flex()
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .when(!today.is_empty(), |this| {
                this.child(self.render_time_group("今天", &today, cx))
            })
            .when(!yesterday.is_empty(), |this| {
                this.child(self.render_time_group("昨天", &yesterday, cx))
            })
            .when(!older.is_empty(), |this| {
                this.child(self.render_time_group("更早", &older, cx))
            })
    }

    fn render_time_group(
        &self,
        label: &str,
        tasks: &[&Rc<WorkspaceTask>],
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .w_full()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .bg(theme.sidebar.opacity(0.95))
                    .border_b_1()
                    .border_color(theme.border.opacity(0.5))
                    .child(
                        div()
                            .text_xs()
                            .font_medium()
                            .text_color(theme.muted_foreground)
                            .child(label.to_uppercase()),
                    ),
            )
            .children(tasks.iter().map(|task| {
                self.render_timeline_task_item(task, cx)
            }))
    }

    fn render_timeline_task_item(
        &self,
        task: &Rc<WorkspaceTask>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let task_id = task.id.clone();
        let task_id_for_click = task_id.clone();
        let is_selected = self.selected_task_id.as_ref() == Some(&task_id);

        v_flex()
            .id(SharedString::from(format!("timeline-task-{}", task_id)))
            .w_full()
            .gap_1()
            .px_3()
            .py_2p5()
            .cursor_pointer()
            .border_b_1()
            .border_color(theme.border.opacity(0.5))
            .when(is_selected, |s| s.bg(theme.accent))
            .when(!is_selected, |s| s.hover(|s| s.bg(theme.accent.opacity(0.5))))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_task(task_id_for_click.clone(), window, cx);
            }))
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(theme.foreground)
                            .overflow_x_hidden()
                            .text_ellipsis()
                            .child(task.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(task.mode.clone()),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(
                        Icon::new(self.status_icon(&task.status))
                            .size_3()
                            .text_color(self.status_color(&task.status)),
                    )
                    .child(
                        div()
                            .overflow_x_hidden()
                            .text_ellipsis()
                            .child(task.agent_name.clone()),
                    )
                    .when_some(task.last_message.clone(), |this, msg| {
                        this.child(div().child("·"))
                            .child(
                                div()
                                    .overflow_x_hidden()
                                    .text_ellipsis()
                                    .child(msg),
                            )
                    }),
            )
            .child(self.render_status_badge(&task.status, cx))
    }

    fn render_status_badge(&self, status: &TaskStatus, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (label, color) = match status {
            TaskStatus::Pending => ("等待中", theme.muted_foreground),
            TaskStatus::InProgress => ("进行中", gpui::rgb(0x22c55e).into()),
            TaskStatus::Completed => ("已完成", gpui::rgb(0x22c55e).into()),
            TaskStatus::Failed => ("失败", gpui::rgb(0xef4444).into()),
        };

        div()
            .text_xs()
            .text_color(color)
            .child(label)
    }

    fn status_icon(&self, status: &TaskStatus) -> IconName {
        match status {
            TaskStatus::Pending => IconName::Asterisk,  // Use Asterisk instead of Clock
            TaskStatus::InProgress => IconName::Loader,
            TaskStatus::Completed => IconName::CircleCheck,
            TaskStatus::Failed => IconName::CircleX,
        }
    }

    fn status_color(&self, status: &TaskStatus) -> gpui::Hsla {
        match status {
            TaskStatus::Pending => gpui::rgb(0x6b7280).into(),
            TaskStatus::InProgress => gpui::rgb(0x3b82f6).into(),
            TaskStatus::Completed => gpui::rgb(0x22c55e).into(),
            TaskStatus::Failed => gpui::rgb(0xef4444).into(),
        }
    }
}

impl Focusable for TaskPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TaskPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("task-panel")
            .track_focus(&self.focus_handle)
            .size_full()
            .child(self.render_header(cx))
            .child(if self.view_mode == ViewMode::Tree {
                self.render_tree_view(cx).into_any_element()
            } else {
                self.render_timeline_view(cx).into_any_element()
            })
            .child(self.render_footer(cx))
    }
}
