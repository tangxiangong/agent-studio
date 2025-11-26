use std::{cell::RefCell, collections::HashSet, rc::Rc, time::Duration};

use gpui::{
    actions, div, prelude::FluentBuilder, px, App, AppContext, Context, ElementId,
    Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, RenderOnce, SharedString, Styled, Subscription, Task, Timer, Window,
};
use serde::Deserialize;

use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
    spinner::Spinner,
    v_flex, ActiveTheme, Icon, IconName, IndexPath, Selectable, Sizable,
};

use crate::{CreateTaskFromWelcome, ShowConversationPanel, ShowWelcomePanel};

actions!(list_task, [SelectedAgentTask]);

/// Task status enumeration
#[derive(Clone, Default, Debug, Deserialize)]
enum TaskStatus {
    /// Task is pending
    #[default]
    Pending,
    /// Task is currently running
    InProgress,
    /// Task completed successfully
    Completed,
    /// Task failed to complete
    Failed,
}

#[derive(Clone, Default, Deserialize)]
struct AgentTask {
    name: String,
    task_type: String,
    add_new_code_lines: i16,
    delete_code_lines: i16,
    status: TaskStatus,

    #[serde(skip)]
    change_timestamp: i16,
    #[serde(skip)]
    change_timestamp_str: SharedString,
    #[serde(skip)]
    add_new_code_lines_str: SharedString,
    #[serde(skip)]
    delete_code_lines_str: SharedString,
}

impl AgentTask {
    fn prepare(mut self) -> Self {
        self.add_new_code_lines_str = format!("+{}", self.add_new_code_lines).into();
        self.delete_code_lines_str = format!("-{}", self.delete_code_lines).into();
        self
    }
}

#[derive(IntoElement)]
struct TaskListItem {
    base: ListItem,
    agent_task: Rc<AgentTask>,
    selected: bool,
}

impl TaskListItem {
    pub fn new(id: impl Into<ElementId>, agent_task: Rc<AgentTask>, selected: bool) -> Self {
        TaskListItem {
            agent_task,
            base: ListItem::new(id).selected(selected),
            selected,
        }
    }
}

impl Selectable for TaskListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for TaskListItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_color = if self.selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        };

        let muted_color = cx.theme().muted_foreground;
        let add_color = cx.theme().green;
        let delete_color = cx.theme().red;

        // Show metadata only when not selected
        let _show_metadata = !self.selected;

        // Check if task is in progress to use Spinner
        let is_in_progress = matches!(self.agent_task.status, TaskStatus::InProgress);

        self.base
            .px_3()
            .py_2()
            .overflow_x_hidden()
            .rounded(cx.theme().radius)
            .child(
                h_flex()
                    .items_start() // Top align instead of center
                    .gap_3()
                    .mt(px(2.))
                    .child(div().mt(px(2.)).map(|this| {
                        if is_in_progress {
                            // Use Spinner for InProgress status
                            this.child(Spinner::new().with_size(px(14.)).color(cx.theme().accent))
                        } else {
                            // Use Icon for other statuses
                            let (icon_name, icon_color) = match self.agent_task.status {
                                TaskStatus::Pending => (IconName::File, muted_color),
                                TaskStatus::Completed => (IconName::CircleCheck, cx.theme().green),
                                TaskStatus::Failed => (IconName::CircleX, cx.theme().red),
                                _ => (IconName::File, muted_color),
                            };
                            this.child(Icon::new(icon_name).text_color(icon_color).size(px(14.)))
                        }
                    }))
                    .child(
                        // Vertical layout for title and subtitle
                        v_flex()
                            .gap_0p5()
                            .flex_1()
                            .overflow_x_hidden()
                            .child(
                                // Title - reduced font size
                                div()
                                    .text_size(px(13.))
                                    .text_color(text_color)
                                    .whitespace_nowrap()
                                    .child(self.agent_task.name.clone()),
                            ).child(
                                // Subtitle with metadata - conditionally shown
                                h_flex()
                                    .gap_1()
                                    .text_size(px(11.))
                                    .text_color(muted_color)
                                    .child("2 Files ")
                                    .child(
                                        div().text_color(add_color).child(
                                            self.agent_task.add_new_code_lines_str.clone(),
                                        ),
                                    )
                                    .child(
                                        div().text_color(delete_color).child(
                                            self.agent_task.delete_code_lines_str.clone(),
                                        ),
                                    )
                                    .child(" · ")
                                    .child(self.agent_task.task_type.clone()),
                            )
                    ),
            )
    }
}

