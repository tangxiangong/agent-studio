use std::sync::Arc;

use gpui::{App, AppContext, Entity, Global, SharedString};

use crate::{
    acp_client::{AgentManager, PermissionStore},
    session_bus::SessionUpdateBusContainer,
    permission_bus::PermissionBusContainer,
};

pub struct AppState {
    pub invisible_panels: Entity<Vec<SharedString>>,
    agent_manager: Option<Arc<AgentManager>>,
    permission_store: Option<Arc<PermissionStore>>,
    pub session_bus: SessionUpdateBusContainer,
    pub permission_bus: PermissionBusContainer,
}

impl AppState {
    pub fn init(cx: &mut App) {
        let state = Self {
            invisible_panels: cx.new(|_| Vec::new()),
            agent_manager: None,
            permission_store: None,
            session_bus: SessionUpdateBusContainer::new(),
            permission_bus: PermissionBusContainer::new(),
        };
        cx.set_global::<AppState>(state);
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Set the AgentManager after async initialization
    pub fn set_agent_manager(&mut self, manager: Arc<AgentManager>) {
        log::info!(
            "Setting AgentManager with {} agents",
            manager.list_agents().len()
        );
        self.agent_manager = Some(manager);
    }

    /// Set the PermissionStore
    pub fn set_permission_store(&mut self, store: Arc<PermissionStore>) {
        log::info!("Setting PermissionStore");
        self.permission_store = Some(store);
    }

    /// Get a reference to the AgentManager if initialized
    pub fn agent_manager(&self) -> Option<&Arc<AgentManager>> {
        self.agent_manager.as_ref()
    }

    /// Get the PermissionStore if set
    pub fn permission_store(&self) -> Option<&Arc<PermissionStore>> {
        self.permission_store.as_ref()
    }
}
impl Global for AppState {}
