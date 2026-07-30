# Bimo — Architecture & Implementation Plan

## Overview

Bimo is a coding agent. The project is being refactored from a monolithic legacy
codebase (`legacy/`) into a multi-crate workspace (`src/`):

```
bimo/
├── src/
│   ├── core/      # bimo_core  — library crate (no binary)
│   ├── api/       # bimo_api   — HTTP API (axum server, depends on core)
│   └── rpc/       # bimo_rpc   — JSON-RPC / IPC (depends on core)
├── legacy/        # old monolithic code, kept for reference
└── AGENTS.md      # this file
```

## Phase 1: Core library (`src/core/`) — FIRST PRIORITY

The core crate must be a `lib` crate (not a binary). It provides all domain
logic but no transport. The API and RPC crates depend on it.

### 1.1 Error model (`error.rs`)

**Keep** the unified `BimoError` enum approach from `legacy/src/error.rs`.
Variants needed:

- `Config` — config file read/write/parse failures
- `Provider` — provider resolution, API key missing
- `Model` — model selection, unknown model
- `Session` — session operations (load/save/delete)
- `Network` — HTTP failures, timeouts
- `Api` — generic API errors
- `Serialization` — JSON parse/serialize errors

Drop `Command` (moves to client) and `NotImplemented`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum BimoError { ... }
pub type Result<T> = std::result::Result<T, BimoError>;
```

### 1.2 Configuration (`config/`)

**Replace** the single `config.json` with multiple files in `~/.config/bimo/`:

- `providers.json` — stored provider configs (base URLs, API keys)
- `settings.json` — general settings (selected provider, selected model, thinking config)

Types to keep from legacy:

- `ProviderPersistedConfig` (base_url, api_key)
- `ThinkingConfig` (enabled, budget_tokens, reasoning_effort)
- `CustomProviderConfig`

### 1.3 Provider system (`provider/`)

**Simplify** to exactly 2 request body formats:

- `OpenAi` — OpenAI-compatible (covers OpenAI, OpenRouter, Ollama, LM Studio, etc.)
- `Anthropic` — Anthropic-compatible

Drop `Ollama` format entirely. Built-in providers should be:

- `openai` (requires_api_key, OpenAi format)
- `anthropic` (requires_api_key, Anthropic format)
- `openrouter` (requires_api_key, OpenAi format)
- `lmstudio` (no key, OpenAi format)
- Custom providers (user-defined, default to OpenAi format)

`ProviderRuntime` carries base_url, api_key, chat_endpoint, models_endpoint,
auth_header, auth_prefix, and request_body_format.

HTTP module (`provider/http.rs`):

- `fetch_models()` — GET models endpoint, parse response
- `chat_completion_streaming()` — POST streaming chat, return event stream
- `build_request_body()` — build JSON body per format
- `parse_chat_response()` — parse non-streaming response
- `extract_stream_delta()` — extract text delta from stream chunk

No non-streaming chat function in the core (only streaming).

### 1.4 Model system (`model.rs`)

**Keep** `ModelInfo`, `fetch_models_for_provider()`, `lookup_known_context_window()`.

### 1.5 Session system (`session/`)

**Key change:** Do NOT persist empty sessions on creation. Only save when
messages have actually been added (`dirty` flag).

Types:

- `Role` — System, User, Assistant, Tool
- `Message` — role, content, timestamp, model, provider, estimated_tokens
- `Session` — id, messages, todos, created_at, updated_at, dirty flag
- `SessionInfo` — summary for listing

Methods on `Session`:

- `new()` — creates in-memory only, no disk write, dirty=false
- `add_user_message()`, `add_assistant_message()`, `add_system_message()`, `add_tool_message()`
- `to_chat_messages()` — convert to provider ChatMessage format
- `clear()`, `message_count()`, `info()`
- `fork(index)` — fork at message index, saves new session
- `revert(index)` — truncate to index, marks dirty
- `compact(summary)` — replace non-system messages with summary
- `save()` — only writes to disk if dirty (or always, but dirty tracks it)
- `load(id)` — load from disk
- `list_saved()` — list all saved sessions
- `delete_saved(id)`, `delete_all_saved()`

**No SessionManager in core** — that's a client-side concern for multi-session
management. Core just gives you the Session type and persistence methods.

### 1.6 Tool system (`tool/`)

**Simplify** to exactly 5 tools:

| Tool          | Parameters                                                    | Description             |
| ------------- | ------------------------------------------------------------- | ----------------------- |
| `read_file`   | path (required)                                               | Read file contents      |
| `edit_file`   | path (required), old_string (required), new_string (required) | Search-and-replace edit |
| `write_file`  | path (required), content (required)                           | Write full file content |
| `run_command` | command (required), timeout (optional)                        | Execute shell command   |
| `manage_todo` | action (required), id, description, status                    | Manage todo list        |

Use a proper tool call parsing library instead of custom regex. The `tool-parser`
crate supports XML tool blocks and JSON tool calls out of the box. The core
provides:

- `Tool` / `ToolParameter` types (metadata for prompt rendering)
- `ToolRegistry` (list, register, render XML)
- Tool execution functions (async)

Important: The tool execution functions should be modular enough that the
client (API/RPC/TUI) can provide its own implementations or override them.

### 1.7 Todo system (`todo.rs`)

**Keep** `TodoList`, `TodoItem`, `TodoStatus` as-is from legacy. It's clean.

### 1.8 Chat agent (`agent.rs`)

**Core chat function** — only streaming:

```rust
pub async fn chat_stream(
    &mut self,
    user_message: &str,
    tx: tokio::sync::mpsc::Sender<ChatStreamEvent>,
) -> Result<()>
```

Tool calling loop (max iterations per request should be disable by default and enabled or changed in the settings):

1. Send messages to provider
2. Stream response deltas to channel
3. Parse tool calls from accumulated response
4. If no tool calls → emit Done event, return
5. Execute each tool call, emit ToolStart/ToolResult events
6. Add tool results to session, loop back to 1

`ChatStreamEvent` enum:

- `Content { delta: String }`
- `ToolStart { tool: String, args: Option<Value> }`
- `ToolResult { tool: String, is_error: bool }`
- `Done { model, usage, session_id }`
- `Error { message: String }`

### 1.9 Prompt system (`prompts/`)

**Keep** the external `.md` template approach with `{{PLACEHOLDER}}` syntax.

- `SYSTEM.md` — system prompt template (placeholders: TOOLS, DATE, CWD, PROJECT_CONTEXT)
- `COMPACT.md` — session summarization prompt (placeholder: CONVERSATION)
- `COMPACT_PREFIX.md` — compacted context prefix (placeholder: SUMMARY) *Should be renamed into SUMARY.md

Prompt loading should check:

1. `BIMO_PROMPTS_DIR` env var
2. `~/.config/bimo/prompts/` into the home directory
3. `./agents/prompts/` relative to the session project directory
4. `~/.agents/prompts/` into the home directory
5. `./prompts/` relative to CWD
6. Built-in embedded defaults

### 1.10 Project context (`context.rs`)

**Move** `build_project_context()` and `load_agent_instructions()` to core as
a utility module. Includes git branch detection, directory listing, and
agent instruction file loading (AGENTS.md, CLAUDE.md, etc.).

### 1.11 Core crate structure

```
src/core/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── config/
│   │   ├── mod.rs
│   │   ├── providers.rs       # per-provider persisted config
│   │   └── settings.rs        # global settings (selected provider/model, thinking)
│   ├── provider/
│   │   ├── mod.rs
│   │   ├── types.rs           # ProviderInfo, ProviderRuntime, RequestBodyFormat, etc.
│   │   ├── registry.rs        # built-in + custom provider registry
│   │   └── http.rs            # HTTP client for providers
│   ├── model.rs
│   ├── session/
│   │   ├── mod.rs             # Session, Message, Role
│   │   └── persistence.rs     # save/load/list/delete
│   ├── tool/
│   │   ├── mod.rs             # Tool, ToolParameter, ToolRegistry
│   │   └── execute.rs         # tool execution functions
│   ├── todo.rs
│   ├── prompts.rs             # prompt loading + rendering
│   ├── context.rs             # project context builder
│   └── agent.rs               # CoreAgent — chat, tool loop, session management
```

### 1.12 Key dependencies for core

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json", "stream"] }
thiserror = "2"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
dirs = "6"
regex = "1"
futures-util = "0.3"
tool-parser = "0.5"
async-stream = "0.3"
```