struct TaskListDelegate {
    industries: Vec<SharedString>,
    _agent_tasks: Vec<Rc<AgentTask>>,
    matched_agent_tasks: Vec<Vec<Rc<AgentTask>>>,
    selected_index: Option<IndexPath>,
    confirmed_index: Option<IndexPath>,
    query: SharedString,
    loading: bool,
    eof: bool,
    lazy_load: bool,
    // Track which sections are collapsed (using RefCell for interior mutability)
    collapsed_sections: Rc<RefCell<HashSet<usize>>>,
    // Store weak reference to list state to notify on collapse toggle
    list_state: Option<gpui::WeakEntity<ListState<TaskListDelegate>>>,
}

impl TaskListDelegate {
    fn is_section_collapsed(&self, section: usize) -> bool {
        self.collapsed_sections.borrow().contains(&section)
    }

    fn prepare(&mut self, query: impl Into<SharedString>) {
        self.query = query.into();
        // Clear previous data before rebuilding
        self.industries.clear();
        self.matched_agent_tasks.clear();

        let agent_tasks: Vec<Rc<AgentTask>> = self
            ._agent_tasks
            .iter()
            .filter(|agent_task| {
                agent_task
                    .name
                    .to_lowercase()
                    .contains(&self.query.to_lowercase())
            })
            .cloned()
            .collect();
        for agent_task in agent_tasks.into_iter() {
            if let Some(ix) = self
                .industries
                .iter()
                .position(|s| s.as_ref() == agent_task.task_type.as_str())
            {
                self.matched_agent_tasks[ix].push(agent_task);
            } else {
                self.industries.push(agent_task.task_type.clone().into());
                self.matched_agent_tasks.push(vec![agent_task]);
            }
        }
    }

    fn load_all_tasks(&mut self) {
        let tasks = load_mock_tasks();
        self._agent_tasks = tasks.into_iter().map(Rc::new).collect();
        self.prepare(self.query.clone());
    }

    fn extend_more(&mut self, _len: usize) {
        // For mock data, we just use the initial JSON load
        // If we want to support pagination/lazy loading, we could cycle through tasks
        // For now, just do nothing as all tasks are loaded initially
    }

    fn selected_agent_task(&self) -> Option<Rc<AgentTask>> {
        let Some(ix) = self.selected_index else {
            return None;
        };

        self.matched_agent_tasks
            .get(ix.section)
            .and_then(|c| c.get(ix.row))
            .cloned()
    }
}

impl ListDelegate for TaskListDelegate {
    type Item = TaskListItem;

    fn sections_count(&self, _: &App) -> usize {
        self.industries.len()
    }

    fn items_count(&self, section: usize, _: &App) -> usize {
        // Return 0 items if the section is collapsed
        if self.is_section_collapsed(section) {
            0
        } else {
            self.matched_agent_tasks[section].len()
        }
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.prepare(query.to_owned());
        Task::ready(())
    }

    fn confirm(&mut self, secondary: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        println!("Confirmed with secondary: {}", secondary);
        window.dispatch_action(Box::new(SelectedAgentTask), cx);
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }

