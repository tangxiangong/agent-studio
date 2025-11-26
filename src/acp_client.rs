//! A CLI interactive execution program that can talk to multiple ACP agents concurrently.
//!
//! The service reads `config.json` (configurable via `--config`) to determine
//! which agent binaries to spawn, and provides a REPL to interact with them.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
};

use agent_client_protocol::{self as acp, Agent as _};
use anyhow::{Context, Result, anyhow};
use log::{error, warn};
use serde::Deserialize;
use tokio::{
    runtime::Builder as RuntimeBuilder,
    sync::{RwLock, mpsc, oneshot},
    task::LocalSet,
};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {

    let settings = Settings::parse()?;
    let config: Config = {
        let raw = std::fs::read_to_string(&settings.config_path)
            .with_context(|| format!("failed to read {}", settings.config_path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("invalid config at {}", settings.config_path.display()))?
    };

    let permission_store = Arc::new(PermissionStore::default());
    let manager = AgentManager::initialize(config.agent_servers.clone(), permission_store.clone()).await?;

    println!("Loaded {} agents.", manager.list_agents().len());
    println!("Type '/help' for available commands.");

    let mut active_agent: Option<String> = manager.list_agents().first().cloned();
    let mut active_sessions: HashMap<String, String> = HashMap::new(); // Agent -> SessionId

    if let Some(ref agent) = active_agent {
        println!("Active agent set to: {}", agent);
    }

    use std::io::{self, Write};
    let mut input_buffer = String::new();
    
    loop {
        let prompt = if let Some(ref agent) = active_agent {
            let session = active_sessions.get(agent).map(|s| s.as_str()).unwrap_or("no-session");
            format!("[{} : {}]> ", agent, session)
        } else {
            "[no-agent]> ".to_string()
        };

        print!("{}", prompt);
        io::stdout().flush()?;

        input_buffer.clear();
        match io::stdin().read_line(&mut input_buffer) {
            Ok(0) => {
                println!("CTRL-D");
                break;
            }
            Ok(_) => {
                let line = input_buffer.trim();
                if line.is_empty() {
                    continue;
                }

                if line.starts_with("/") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    match parts[0] {
                        "/help" => {
                            println!("Commands:");
                            println!("  /agent <name>              - Switch active agent");
                            println!("  /agents                    - List available agents");
                            println!("  /session new               - Create new session for active agent");
                            println!("  /session switch <id>       - Switch active session for active agent");
                            println!("  /decide <req_id> <opt_id>  - Approve permission request with option");
                            println!("  /quit                      - Exit");
                        }
                        "/quit" => break,
                        "/agents" => {
                            let agents = manager.list_agents();
                            for agent in agents {
                                let prefix = if Some(&agent) == active_agent.as_ref() { "*" } else { " " };
                                println!("{} {}", prefix, agent);
                            }
                        }
                        "/agent" => {
                            if parts.len() < 2 {
                                println!("Usage: /agent <name>");
                            } else {
                                let name = parts[1].to_string();
                                if manager.get(&name).is_some() {
                                    active_agent = Some(name);
                                    println!("Switched to agent: {}", parts[1]);
                                } else {
                                    println!("Agent not found: {}", name);
                                }
                            }
                        }
                        "/session" => {
                            if let Some(ref agent_name) = active_agent {
                                if let Some(agent_handle) = manager.get(agent_name) {
                                    if parts.len() >= 2 {
                                        match parts[1] {
                                            "new" => {
                                                match agent_handle.new_session(acp::NewSessionRequest {
                                                    cwd: std::env::current_dir()?,
                                                    mcp_servers: vec![],
                                                    meta: None,
                                                }).await {
                                                    Ok(resp) => {
                                                        let sid = resp.session_id.to_string();
                                                        println!("Created session: {}", sid);
                                                        active_sessions.insert(agent_name.clone(), sid);
                                                    }
                                                    Err(e) => println!("Error creating session: {}", e),
                                                }
                                            }
                                            "switch" => {
                                                if parts.len() < 3 {
                                                    println!("Usage: /session switch <id>");
                                                } else {
                                                    let sid = parts[2].to_string();
                                                    active_sessions.insert(agent_name.clone(), sid);
                                                    println!("Switched to session: {}", parts[2]);
                                                }
                                            }
                                            _ => println!("Unknown session command. Use 'new' or 'switch'."),
                                        }
                                    } else {
                                        println!("Usage: /session <new|switch>");
                                    }
                                }
                            } else {
                                println!("No active agent selected.");
                            }
                        }
                        "/decide" => {
                            if parts.len() < 3 {
                                println!("Usage: /decide <req_id> <opt_id>");
                            } else {
                                let req_id = parts[1];
                                let opt_id = parts[2];
                                if let Some(pending) = permission_store.remove(req_id).await {
                                    let response = acp::RequestPermissionResponse {
                                        outcome: acp::RequestPermissionOutcome::Selected { 
                                            option_id: acp::PermissionOptionId(Arc::from(opt_id)) 
                                        },
                                        meta: None,
                                    };
                                    if let Err(_) = pending.responder.send(response) {
                                        println!("Failed to send response (channel closed)");
                                    } else {
                                        println!("Permission {} decided with {}", req_id, opt_id);
                                    }
                                } else {
                                    println!("Permission Request ID not found: {}", req_id);
                                }
                            }
                        }
                        _ => println!("Unknown command: {}", parts[0]),
                    }
                } else {
                    // Send as prompt
                    if let Some(ref agent_name) = active_agent {
                        if let Some(session_id) = active_sessions.get(agent_name) {
                            if let Some(agent_handle) = manager.get(agent_name) {
                                let req = acp::PromptRequest {
                                    session_id: acp::SessionId::from(session_id.clone()),
                                    prompt: vec![line.to_string().into()],
                                    meta: None,
                                };
                                
                                match agent_handle.prompt(req).await {
                                    Ok(_) => {
                                        // Output is handled by CliClient printing to stdout
                                    }
                                    Err(e) => println!("Error sending prompt: {}", e),
                                }
                            }
                        } else {
                            println!("No active session. Use '/session new' to create one.");
                        }
                    } else {
                        println!("No active agent. Use '/agent <name>' to select one.");
                    }
                }
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

pub struct Settings {
    config_path: PathBuf,
}

impl Settings {
    fn parse() -> Result<Self> {
        let mut config_path = PathBuf::from("config.json");
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--config" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--config requires a value"))?;
                    config_path = PathBuf::from(value);
                }
                other => return Err(anyhow!("unknown flag: {other}")),
            }
        }
        Ok(Self { config_path })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    agent_servers: HashMap<String, AgentProcessConfig>,
    #[serde(default = "default_upload_dir")]
    upload_dir: PathBuf,
}