## Phase 2: API crate (`src/api/`) — LATER

HTTP server (axum) wrapping core. Same API surface as legacy main.rs but
cleaner, with the core doing all the work.

## Phase 3: RPC crate (`src/rpc/`) — LATER

JSON-RPC or gRPC transport wrapping core.

## Phase 4: TUI / GUI — LATER

Client applications using core as a library. Slash commands live here.

---

## Implementation order (Phase 1)

1. Set up `core` as lib crate (change from binary to library)
2. `error.rs` — BimoError enum
3. `config/` — settings, provider configs, filesystem paths
4. `provider/types.rs` — ProviderInfo, ProviderRuntime, RequestBodyFormat
5. `provider/registry.rs` — built-in providers
6. `provider/http.rs` — HTTP calls (fetch models, streaming chat)
7. `model.rs` — ModelInfo, context window lookup
8. `session/` — Session, Message, Role, persistence
9. `todo.rs` — TodoList
10. `tool/` — Tool types, registry, execution
11. `prompts.rs` — loading and rendering
12. `context.rs` — project context builder
13. `agent.rs` — CoreAgent with streaming chat + tool loop
14. Wire everything in `lib.rs`
15. Remove `legacy/src/main.rs` temporary binary in core
16. Test the core with `cargo test -p bimo_core`
