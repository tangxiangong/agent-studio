# Agent Studio

An Agent Studio built with gpui-component to create a window with a custom title bar. This application provides a comprehensive environment for interacting with AI agents, featuring a dock-based interface, code editing capabilities, and conversation management.

## Features

- **Dock-based Interface**: Flexible panel layout with support for multiple views
- **Code Editor**: Integrated code editing with language server protocol (LSP) support
- **AI Agent Integration**: Connect and interact with multiple AI agents
- **Conversation Management**: Rich conversation interface with message history
- **Task Management**: Track and manage tasks within the application
- **Custom Title Bar**: Native-appearing custom window controls
- **Theme Support**: Multiple color themes with system integration

## Installation

### Prerequisites

- Rust (latest stable version)
- Cargo

### Building from Source

1. Clone the repository:
```bash
git clone https://github.com/sxhxliang/agent-studio.git
cd agent-studio
```

2. Build the application:
```bash
cargo build --release
```

3. Run the application:
```bash
cargo run
```

## Usage

The application can be configured using a configuration file. By default, it looks for `config.json` in the project root directory. You can specify a custom config path using command line arguments.

The main interface includes:
- Left panel: Code editor with LSP features
- Right panel: Conversation history and chat input
- Bottom panel: Task management and agent interactions

## Project Structure

```
src/
├── acp_client.rs          # Agent Client Protocol client
├── app/                   # Application-level components
│   ├── app_state.rs       # Global application state
│   └── ...                # Menu, themes, title bar
├── chat_input.rs          # Chat input component
├── code_editor.rs         # Code editor with LSP integration
├── components/            # Reusable UI components
│   ├── agent_message.rs   # Agent message display
│   ├── user_message.rs    # User message display
│   └── ...                # Other components
├── conversation.rs        # Conversation management
├── conversation_acp.rs    # ACP conversation handling
├── dock_panel.rs          # Dock panel system
├── settings_window.rs     # Settings window implementation
├── task_list.rs           # Task list management
├── task_turn_view.rs      # Task turn view component
├── welcome_panel.rs       # Welcome panel
├── workspace.rs           # Main workspace management
└── ...                    # Other utility files
```

## Configuration

The application can be configured through a `config.json` file. Example configuration:

```json
{
  "agent_servers": [
    {
      "name": "example-agent",
      "url": "http://localhost:8080"
    }
  ]
}
```

## Dependencies

- [gpui](https://github.com/zed-industries/zed): Zed's native GUI library
- [gpui-component](https://github.com/sxhxliang/gpui-component): UI components library
- [agent-client-protocol](https://crates.io/crates/agent-client-protocol): Agent communication protocol
- [tokio](https://tokio.rs/): Async runtime
- [serde](https://serde.rs/): Serialization framework
- [lsp-types](https://crates.io/crates/lsp-types): Language Server Protocol types

## License

MIT License. See the [LICENSE](LICENSE) file for details.