# Bimo API

Bimo is a general-purpose coding agent exposed as a JSON API. Any frontend — TUI, GUI, web app, or script — can interact with it over HTTP.

## Base URL

```
http://localhost:3847
```

Configurable via environment variables `BIMO_HOST` (default `0.0.0.0`) and `BIMO_PORT` (default `3847`).

## Response Envelope

Every endpoint returns the same JSON envelope:

```json
{
  "success": true,
  "data": { ... },
  "error": null
}
```

On failure:

```json
{
  "success": false,
  "data": null,
  "error": {
    "code": "PROVIDER_ERROR",
    "message": "provider 'openai' requires an API key."
  }
}
```

HTTP status codes: **200** on success, **400** on error.

### Error Codes

| Code | Meaning |
|---|---|
| `CONFIG_ERROR` | Configuration file read/write failure |
| `PROVIDER_ERROR` | Provider not found, missing API key, or connection failure |
| `MODEL_ERROR` | Model not found or not selected |
| `SESSION_ERROR` | Session error |
| `NETWORK_ERROR` | HTTP request to provider failed |
| `COMMAND_ERROR` | Unknown or malformed slash command |
| `API_ERROR` | General API-layer error |
| `SERIALIZATION_ERROR` | JSON parse failure |
| `NOT_IMPLEMENTED` | Feature not yet implemented |

---

## Endpoints

### Providers

#### `GET /api/provider/list`

List all available providers (built-in + custom).

**Response** `data`:

```json
[
  {
    "id": "openai",
    "name": "OpenAI",
    "category": "cloud",
    "requires_api_key": true,
    "default_base_url": "https://api.openai.com/v1",
    "builtin": true
  },
  {
    "id": "anthropic",
    "name": "Anthropic",
    "category": "cloud",
    "requires_api_key": true,
    "default_base_url": "https://api.anthropic.com",
    "builtin": true
  },
  {
    "id": "ollama",
    "name": "Ollama",
    "category": "local",
    "requires_api_key": false,
    "default_base_url": "http://localhost:11434",
    "builtin": true
  }
]
```

| Field | Type | Description |
|---|---|---|
| `id` | string | Unique identifier |
| `name` | string | Display name |
| `category` | string | `"local"` or `"cloud"` |
| `requires_api_key` | bool | Whether an API key is needed |
| `default_base_url` | string | Default API base URL |
| `builtin` | bool | `true` for built-in providers, `false` for user-added |

---

#### `POST /api/provider/select`

Select a provider. Fetches available models automatically. Fails if the provider requires an API key and none is configured.

**Request**:

```json
{
  "provider_id": "openai"
}
```

| Field | Type | Required |
|---|---|---|
| `provider_id` | string | yes |

