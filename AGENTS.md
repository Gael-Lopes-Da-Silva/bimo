# AGENTS.md - Bimo TUI Development Context

## Project Overview
This is a Rust coding agent harness with three crates:
- **bimo-core**: Core library (agent, session, tools, providers)
- **bimo-cli**: CLI handlers using clap
- **bimo-tui**: Cursive-based TUI (THIS CRATE - to be implemented)
- **bimo**: Main binary combining everything

## Current Task
Implement the `bimo-tui` crate following the plan in `PLAN.md`.

Status: workspace builds clean (`cargo build --workspace`), clippy clean, 17 unit tests pass. The TUI boots and is wired to the agent (see "Notes for Next Session").

## Key Conventions

### Code Style
- Edition 2024
- Use `thiserror` for error types
- Use `tracing` for logging
- Public API in `lib.rs`, internal modules private
- Async with `tokio`

### Cursive Patterns (verified against cursive 0.21.1 / cursive_core 0.4.7)
- Use `Cursive` + `cursive::CursiveExt` (trait needed for `.run()`)
- Theme via `cursive::theme::Theme`
- Views: `LinearLayout`, `ScrollView`, `Panel`, `EditView`, `TextView`, `SelectView`, `Dialog`, `DummyView`
- **No `Key::Ctrl(...)`** — use `Event::CtrlChar('c')` / `Event::Key(Key::Esc)`
- **No custom `ViewWrapper`/`wrap_impl!`** — build layers with `Dialog`/standard views instead
- `LinearLayout`/`ScrollView`/`EditView`/`SelectView`/`Panel` do **not** implement `Clone` — keep owned state, clone via captured `Arc` (see `Command.action`)
- Chain `on_submit`/`on_edit` **before** `.with_name(...)` (methods are on the raw view, not `NamedView`)
- `SelectView::<String>::new()` needs the explicit type param (else `on_submit` borrow type is ambiguous)
- For `fixed_width`/`fixed_height`, import `cursive::view::Resizable`
- For `.with_name(...)`, import `cursive::view::Nameable`
- `TextView::append`/`set_content` take `Into<StyledString>` (String works); `get_content()` returns `TextContentRef` (derefs to `StyledString`; use `.source()` for `&str`, not `.to_string()`)
- `Panel` has `title()`/`set_title()` but **no** `get_title()`
- Streaming updates: give a view a name (e.g. `panel.with_name("current_assistant")`) then `siv.call_on_name("name", |v: &mut Panel<TextView>| ...)` to mutate it; add a fresh named view when the callback returns `None`
- Callbacks: `cb_sink.send(Box::new(|siv| { ... }))`
- Keybindings: `siv.add_global_callback(Event::CtrlChar('j'), ...)`; guard Esc/pop with `if siv.screen().len() > 1 { siv.pop_layer(); }`
- `Cursive::user_data::<T>(&mut self) -> Option<&mut T>` — takes `&mut self`; cloning data out before other calls avoids double-borrow
- `Cursive::set_user_data<T: Any>(&mut self, data: T)`

### bimo-core Types to Know
```rust
// Agent events (from broadcast::Receiver)
enum AgentEvent {
    TextDelta(String),
    ReasoningDelta(String),
    ToolCallStart { tool_name: String, args: serde_json::Value },
    ToolCallEnd { tool_name: String, result: Result<String, String> },
    Steering(String),
    Retrying { attempt: usize, error: String },
    Error(String),
    Done,
}

// Session messages
struct Message {
    id: String,
    role: String,  // "user" | "assistant" | "system" | "tool"
    content: String,
    timestamp: DateTime<Utc>,
}

// Agent entry point
Agent::builder()
    .session(session)
    .provider("anthropic")
    .model("claude-3-5-sonnet")
    .build()
    .await?
    .run()  // returns broadcast::Receiver<AgentEvent>
```

