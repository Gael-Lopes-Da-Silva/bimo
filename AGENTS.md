# Bimo — Architecture & Implementation Plan

## Overview

Bimo is a coding agent. The project is being refactored from a monolithic legacy
codebase (`legacy/`) into a multi-crate workspace (`src/`):

```
bimo/
├── src/
│   ├── core/      # bimo_core  — library crate (no binary) — COMPLETE
│   ├── api/       # bimo_api   — HTTP API (axum server, depends on core)
│   └── rpc/       # bimo_rpc   — JSON-RPC / IPC (depends on core)
├── legacy/        # old monolithic code, kept for reference
└── AGENTS.md      # this file
```

## Phase 1: Core library (`src/core/`) — COMPLETE

### 1.1 Error model (`error.rs`)

Unified `BimoError` enum with 7 variants: `Config`, `Provider`, `Model`,
`Session`, `Network`, `Api`, `Serialization`. Uses `thiserror` for Display
impl. Clone + Serialize/Deserialize. `Result<T>` type alias.

### 1.2 Configuration (`config/`)

Two JSON files in `~/.config/bimo/`:

- `providers.json` — per-provider persisted config (base URLs, API keys),
  custom provider definitions
- `settings.json` — general settings (selected provider, selected model,
  thinking config, max tool iterations)

Key types: `Settings`, `ThinkingConfig`, `ProvidersConfig`,
`ProviderPersistedConfig`, `CustomProviderConfig`.

### 1.3 Provider system (`provider/`)

Two request body formats: `OpenAi` and `Anthropic`.

Built-in providers:

- `openai` (requires_api_key, OpenAi format)
- `anthropic` (requires_api_key, Anthropic format)
- `openrouter` (requires_api_key, OpenAi format)
- `lmstudio` (no key, OpenAi format)
- Custom providers (user-defined, default to OpenAi format)

Key types: `ProviderInfo`, `ProviderRuntime` (base_url, api_key,
chat_endpoint, models_endpoint, auth_header, auth_prefix,
request_body_format), `RequestBodyFormat`, `ChatMessage`,
`ChatCompletionResponse`, `RawModel`, `UsageInfo`, `ProviderRegistry`.

HTTP functions:

- `fetch_models()` — GET models endpoint, parse response
- `chat_completion_streaming()` — POST streaming chat, return SSE event
  stream
- `build_request_body()` — build JSON body per format
- `parse_chat_response()` — parse non-streaming response
- `extract_stream_delta()` — extract text delta from stream chunk

No non-streaming chat in core (only streaming).

### 1.4 Model system (`model.rs`)

`ModelInfo` with id, name, tier, context_window.
`fetch_models_for_provider()` fetches and enriches with known context
windows. `lookup_known_context_window()` covers Claude, GPT-4/4o/4.1/4.5,
o-series, Gemini, Llama, DeepSeek, Mistral, Qwen, Command-R.

### 1.5 Session system (`session/`)

Dirty-flag based persistence — sessions not saved on creation, only when
messages are added.

Types: `Role` (System, User, Assistant, Tool), `Message`, `Session` (with
dirty flag), `SessionInfo`.

Methods on `Session`: `new()`, `add_user_message()`,
`add_assistant_message()`, `add_assistant_response()`,
`add_system_message()`, `add_tool_message()`, `to_chat_messages()`,
`clear()`, `message_count()`, `info()`, `fork()`, `revert()`,
`compact()`, `save()`, `load()`, `list_saved()`, `delete_saved()`,
`delete_all_saved()`.

No SessionManager in core — that's a client-side concern.

### 1.6 Tool system (`tool/`)

5 tools:

| Tool          | Parameters                                                    | Description             |
| ------------- | ------------------------------------------------------------- | ----------------------- |
| `read_file`   | path (required)                                               | Read file contents      |
| `edit_file`   | path (required), old_string (required), new_string (required) | Search-and-replace edit |
| `write_file`  | path (required), content (required)                           | Write full file content |
| `run_command` | command (required), timeout (optional)                        | Execute shell command   |
| `manage_todo` | action (required), id, description, status                    | Manage todo list        |

