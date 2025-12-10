//! Configuration File Watcher Service
//!
//! Monitors the agent configuration file for changes and triggers
//! automatic reloading of agent configurations.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use notify::{
    event::{EventKind, ModifyKind},
    Event, RecommendedWatcher, RecursiveMode, Watcher,
};
use tokio::sync::mpsc;

use crate::core::{agent::AgentManager, config::Config};

/// Configuration file watcher service
pub struct ConfigWatcher {
    config_path: PathBuf,
    agent_manager: Arc<AgentManager>,
}

impl ConfigWatcher {
    /// Create a new configuration watcher
    pub fn new(config_path: PathBuf, agent_manager: Arc<AgentManager>) -> Self {
        Self {
            config_path,
            agent_manager,
        }
    }

    /// Start watching the configuration file for changes
    pub async fn start_watching(self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(100);

        // Spawn file watcher in a separate thread
        let config_path = self.config_path.clone();
        let watcher_path = config_path.clone();

        std::thread::spawn(move || {
            if let Err(e) = Self::run_watcher(&watcher_path, tx) {
                log::error!("Config watcher error: {}", e);
            }
        });

        log::info!(
            "Started watching configuration file: {}",
            config_path.display()
        );

        // Process file change events
        while let Some(event) = rx.recv().await {
            if Self::should_reload(&event) {
                log::info!(
                    "Configuration file changed, reloading: {}",
                    config_path.display()
                );

                // Add a small delay to ensure the file is completely written
                tokio::time::sleep(Duration::from_millis(500)).await;

                // Reload configuration
                if let Err(e) = self.reload_config().await {
                    log::error!("Failed to reload configuration: {}", e);
                } else {
                    log::info!("Configuration reloaded successfully");
                }
            }
        }

        Ok(())
    }

    /// Run the file watcher
    fn run_watcher(path: &Path, tx: mpsc::Sender<Event>) -> Result<()> {
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
            match res {
                Ok(event) => {
                    let _ = tx.blocking_send(event);
                }
                Err(e) => {
                    log::error!("Watch error: {:?}", e);
                }
            }
        })
        .context("Failed to create file watcher")?;

        // Watch the parent directory since watching a file directly can be unreliable
        let watch_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        watcher
            .watch(&watch_path, RecursiveMode::NonRecursive)
            .context("Failed to watch configuration directory")?;

        log::info!("File watcher started for: {}", watch_path.display());

        // Keep the watcher alive
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    /// Determine if we should reload based on the event
    fn should_reload(event: &Event) -> bool {
        match event.kind {
            EventKind::Modify(ModifyKind::Data(_)) => true,
            EventKind::Create(_) => true,
            _ => false,
        }
    }

    /// Reload the configuration file and update agent manager
    async fn reload_config(&self) -> Result<()> {
        log::info!("Reading configuration from: {}", self.config_path.display());

        // Read and parse configuration file
        let config_content = std::fs::read_to_string(&self.config_path)
            .with_context(|| format!("Failed to read {}", self.config_path.display()))?;

        let config: Config = serde_json::from_str(&config_content)
            .with_context(|| format!("Invalid config at {}", self.config_path.display()))?;

        // Reload agent manager with new configuration
        self.agent_manager
            .reload_config(config.agent_servers)
            .await
            .context("Failed to reload agent configuration")?;

        log::info!("Agent configuration reloaded successfully");
        Ok(())
    }
}
