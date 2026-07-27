# Bimo — Project Context for Coding Agents

## Overview

Bimo is a general-purpose coding agent written in Rust. It follows a **client-server architecture** with two independent Rust crates (no Cargo workspace):

- **`api/`** — `bimo_api`: Core agent logic + Axum HTTP server
- **`tui/`** — `bimo_tui`: Terminal UI client (ratatui + crossterm)

The TUI communicates with the API exclusively over HTTP (JSON). The `BimoApi` struct is transport-agnostic, designed so any frontend (GUI, web, scripts) can use the same API server.

## Quick Start

```bash
# From api/ directory:
cargo run              # Starts server on 0.0.0.0:3847

# From tui/ directory:
cargo run              # Connects to http://localhost:3847
```

## Build & Lint

```bash
# Each crate must be built independently (no workspace)
cd api && cargo build && cargo clippy && cargo fmt --check
cd tui && cargo build && cargo clippy && cargo fmt --check
```

There are no tests in the project currently.

## Environment Variables

| Variable | Crate | Default | Purpose |
|---|---|---|---|
| `BIMO_HOST` | api | `0.0.0.0` | Server bind address |
| `BIMO_PORT` | api | `3847` | Server port |
| `BIMO_URL` | tui | `http://localhost:3847` | API server URL |
| `OPENAI_API_KEY` | api | — | OpenAI API key (fallback) |
| `ANTHROPIC_API_KEY` | api | — | Anthropic API key (fallback) |
| `RUST_LOG` | api | `info` | Log level (tracing env filter) |

## Architecture

```
┌─────────────────┐      HTTP (JSON)       ┌─────────────────────┐
│   bimo_tui      │◄──────────────────────►│     bimo_api         │
│   (ratatui UI)  │   localhost:3847        │   (Axum server)      │
│                 │                         │                      │
│  - Chat display │   12 REST endpoints     │  - Agent (state)     │
│  - Autocomplete │                         │  - Provider registry │
│  - Status bar   │                         │  - Session mgmt      │
└─────────────────┘                         │  - Command system    │
                                            │  - Tool registry     │
                                            └──────────┬───────────┘
                                                       │ reqwest HTTP
                                                       ▼
                                            ┌─────────────────────┐
                                            │   LLM Providers      │
                                            │  OpenAI / Anthropic  │
                                            │  Ollama / Custom     │
                                            └─────────────────────┘
```

## API Crate (`api/`)

### Source Files

| File | Lines | Purpose |
|---|---|---|
| `main.rs` | 151 | Axum HTTP server, route definitions, request handlers |
| `lib.rs` | 13 | Module declarations and re-exports (`BimoApi`, `BimoError`, `Result`) |
| `agent.rs` | 390 | Central `Agent` struct — holds all state, implements all operations |
| `api.rs` | 298 | `BimoApi` public interface, request/response types, `ApiResponse` envelope |
| `provider.rs` | 528 | `ProviderRegistry`, `ProviderRuntime`, built-in providers (OpenAI/Anthropic/Ollama), HTTP chat completion + model fetching |
| `session.rs` | 235 | `Session` struct, `Message`/`Role` types, persistence to `~/.bimo/sessions/` |
| `command.rs` | 817 | Slash command system: `SlashCommand`/`AsyncSlashCommand` traits, `CommandRegistry`, 8 built-in commands |
| `config.rs` | 103 | `AppConfig` persistence to `~/.bimo/config.json` |
| `tools.rs` | 141 | `ToolRegistry` with 6 built-in tool definitions (metadata only, not yet invoked) |
| `model.rs` | 29 | `ModelInfo` struct, `fetch_models_for_provider` |
| `error.rs` | 65 | `BimoError` enum (9 variants), `ApiErrorPayload` |

### Key Types

- **`Agent`** (`agent.rs:19`) — Holds `AppConfig`, `Session`, `ProviderRegistry`, `ToolRegistry`, `CommandRegistry`, `Vec<ModelInfo>`, and `Option<ProviderRuntime>`.
- **`BimoApi`** (`api.rs:138`) — Wraps `Agent`, provides transport-agnostic typed methods that return `ApiResponse`. The HTTP server in `main.rs` calls these methods.
- **`ProviderRuntime`** (`provider.rs:38`) — Resolved configuration for talking to a specific provider (base URL, auth, endpoints, request format).
- **`Session`** (`session.rs:27`) — Conversation history with UUID, timestamps, persistence.
- **`CommandContext`** (`command.rs:91`) — Mutable state snapshot passed to slash commands.

### HTTP Endpoints