Uses `tool-parser` crate's `JsonParser` for parsing JSON tool calls from
model output.

Key types: `Tool`, `ToolParameter`, `ToolRegistry` (register, render XML,
render JSON schemas, parse tool calls), `ParsedToolCall`.
Execution functions in `execute.rs` (async, modular, can be overridden by
client).

### 1.7 Todo system (`todo.rs`)

`TodoList`, `TodoItem`, `TodoStatus` (Pending, InProgress, Done) with
add/update/remove/render methods.

### 1.8 Chat agent (`agent.rs`)

`CoreAgent` with streaming-only chat:

```rust
pub async fn chat_stream(
    &mut self,
    user_message: &str,
    tx: tokio::sync::mpsc::Sender<ChatStreamEvent>,
) -> Result<()>
```

Tool calling loop (configurable max iterations via settings):

1. Add user message to session
2. Send messages to provider via streaming
3. Stream response deltas to channel
4. Parse tool calls from accumulated response
5. If no tool calls → emit Done event, return
6. Execute each tool call, emit ToolStart/ToolResult events
7. Add tool results to session, loop back to 2

`ChatStreamEvent` enum: `Content`, `ToolStart`, `ToolResult`, `Done`,
`Error`.

Provider/model management: `select_provider()`, `select_model()`,
`fetch_models()`, `list_providers()`, `list_models()`.

### 1.9 Prompt system (`prompts.rs`)

External `.md` templates with `{{PLACEHOLDER}}` syntax.

- `SYSTEM.md` — system prompt template (placeholders: TOOLS, DATE, CWD,
  PROJECT_CONTEXT)
- `COMPACT.md` — session summarization prompt (placeholder: CONVERSATION)
- `SUMMARY.md` — compacted context prefix (placeholder: SUMMARY)

Prompt loading priority:

1. `BIMO_PROMPTS_DIR` env var
2. `~/.config/bimo/prompts/`
3. `./agents/prompts/` relative to CWD
4. `~/.agents/prompts/`
5. `./prompts/` relative to CWD
6. `$CARGO_MANIFEST_DIR/prompts/` (dev time)
7. Built-in embedded defaults via `include_str!`

Runtime `.agents/SYSTEM.md` and `~/.agents/SYSTEM.md` overrides for system
prompt.

### 1.10 Project context (`context.rs`)

`build_project_context()` — git branch detection, top-level directory
listing, agent instruction file loading (AGENTS.md, CLAUDE.md, GEMINI.md,
.agents/AGENTS.md).

### 1.11 Current crate structure

```
src/core/
├── Cargo.toml
├── prompts/
│   ├── COMPACT.md
│   ├── SUMMARY.md
│   └── SYSTEM.md
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── config/
│   │   ├── mod.rs
│   │   ├── providers.rs
│   │   └── settings.rs
│   ├── provider/
│   │   ├── mod.rs
│   │   ├── types.rs
│   │   ├── registry.rs
│   │   └── http.rs
│   ├── model.rs
│   ├── session/
│   │   ├── mod.rs
│   │   └── persistence.rs
│   ├── tool/
│   │   ├── mod.rs
│   │   └── execute.rs
│   ├── todo.rs
│   ├── prompts.rs
│   ├── context.rs
│   └── agent.rs
```

### 1.12 Dependencies

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
tool-parser = "1.6.0"
async-stream = "0.3"
```

## Phase 2: API crate (`src/api/`) — NOT STARTED

HTTP server (axum) wrapping core. Same API surface as legacy main.rs but
cleaner, with the core doing all the work.

## Phase 3: RPC crate (`src/rpc/`) — NOT STARTED

JSON-RPC or gRPC transport wrapping core.

## Phase 4: TUI / GUI — NOT STARTED

Client applications using core as a library. Slash commands live here.