fn default_upload_dir() -> PathBuf {
    PathBuf::from(".")
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentProcessConfig {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

#[derive(Clone)]
pub struct AgentManager {
    agents: HashMap<String, Arc<AgentHandle>>,
}

impl AgentManager {
    async fn initialize(
        configs: HashMap<String, AgentProcessConfig>,
        permission_store: Arc<PermissionStore>,
    ) -> Result<Arc<Self>> {
        if configs.is_empty() {
            return Err(anyhow!("no agents defined in config"));
        }
        let mut agents = HashMap::new();
        for (name, cfg) in configs {
            let handle = AgentHandle::spawn(name.clone(), cfg, permission_store.clone()).await?;
            agents.insert(name, Arc::new(handle));
        }
        Ok(Arc::new(Self { agents }))
    }

    fn list_agents(&self) -> Vec<String> {
        let mut list = self.agents.keys().cloned().collect::<Vec<_>>();
        list.sort();
        list
    }

    fn get(&self, name: &str) -> Option<Arc<AgentHandle>> {
        self.agents.get(name).cloned()
    }
}

pub struct AgentHandle {
    name: String,
    sender: mpsc::Sender<AgentCommand>,
}

impl AgentHandle {
    async fn spawn(
        name: String,
        config: AgentProcessConfig,
        permission_store: Arc<PermissionStore>,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel(32);
        let (ready_tx, ready_rx) = oneshot::channel();
        let thread_name = format!("agent-worker-{name}");
        let worker_name = name.clone();
            thread::Builder::new()
                    .name(thread_name)
                    .spawn(move || {
                        let log_name = worker_name.clone();
                        if let Err(err) =
                            run_agent_worker(worker_name, config, permission_store, receiver, ready_tx)
                        {
                            error!("agent {log_name} exited with error: {:?}", err);
                        }
                    })
                    .context("failed to spawn worker thread")?;
        let start_name = name.clone();
        ready_rx
            .await
            .map_err(|_| anyhow!("agent {start_name} failed to start"))??;

        Ok(Self { name, sender })
    }

    async fn new_session(
        &self,
        request: acp::NewSessionRequest,
    ) -> Result<acp::NewSessionResponse> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(AgentCommand::NewSession {
                request,
                respond: tx,
            })
            .await
            .map_err(|_| anyhow!("agent {} is not running", self.name))?;
        let result = rx
            .await
            .map_err(|_| anyhow!("agent {} stopped", self.name))?;
        result
    }

    async fn prompt(&self, request: acp::PromptRequest) -> Result<acp::PromptResponse> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(AgentCommand::Prompt {
                request,
                respond: tx,
            })
            .await
            .map_err(|_| anyhow!("agent {} is not running", self.name))?;
        let result = rx
            .await
            .map_err(|_| anyhow!("agent {} stopped", self.name))?;
        result
    }
}