**Response** `data`: The selected [`ProviderInfo`](#get-apiproviderlist) object.

---

#### `POST /api/provider/configure`

Set or update a provider's base URL and/or API key. Persists to `~/.bimo/config.json`. If this is the currently selected provider, the runtime is rebuilt immediately.

**Request**:

```json
{
  "provider_id": "openai",
  "base_url": "https://my-proxy.example.com/v1",
  "api_key": "sk-..."
}
```

| Field | Type | Required | Default |
|---|---|---|---|
| `provider_id` | string | yes | — |
| `base_url` | string | no | keeps existing |
| `api_key` | string | no | keeps existing |

**Response** `data`: `null`

---

#### `POST /api/provider/add`

Register a custom provider. Custom providers use OpenAI-compatible request formatting by default.

**Request**:

```json
{
  "id": "my-local",
  "name": "My Local Server",
  "category": "local",
  "base_url": "http://localhost:8080",
  "api_key_required": false,
  "chat_endpoint": "/v1/chat/completions",
  "models_endpoint": "/v1/models",
  "auth_header": "Authorization",
  "auth_prefix": "Bearer "
}
```

| Field | Type | Required | Default |
|---|---|---|---|
| `id` | string | yes | — |
| `name` | string | yes | — |
| `category` | string | no | `"local"` |
| `base_url` | string | yes | — |
| `api_key_required` | bool | no | `false` |
| `chat_endpoint` | string | yes | — |
| `models_endpoint` | string | no | `null` |
| `auth_header` | string | no | `null` |
| `auth_prefix` | string | no | `null` |

**Response** `data`: `null`

Fails with `PROVIDER_ERROR` if an `id` collision exists with a built-in or previously registered provider.

---

### Models

#### `GET /api/model/list`

List models for the currently selected provider. If models haven't been fetched yet, triggers a fetch automatically.

**Response** `data`:

```json
[
  {
    "id": "gpt-4o",
    "name": "gpt-4o",
    "provider_id": "openai"
  }
]
```

| Field | Type | Description |
|---|---|---|
| `id` | string | Model identifier (used in chat requests) |
| `name` | string | Display name |
| `provider_id` | string | Parent provider id |

---

#### `POST /api/model/select`

Select a model for use in chat.

**Request**:

```json
{
  "model_id": "gpt-4o"
}
```

| Field | Type | Required |
|---|---|---|
| `model_id` | string | yes |

**Response** `data`: `null`

Fails with `MODEL_ERROR` if the model id is not in the available models list (unless the list is empty, which allows free-form selection).

---

### Chat

#### `POST /api/chat`

Send a message and receive a model response. The full conversation history is included in the request to the provider.

**Request**:

```json
{
  "message": "Explain quicksort in one paragraph."
}
```

| Field | Type | Required |
|---|---|---|
| `message` | string | yes |

**Response** `data`:

```json
{
  "content": "Quicksort is a divide-and-conquer algorithm that...",
  "model": "gpt-4o",
  "usage": {
    "prompt_tokens": 42,
    "completion_tokens": 87,
    "total_tokens": 129
  },
  "session_id": "a1b2c3d4-..."
}
```

| Field | Type | Description |
|---|---|---|
| `content` | string | The assistant's response text |
| `model` | string? | Model used (if reported by provider) |
| `usage` | object? | Token usage (if reported by provider) |
| `session_id` | string | Current session id |

`usage` fields: `prompt_tokens`, `completion_tokens`, `total_tokens` (all `u32`).

---

### Session

#### `GET /api/session`

Get the current session's state.

**Response** `data`:

```json
{
  "session_id": "a1b2c3d4-...",
  "messages": [
    {
      "role": "user",
      "content": "Hello",
      "timestamp": "2026-07-27T12:00:00Z"
    },
    {
      "role": "assistant",
      "content": "Hi there!",
      "timestamp": "2026-07-27T12:00:01Z"
    }
  ],
  "message_count": 2
}
```

| Field | Type | Description |
|---|---|---|
| `session_id` | string | UUID of the session |
| `messages` | array | All messages in order |
| `message_count` | number | Total message count |

Message `role` is one of: `"system"`, `"user"`, `"assistant"`.

---

#### `POST /api/session/clear`

Clear all messages in the current session. The session id is preserved.

**Request**: No body required (POST with empty body).

**Response** `data`: `null`

---

### Commands

#### `POST /api/command`

Execute a slash command. The leading `/` is optional in the request.

**Request**:

```json
{
  "command": "/status"
}
```

| Field | Type | Required |
|---|---|---|
| `command` | string | yes |

**Response** `data`:

```json
{
  "command": "status",
  "output": "Provider:   openai\nModel:      gpt-4o\nSession:    a1b2c3d4-... (4 messages)\nConfigured: yes",
  "data": {
    "provider": "openai",
    "model": "gpt-4o",
    "session_id": "a1b2c3d4-...",
    "message_count": 4,
    "needs_configuration": false
  }
}
```

| Field | Type | Description |
|---|---|---|
| `command` | string | Command name that was executed |
| `output` | string | Human-readable output text |
| `data` | object? | Structured data (varies by command) |

#### Built-in Commands

| Command | Description |
|---|---|
| `/help` | List all available commands |
| `/status` | Show current provider, model, and session info |
| `/provider` | List, select, or configure providers |
| `/model` | List or select models |
| `/clear` | Clear the current conversation session |

`/provider` subcommands:

- `/provider` or `/provider list` — list providers
- `/provider select <id>` — select a provider
- `/provider configure <id>` — instructions to configure via API

`/model` subcommands:

- `/model` or `/model list` — list models
- `/model select <id>` — select a model

---

### Status

#### `GET /api/status`

Get the current agent status.

**Response** `data`:

```json
{
  "provider": "openai",
  "model": "gpt-4o",
  "session_id": "a1b2c3d4-...",
  "message_count": 4,
  "needs_configuration": false
}
```

| Field | Type | Description |
|---|---|---|
| `provider` | string? | Selected provider id (`null` if none) |
| `model` | string? | Selected model id (`null` if none) |
| `session_id` | string | Current session UUID |
| `message_count` | number | Messages in session |
| `needs_configuration` | bool | `true` if no provider is selected |

---

### Help

#### `GET /api/help`

List all registered slash commands.

**Response** `data`:

```json
{
  "commands": [
    { "name": "clear",  "description": "clear the current conversation session" },
    { "name": "help",   "description": "list all available commands" },
    { "name": "model",  "description": "list models, or select a model (/model select <id> or /model list)" },
    { "name": "provider", "description": "list, select, or configure providers (/provider list|select|configure)" },
    { "name": "status", "description": "show current provider, model, and session info" }
  ]
}
```

---

## Configuration

Bimo persists configuration to `~/.bimo/config.json`:

```json
{
  "selected_provider": "openai",
  "selected_model": "gpt-4o",
  "provider_configs": {
    "openai": {
      "base_url": "https://api.openai.com/v1",
      "api_key": "sk-..."
    }
  },
  "custom_providers": []
}
```

API keys can also be set via environment variables:

| Provider | Variable |
|---|---|
| OpenAI | `OPENAI_API_KEY` |
| Anthropic | `ANTHROPIC_API_KEY` |

Environment variables are checked as a fallback when no key is configured via the API.

---

## Quick Start

```bash
# 1. Start the server
cargo run

# 2. Check status (no provider selected yet)
curl http://localhost:3847/api/status

# 3. Configure a provider
curl -X POST http://localhost:3847/api/provider/configure \
  -H 'Content-Type: application/json' \
  -d '{"provider_id":"openai","api_key":"sk-..."}'

# 4. Select the provider (fetches models automatically)
curl -X POST http://localhost:3847/api/provider/select \
  -H 'Content-Type: application/json' \
  -d '{"provider_id":"openai"}'

# 5. Select a model
curl -X POST http://localhost:3847/api/model/select \
  -H 'Content-Type: application/json' \
  -d '{"model_id":"gpt-4o"}'

# 6. Chat
curl -X POST http://localhost:3847/api/chat \
  -H 'Content-Type: application/json' \
  -d '{"message":"Write a hello world in Rust"}'
```

Or for Ollama (no API key needed):

```bash
# 1. Select Ollama
curl -X POST http://localhost:3847/api/provider/select \
  -H 'Content-Type: application/json' \
  -d '{"provider_id":"ollama"}'

# 2. List available models
curl http://localhost:3847/api/model/list

# 3. Select a model
curl -X POST http://localhost:3847/api/model/select \
  -H 'Content-Type: application/json' \
  -d '{"model_id":"llama3"}'

# 4. Chat
curl -X POST http://localhost:3847/api/chat \
  -H 'Content-Type: application/json' \
  -d '{"message":"Hello"}'
```