    fn render_section_header(
        &self,
        section: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement> {
        let Some(task_type) = self.industries.get(section) else {
            return None;
        };

        let is_collapsed = self.is_section_collapsed(section);
        let collapsed_sections = self.collapsed_sections.clone();
        let list_state = self.list_state.clone();

        // Use ChevronRight when collapsed, ChevronDown when expanded
        let chevron_icon = if is_collapsed {
            IconName::ChevronRight
        } else {
            IconName::ChevronDown
        };

        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .pb_1()
                .px_2()
                .gap_2()
                .text_sm()
                .rounded(cx.theme().radius)
                // Left side: collapsible section header
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .flex_1()
                        .text_color(cx.theme().muted_foreground)
                        .cursor_default()
                        .hover(|style| style.bg(cx.theme().secondary))
                        .rounded(cx.theme().radius)
                        .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                            // Toggle the collapsed state
                            let mut collapsed = collapsed_sections.borrow_mut();
                            if collapsed.contains(&section) {
                                collapsed.remove(&section);
                            } else {
                                collapsed.insert(section);
                            }
                            drop(collapsed); // Release the borrow before updating

                            // Notify the list state to re-render
                            if let Some(list_state) = list_state.as_ref() {
                                _ = list_state.update(cx, |_, cx| {
                                    cx.notify();
                                });
                            }
                        })
                        .child(Icon::new(chevron_icon).size(px(14.)))
                        .child(Icon::new(IconName::Folder))
                        .child(task_type.clone()),
                )
                // Right side: add task button
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(20.))
                        .h(px(20.))
                        .rounded(px(4.))
                        .cursor_default()
                        .text_color(cx.theme().muted_foreground)
                        .hover(|style| {
                            style
                                .bg(cx.theme().accent)
                                .text_color(cx.theme().accent_foreground)
                        })
                        .on_mouse_down(MouseButton::Left, move |_, _window, _cx| {
                            println!("Add new task to section: {}", section);
                            // TODO: Implement add task functionality
                        })
                        .child(Icon::new(IconName::Plus).size(px(14.))),
                ),
        )
    }


    fn render_item(&self, ix: IndexPath, _: &mut Window, _: &mut App) -> Option<Self::Item> {
        let selected = Some(ix) == self.selected_index || Some(ix) == self.confirmed_index;
        if let Some(agent_task) = self.matched_agent_tasks[ix.section].get(ix.row) {
            return Some(TaskListItem::new(ix, agent_task.clone(), selected));
        }

        None
    }

    fn render_empty(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        // Check if we have sections but all are collapsed
        let has_collapsed_sections = !self.industries.is_empty()
            && self.industries.len() == self.collapsed_sections.borrow().len();

        if has_collapsed_sections {
            // Render section headers so user can expand them
            let collapsed_sections = self.collapsed_sections.clone();
            let list_state = self.list_state.clone();

            v_flex()
                .w_full()
                .gap_1()
                .children(self.industries.iter().enumerate().map(|(section, task_type)| {
                    let collapsed_sections = collapsed_sections.clone();
                    let list_state = list_state.clone();
                    let task_type = task_type.clone();

                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .pb_1()
                        .px_2()
                        .gap_2()
                        .text_sm()
                        .rounded(cx.theme().radius)
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .flex_1()
                                .text_color(cx.theme().muted_foreground)
                                .cursor_default()
                                .hover(|style| style.bg(cx.theme().secondary))
                                .rounded(cx.theme().radius)
                                .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                    // Expand the section
                                    collapsed_sections.borrow_mut().remove(&section);

                                    if let Some(list_state) = list_state.as_ref() {
                                        _ = list_state.update(cx, |_, cx| {
                                            cx.notify();
                                        });
                                    }
                                })
                                .child(Icon::new(IconName::ChevronRight).size(px(14.)))
                                .child(Icon::new(IconName::Folder))
                                .child(task_type),
                        )
                }))
                .into_any_element()
        } else {
            // Default empty state
            h_flex()
                .size_full()
                .justify_center()
                .text_color(cx.theme().muted_foreground.opacity(0.6))
                .child(Icon::new(IconName::Inbox).size_12())
                .into_any_element()
        }
    }

    fn loading(&self, _: &App) -> bool {
        self.loading
    }

    fn is_eof(&self, _: &App) -> bool {
        return !self.loading && !self.eof;
    }

    fn load_more_threshold(&self) -> usize {
        150
    }

    fn load_more(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        if !self.lazy_load {
            return;
        }

        cx.spawn_in(window, async move |view, window| {
            // Simulate network request, delay 1s to load data.
            Timer::after(Duration::from_secs(1)).await;

            _ = view.update_in(window, move |view, window, cx| {
                let query = view.delegate().query.clone();
                view.delegate_mut().extend_more(200);
                _ = view.delegate_mut().perform_search(&query, window, cx);
                view.delegate_mut().eof = view.delegate()._agent_tasks.len() >= 6000;
            });
        })
        .detach();
    }
}

pub struct ListTaskPanel {
    focus_handle: FocusHandle,
    task_list: Entity<ListState<TaskListDelegate>>,
    selected_agent_task: Option<Rc<AgentTask>>,
    _subscriptions: Vec<Subscription>,
}

