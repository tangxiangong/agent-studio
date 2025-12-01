use std::sync::Arc;

/// Permission request event that can be broadcast to subscribers
#[derive(Clone, Debug)]
pub struct PermissionRequestEvent {
    /// Unique permission request ID from PermissionStore
    pub permission_id: String,
    /// Session ID for this permission request
    pub session_id: String,
    /// Agent name requesting permission
    pub agent_name: String,
    /// Tool call details
    pub tool_call: agent_client_protocol::ToolCallUpdate,
    /// Available permission options
    pub options: Vec<agent_client_protocol::PermissionOption>,
}

/// Global event bus for permission requests
pub struct PermissionBus {
    subscribers: Vec<Box<dyn Fn(&PermissionRequestEvent) + Send + Sync>>,
}

impl PermissionBus {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Subscribe to permission requests
    pub fn subscribe<F>(&mut self, callback: F)
    where
        F: Fn(&PermissionRequestEvent) + Send + Sync + 'static,
    {
        self.subscribers.push(Box::new(callback));
    }

    /// Publish a permission request to all subscribers
    pub fn publish(&self, event: PermissionRequestEvent) {
        for subscriber in &self.subscribers {
            subscriber(&event);
        }
    }
}

impl Default for PermissionBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Global container for the permission bus
pub struct PermissionBusContainer {
    inner: Arc<std::sync::Mutex<PermissionBus>>,
}

impl PermissionBusContainer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(std::sync::Mutex::new(PermissionBus::new())),
        }
    }

    pub fn subscribe<F>(&self, callback: F)
    where
        F: Fn(&PermissionRequestEvent) + Send + Sync + 'static,
    {
        if let Ok(mut bus) = self.inner.lock() {
            bus.subscribe(callback);
        }
    }

    pub fn publish(&self, event: PermissionRequestEvent) {
        if let Ok(bus) = self.inner.lock() {
            bus.publish(event);
        }
    }
}

impl Default for PermissionBusContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for PermissionBusContainer {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