All responses use a JSON envelope: `{ "success": bool, "data": ..., "error": ... }`.

| Method | Route | Handler |
|---|---|---|
| GET | `/api/provider/list` | List all providers |
| POST | `/api/provider/select` | Select provider + fetch models |
| POST | `/api/provider/configure` | Set base URL / API key |
| POST | `/api/provider/add` | Register custom provider |
| GET | `/api/model/list` | List models for current provider |
| POST | `/api/model/select` | Select a model |
| POST | `/api/chat` | Send message, get LLM response |
| GET | `/api/session` | Get current session state |
| POST | `/api/session/clear` | Clear session messages |
| POST | `/api/command` | Execute slash command |
| GET | `/api/commands` | List commands (with subcommand metadata) |
| GET | `/api/status` | Get agent status |
| GET | `/api/help` | List commands with descriptions |

### Built-in Providers

| ID | Category | API Key | Format | Base URL |
|---|---|---|---|---|
| `openai` | cloud | required | OpenAI | `https://api.openai.com/v1` |
| `anthropic` | cloud | required | Anthropic | `https://api.anthropic.com` |
| `ollama` | local | none | Ollama | `http://localhost:11434` |

Custom providers default to OpenAI-compatible request format. Auth can be configured with custom `auth_header` and `auth_prefix`.

### Built-in Commands

| Command | Async | Description |
|---|---|---|
| `/help` | no | List all commands |
| `/status` | no | Show provider, model, session info |
| `/clear` | no | Clear conversation |
| `/model [list\|select <id>]` | no | Model management |
| `/provider [list\|select\|configure]` | no | Provider management |
| `/tools` | no | List registered tools |
| `/session [list\|save\|resume\|delete\|info\|purge]` | no | Session management |
| `/compact` | yes | Summarize session via LLM |

### Built-in Tools (metadata only, not yet invoked in chat)

`read_file`, `write_file`, `list_files`, `run_command`, `search_files`, `search_content`

### Config Persistence

- Config: `~/.bimo/config.json` — selected provider/model, per-provider base URLs and API keys, custom providers
- Sessions: `~/.bimo/sessions/<uuid>.json` — full conversation history

## TUI Crate (`tui/`)

### Source Files

| File | Lines | Purpose |
|---|---|---|
| `main.rs` | 816 | Entire TUI application in a single file |

### Features

- **Chat interface**: Displays user/assistant/system/error messages with colored prefixes (`you`, `bimo`, `sys`, `err`)
- **Slash command autocomplete**: Tab-completes commands from `GET /api/commands`, with popup list widget
- **Status bar**: Shows current provider, model, and keybinding hints
- **Keyboard controls**: Esc (quit), Enter (send), Tab (autocomplete), Up/Down/PageUp/PageDown/Home/End (scroll)
- **Background tasks**: Status refresh every 5 seconds, command list loaded at startup via `tokio::mpsc` channels

### Client-side Commands

Handled locally without forwarding to API:
- `/clear` — clears local display
- `/exit`, `/quit` — exits TUI

All other `/`-commands are forwarded to `POST /api/command`.

## Design Principles

1. **Transport agnosticism**: `BimoApi` is the public interface; HTTP is just one transport layer. The `Agent` struct is fully decoupled from HTTP.
2. **Envelope pattern**: Every API response uses `{ success, data, error }` — errors include machine-readable codes (`PROVIDER_ERROR`, etc.).
3. **Persistence**: Config and sessions are saved to `~/.bimo/` as JSON. Sessions support save/resume/delete with prefix ID matching.
4. **Provider abstraction**: `ProviderRuntime` + `RequestBodyFormat` enum handles OpenAI, Anthropic, Ollama, and custom providers through a unified interface.
5. **Command extensibility**: New commands are added by implementing `SlashCommand` or `AsyncSlashCommand` trait and registering in `CommandRegistry::new()`.
6. **Tool registry**: Tools are defined as metadata (name, description, parameters). Tool execution is not yet wired into the chat flow — this is an active area of development.

## Current Limitations / TODOs

- **Tools are not invoked during chat**: `ToolRegistry` defines tools but the agent doesn't call them. The LLM provider APIs support function calling / tool use, but it's not wired up yet.
- **No streaming**: Chat completions are request-response only (`"stream": false` for Ollama, no SSE handling).
- **No system prompt**: There is no system prompt configuration — the agent sends the raw conversation history to the provider.
- **No tests**: Neither crate has test coverage.
- **No Cargo workspace**: The two crates are independent; builds must be run from each crate's directory separately.