impl super::DockPanel for ListTaskPanel {
    fn title() -> &'static str {
        "Tasks"
    }

    fn description() -> &'static str {
        "A list displays a series of items."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl ListTaskPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut delegate = TaskListDelegate {
            industries: vec![],
            matched_agent_tasks: vec![],
            _agent_tasks: vec![],
            selected_index: Some(IndexPath::default()),
            confirmed_index: None,
            query: "".into(),
            loading: false,
            eof: false,
            lazy_load: false,
            collapsed_sections: Rc::new(RefCell::new(HashSet::new())),
            list_state: None,
        };
        delegate.load_all_tasks();

        let task_list = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));

        // Set the weak reference to the list state in the delegate
        task_list.update(cx, |list, _| {
            list.delegate_mut().list_state = Some(task_list.downgrade());
        });

        let _subscriptions = vec![
            cx.subscribe_in(&task_list, window, |_this, _, ev: &ListEvent, window, cx| match ev {
                ListEvent::Select(ix) => {
                    println!("List Selected: {:?}", ix);
                }
                ListEvent::Confirm(ix) => {
                    println!("List Confirmed: {:?}", ix);
                    // Dispatch action to show conversation panel
                    window.dispatch_action(Box::new(ShowConversationPanel), cx);
                }
                ListEvent::Cancel => {
                    println!("List Cancelled");
                }
            }),
        ];

        // Spawn a background task to randomly update task status for demo
        cx.spawn(async move |this, cx| {
            this.update(cx, |this, cx| {
                this.task_list.update(cx, |picker, _| {
                    picker
                        .delegate_mut()
                        ._agent_tasks
                        .iter_mut()
                        .for_each(|agent_task| {
                            // Clone the task and update its status
                            let mut updated_task = (**agent_task).clone();
                            updated_task.status = random_status();
                            *agent_task = Rc::new(updated_task.prepare());
                        });
                    picker.delegate_mut().prepare("");
                });
                cx.notify();
            })
            .ok();
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            task_list,
            selected_agent_task: None,
            _subscriptions,
        }
    }

    fn selected_agent_task(
        &mut self,
        _: &SelectedAgentTask,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picker = self.task_list.read(cx);
        if let Some(agent_task) = picker.delegate().selected_agent_task() {
            self.selected_agent_task = Some(agent_task);
        }
    }

    /// Handle action to create a new task from the welcome panel
    fn on_create_task_from_welcome(
        &mut self,
        action: &CreateTaskFromWelcome,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task_name = action.0.to_string();

        // Create a new task with InProgress status
        let new_task = AgentTask {
            name: task_name,
            task_type: "Conversation".to_string(),
            add_new_code_lines: 0,
            delete_code_lines: 0,
            status: TaskStatus::InProgress,
            change_timestamp: 0,
            change_timestamp_str: "".into(),
            add_new_code_lines_str: "+0".into(),
            delete_code_lines_str: "-0".into(),
        }
        .prepare();

        // Add task to the beginning of the list
        self.task_list.update(cx, |list, cx| {
            let delegate = list.delegate_mut();
            delegate._agent_tasks.insert(0, Rc::new(new_task));
            delegate.prepare("");
            cx.notify();
        });
    }

    /// Handle click on "New Task" button - shows the welcome panel
    fn on_new_task_click(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Ensure this panel has focus before dispatching action
        window.focus(&self.focus_handle);
        window.dispatch_action(Box::new(ShowWelcomePanel), cx);
    }
}

/// Load mock agent tasks from JSON file
fn load_mock_tasks() -> Vec<AgentTask> {
    let json_data = include_str!("../mock_tasks.json");
    match serde_json::from_str::<Vec<AgentTask>>(json_data) {
        Ok(tasks) => tasks.into_iter().map(|task| task.prepare()).collect(),
        Err(e) => {
            eprintln!("Failed to load mock tasks: {}", e);
            Vec::new()
        }
    }
}

/// Generate a random task status for demo purposes
fn random_status() -> TaskStatus {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u8;
    match seed % 4 {
        0 => TaskStatus::Pending,
        1 => TaskStatus::InProgress,
        2 => TaskStatus::Completed,
        _ => TaskStatus::Failed,
    }
}

impl Focusable for ListTaskPanel {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ListTaskPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // let lazy_load = self.task_list.read(cx).delegate().lazy_load;

        v_flex()
            .child(
                Button::new("btn-new-task")
                    .label("New Task")
                    .primary()
                    .icon(Icon::new(IconName::Plus))
                    .on_click(cx.listener(Self::on_new_task_click)),
            )
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::selected_agent_task))
            .on_action(cx.listener(Self::on_create_task_from_welcome))
            .size_full()
            .gap_4()
            .child(
                List::new(&self.task_list)
                    .p(px(8.))
                    .flex_1()
                    .w_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius),
            )
    }
}