enum AgentCommand {
    Initialize {
        request: acp::InitializeRequest,
        respond: oneshot::Sender<Result<acp::InitializeResponse>>,
    },
    NewSession {
        request: acp::NewSessionRequest,
        respond: oneshot::Sender<Result<acp::NewSessionResponse>>,
    },
    Prompt {
        request: acp::PromptRequest,
        respond: oneshot::Sender<Result<acp::PromptResponse>>,
    },
}

fn run_agent_worker(
    agent_name: String,
    config: AgentProcessConfig,
    permission_store: Arc<PermissionStore>,
    command_rx: mpsc::Receiver<AgentCommand>,
    ready_tx: oneshot::Sender<Result<()>>,
) -> Result<()> {
    let runtime = RuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build runtime")?;

    runtime.block_on(async move {
        let local_set = LocalSet::new();
        local_set
            .run_until(
                agent_event_loop(
                    agent_name, config, permission_store, command_rx, ready_tx,
                ),
            )
            .await
    })
}

async fn agent_event_loop(
    agent_name: String,
    config: AgentProcessConfig,
    permission_store: Arc<PermissionStore>,
    mut command_rx: mpsc::Receiver<AgentCommand>,
    ready_tx: oneshot::Sender<Result<()>>,
) -> Result<()> {
    let mut command = tokio::process::Command::new(&config.command);
    command.args(&config.args);
    command.envs(&config.env);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::inherit());

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn agent {agent_name}"))?;
    let outgoing = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("agent {agent_name} missing stdin"))? 
        .compat_write();
    let incoming = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("agent {agent_name} missing stdout"))? 
        .compat();

    let client = CliClient::new(agent_name.clone(), permission_store);
    let (conn, io_task) = acp::ClientSideConnection::new(client, outgoing, incoming, |fut| {
        tokio::task::spawn_local(fut);
    });

    let io_handle = tokio::task::spawn_local(async move {
        if let Err(err) = io_task.await {
            warn!("agent I/O task ended: {:?}", err);
        }
    });

    let init_result = conn
        .initialize(acp::InitializeRequest {
            protocol_version: acp::V1,
            client_capabilities: acp::ClientCapabilities::default(),
            client_info: Some(acp::Implementation {
                name: "cli-client".into(),
                title: Some("CLI Client".into()),
                version: env!("CARGO_PKG_VERSION").into(),
            }),
            meta: None,
        })
        .await;

    match init_result {
        Ok(_) => {
            let _ = ready_tx.send(Ok(()));
        }
        Err(err) => {
            let message = format!("failed to initialize agent {agent_name}: {:?}", err);
            let _ = ready_tx.send(Err(anyhow!(message.clone())));
            return Err(anyhow!(message));
        }
    }

    while let Some(command) = command_rx.recv().await {
        match command {
            AgentCommand::Initialize { request, respond } => {
                let result = conn.initialize(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::NewSession { request, respond } => {
                let result = conn.new_session(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::Prompt { request, respond } => {
                let result = conn.prompt(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
        }
    }

    drop(conn);
    let _ = io_handle.await;
    if child.id().is_some() {
        let _ = child.kill().await;
    }
    Ok(())
}

pub struct CliClient {
    agent_name: String,
    permission_store: Arc<PermissionStore>,
}

impl CliClient {
    fn new(agent_name: String, permission_store: Arc<PermissionStore>) -> Self {
        Self {
            agent_name,
            permission_store,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl acp::Client for CliClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        let (tx, rx) = oneshot::channel();
        let id = self.permission_store.add(self.agent_name.clone(), args.session_id.to_string(), tx).await;
        
        println!("\n[PERMISSION REQUEST] Agent '{}' session '{}'", self.agent_name, args.session_id);
        
        if let Some(title) = &args.tool_call.fields.title {
             println!("  Action: {}", title);
        }
        if let Some(locations) = &args.tool_call.fields.locations {
            for loc in locations {
                println!("  Location: {:?}", loc.path);
            }
        }
        
        println!("Options:");
        for opt in &args.options {
            println!("  [{}] {}", opt.id.0, opt.name);
        }
        
        println!("To select an option, type: /decide {} <option_id>", id);

        rx.await.map_err(|_| {
            acp::Error::internal_error().with_data("permission request channel closed")
        })
    }

    async fn write_text_file(
        &self,
        _args: acp::WriteTextFileRequest,
    ) -> acp::Result<acp::WriteTextFileResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn read_text_file(
        &self,
        _args: acp::ReadTextFileRequest,
    ) -> acp::Result<acp::ReadTextFileResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn create_terminal(
        &self,
        _args: acp::CreateTerminalRequest,
    ) -> Result<acp::CreateTerminalResponse, acp::Error> {
        Err(acp::Error::method_not_found())
    }

    async fn terminal_output(
        &self,
        _args: acp::TerminalOutputRequest,
    ) -> acp::Result<acp::TerminalOutputResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn release_terminal(
        &self,
        _args: acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn wait_for_terminal_exit(
        &self,
        _args: acp::WaitForTerminalExitRequest,
    ) -> acp::Result<acp::WaitForTerminalExitResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn kill_terminal_command(
        &self,
        _args: acp::KillTerminalCommandRequest,
    ) -> acp::Result<acp::KillTerminalCommandResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn session_notification(
        &self,
        args: acp::SessionNotification,
    ) -> acp::Result<(), acp::Error> {
        match args.update {
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk { content, .. }) => {
                 let text = match content {
                    acp::ContentBlock::Text(text_content) => text_content.text,
                    acp::ContentBlock::Image(_) => "<image>".into(),
                    acp::ContentBlock::Audio(_) => "<audio>".into(),
                    acp::ContentBlock::ResourceLink(resource_link) => resource_link.uri,
                    acp::ContentBlock::Resource(_) => "<resource>".into(),
                };
                println!("\n| [{}] {}", self.agent_name, text);
            }
            _ => {}
        }
        Ok(())
    }

    async fn ext_method(&self, _args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn ext_notification(&self, _args: acp::ExtNotification) -> acp::Result<()> {
        Ok(())
    }
}

pub struct PendingPermission {
    agent: String,
    session_id: String,
    responder: oneshot::Sender<acp::RequestPermissionResponse>,
}

#[derive(Default)]
pub struct PermissionStore {
    pending: RwLock<HashMap<String, PendingPermission>>,
    next_id: AtomicU64,
}

impl PermissionStore {
    async fn add(&self, agent: String, session_id: String, responder: oneshot::Sender<acp::RequestPermissionResponse>) -> String {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst).to_string();
        self.pending.write().await.insert(id.clone(), PendingPermission {
            agent,
            session_id,
            responder,
        });
        id
    }

    async fn remove(&self, id: &str) -> Option<PendingPermission> {
        self.pending.write().await.remove(id)
    }
}