## Dependencies (Already in Cargo.toml)
- `cursive = { version = "0.21.1", features = ["crossterm-backend"] }`
- `tokio = { version = "1", features = ["full"] }`
- `bimo_core = { path = "../bimo-core" }`
- `serde`, `serde_json`, `tracing`, `dirs`, `thiserror`
- **Add**: `pulldown-cmark = "0.12"`, `walkdir = "2"`

## Implementation Priority (from PLAN.md)

1. **Theme & Config** → `theme.rs`, `config/theme_config.rs`
2. **Layout** → `layout.rs`
3. **Input Area** → `input/input_area.rs`, `input/autocomplete.rs`, `input/keybindings.rs`
4. **Output** → `output/markdown.rs`, `output/message_view.rs`, `output/scroll.rs`
5. **Command Palette** → `palette/command.rs`, `palette/registry.rs`, `palette/view.rs`
6. **Events** → `events/bridge.rs`, `events/handler.rs`
7. **App** → `app.rs`, `lib.rs`
8. **Integration** → Update `crates/bimo/src/main.rs`

## Key Design Decisions (User Confirmed)

| Decision | Choice |
|----------|--------|
| Markdown rendering | Custom implementation with `pulldown-cmark` + Cursive `TextView` |
| Theme | Terminal-aware default (detect dark/light), optional config file at `~/.config/bimo/themes/*.json` |
| Async handling | `tokio` + Cursive `cb_sink` callbacks (no `cursive-async-view`) |
| Input expansion | Ctrl+J adds newline, grows 3→15 lines max |
| Autocomplete | `/` at start = commands, `@` or `./` anywhere = file paths |
| Command palette | Ctrl+P, search input + scrollable list, show shortcuts |

## Testing Commands
```bash
# Build
cargo build --package bimo-tui

# Run (after implementation)
cargo run --package bimo

# Check
cargo check --package bimo-tui
cargo clippy --package bimo-tui
```

## File Structure (current state)
```
crates/bimo-tui/src/
├── lib.rs          # pub modules: app, config, error, events, input, palette, theme
├── app.rs          # App (Cursive root), prompt channel -> agent -> EventBridge
├── theme.rs        # BimoTheme, ThemeColors, ThemeVariant, ThemeError, parse_color
├── config/
│   ├── mod.rs
│   └── theme_config.rs  # load/save/list themes as JSON
├── input/
│   ├── mod.rs      # autocomplete, keybindings (input_area.rs not yet wired)
│   ├── autocomplete.rs
│   └── keybindings.rs
├── palette/
│   ├── mod.rs
│   ├── command.rs
│   ├── registry.rs
│   └── view.rs     # create_command_palette_layer() -> Dialog
├── events.rs       # EventBridge + handle_agent_event (single file)
├── error.rs        # Error/Result
└── (orphaned, NOT in module tree: layout.rs, output/*)
```

## Notes for Next Session
- Workspace builds clean, clippy clean, 17 tests pass (9 in bimo-tui, 8 in bimo-core).
- `run_tui(session)` is now sync; `bimo/src/main.rs` calls it without `.await`.
- Agent flow: user Enter -> `UnboundedSender<String>` in `Cursive::user_data` -> `prompt_loop` -> `run_agent_once` (builds `Agent` with `with_user_prompt`, `run_steerable`, drops steer_tx) -> `EventBridge` streams into named views (`current_assistant`, `current_reasoning`, `current_tool`) in the `messages` layout.
- Provider/model come from env `ANTHROPIC_API_KEY` and `BIMO_MODEL`; hardcoded Anthropic cloud provider in `app::run_agent_once`.
- Todo items:
  - Wire up `layout.rs` and `output/*` (markdown rendering via `pulldown-cmark`, message view, scroll) — currently orphaned files.
  - Replace single-line `EditView` input with growable multi-line input (3->15 lines, Ctrl+J); wire `input/autocomplete.rs`.
  - Auto-scroll `output_area` to bottom on new content; colored message types (green/red tool boxes) per PLAN.
  - Use `run_steerable` steering (hold `steer_tx`, gate tool calls, `SteerCommand::Continue`/`Inject`).
- `cargo test --workspace` / `cargo clippy --workspace --all-targets` to verify.
