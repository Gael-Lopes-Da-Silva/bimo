# Bimo TUI Implementation Plan

## Overview
Implement the `bimo-tui` crate as a Cursive-based terminal UI library that interfaces with `bimo-core` to provide an interactive coding agent interface.

## Architecture

### Crate Structure
```
crates/bimo-tui/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public exports
│   ├── app.rs              # Main App struct, Cursive root, event loop
│   ├── theme.rs            # Theme system (terminal-aware + config file)
│   ├── layout.rs           # Main layout: output + input + command palette
│   ├── input/
│   │   ├── mod.rs
│   │   ├── input_area.rs   # Expandable input box (3-15 lines)
│   │   ├── autocomplete.rs # / commands, @/./ file paths
│   │   └── keybindings.rs  # Ctrl+J, Ctrl+P, etc.
│   ├── output/
│   │   ├── mod.rs
│   │   ├── message_view.rs # Message rendering (user, agent, tool)
│   │   ├── markdown.rs     # Custom markdown renderer
│   │   └── scroll.rs       # Scrollable output area
│   ├── palette/
│   │   ├── mod.rs
│   │   ├── command.rs      # Command definition
│   │   ├── registry.rs     # Command registry with shortcuts
│   │   └── view.rs         # Command palette UI
│   ├── events/
│   │   ├── mod.rs
│   │   ├── handler.rs      # AgentEvent -> UI updates
│   │   └── bridge.rs       # tokio broadcast -> Cursive cb_sink
│   └── config/
│       ├── mod.rs
│       └── theme_config.rs # Theme loading from ~/.config/bimo/themes/
```

## Dependencies (Cargo.toml)
```toml
[dependencies]
bimo_core = { path = "../bimo-core" }
cursive = { version = "0.21.1", features = ["crossterm-backend"] }
tokio = { version = "1", features = ["full", "sync"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
dirs = "6"
thiserror = "2"
pulldown-cmark = "0.12"  # for markdown parsing
walkdir = "2"             # for file completion
```

---

## Phase 1: Foundation (Theme + Layout)

### 1.1 Theme System (`theme.rs`)
- **Terminal-aware default**: Detect terminal background (light/dark) via `COLORTERM`, `TERM`, or ANSI query. Fallback to dark.
- **Color palette** (no borders, background colors only):
  - `background` - main bg
  - `surface` - box backgrounds (input, messages, palette)
  - `surface_alt` - alternating message backgrounds
  - `primary` - accent (titles, highlights)
  - `success` - green for tool success
  - `error` - red for tool failure
  - `muted` - gray for user prompts, timestamps
  - `text` - primary text
  - `text_secondary` - dim text
- **Config file**: `~/.config/bimo/themes/<name>.json` with full color override
- **Cursive theme application**: Implement `cursive::theme::Theme` builder

### 1.2 Main Layout (`layout.rs`)
```
┌─────────────────────────────────────┐
│          Output Area (scrollable)   │  ← flex: grows
│  ┌───────────────────────────────┐  │
│  │ User prompt (gray box)        │  │
│  │ Agent message (markdown)      │  │
│  │ Tool call (green/red box)     │  │
│  └───────────────────────────────┘  │
├─────────────────────────────────────┤
│          Input Area (3-15 lines)    │  ← fixed bottom
│  ┌───────────────────────────────┐  │
│  │ [input field]                 │  │
│  └───────────────────────────────┘  │
└─────────────────────────────────────┘
```
- Use `LinearLayout(vertical)` with `ResizedView` for input area
- Output area: `ScrollView` wrapping `LinearLayout(vertical)` for messages
- Command palette: `Layer` overlay (hidden by default)

---

## Phase 2: Input Area (`input/`)

### 2.1 Input Area (`input_area.rs`)
- **Base height**: 3 lines (1 border-top, 1 content, 1 border-bottom — but NO borders, use background)
- **Expand on Ctrl+J**: Insert newline, grow by 1 line up to max 15 lines
- **Cursive view**: `EditView` or `TextArea` (custom for multi-line)
- **Keybindings**:
  - `Ctrl+J`: Add newline (expand if < 15 lines)
  - `Enter` (no Ctrl): Submit prompt
  - `Ctrl+P`: Open command palette
  - `Esc`: Clear input / close palette

### 2.2 Autocomplete (`autocomplete.rs`)
- **Trigger `/` at start**: Show command completions (`/exit`, `/help`, etc.)
- **Trigger `@` or `./` anywhere**: File path completion
  - `@` → relative to project root
  - `./` → relative to cwd
- **Implementation**: Custom `EditView` subclass or `OnEdit` callback + popup `SelectView`
- **Replacement**: Replace trigger symbol with selected path

### 2.3 Keybindings (`keybindings.rs`)
Centralized keybinding map for the entire app.

---

## Phase 3: Output Area (`output/`)

### 3.1 Message View (`message_view.rs`)
Each message type rendered as a box with 1-char padding, 1-char spacing between messages:

| Role | Style |
|------|-------|
| `user` | Gray background box, padding 1, content left-aligned |
| `assistant` | No background, markdown-rendered text |
| `tool` (success) | Green background box, padding 1, title = tool name |
| `tool` (failure) | Red background box, padding 1, title = tool name |
| `tool` (run_command) | Show command between title and output |

**Structure for tool calls**:
```
┌─────────────────────────────────────┐
│ run_command                    ✓/✗ │  ← title bar (tool name + status)
├─────────────────────────────────────┤
│ $ ls -la                            │  ← command (only for run_command)
├─────────────────────────────────────┤
│ output content here...              │
└─────────────────────────────────────┘
```

### 3.2 Markdown Renderer (`markdown.rs`)
**Custom implementation** using Cursive's `TextView` with `RichText`/`StyledText`:
- Parse markdown (use `pulldown-cmark`)
- Map to Cursive styles:
  - `# Heading` → bold + primary color + size
  - `**bold**` → `Style::bold()`
  - `*italic*` → `Style::italic()`
  - `` `code` `` → monospace + background
  - ```code block``` → monospace box with surface background
  - `- list` → bullet + indent
  - `> quote` → left border + muted color
  - `[link](url)` → underlined + primary color

### 3.3 Scrollable Output (`scroll.rs`)
- `ScrollView` around message container
- Auto-scroll to bottom on new message (configurable)
- Preserve scroll position when user scrolls up

---

## Phase 4: Command Palette (`palette/`)

### 4.1 Command Registry (`registry.rs`)
```rust
struct Command {
    id: String,           // "exit", "help", "new_session"
    name: String,         // "Exit Application"
    description: String,  // "Close the TUI"
    shortcut: Option<Key>, // Some(Key::Ctrl('Q'))
    action: Box<dyn Fn(&mut Cursive) + Send + Sync>,
}
```

### 4.2 Palette View (`view.rs`)
- **Trigger**: `Ctrl+P` → add layer on top
- **Layout**:
  ```
  ┌─────────────────────────────────────┐
  │ Search: [____________________]      │  ← EditView
  ├─────────────────────────────────────┤
  │ ▸ Exit Application          Ctrl+Q  │  ← SelectView (scrollable)
  │   New Session               Ctrl+N  │
  │   Help                      F1      │
  └─────────────────────────────────────┘
  ```
- **Filtering**: Fuzzy match on name/description
- **Navigation**: Up/Down, Enter to execute, Esc to close
- **Shortcut display**: Show in right column if exists

---

## Phase 5: Event Bridge (`events/`)

### 5.1 Bridge (`bridge.rs`)
```rust
struct EventBridge {
    cb_sink: CursiveCbSink,
    rx: broadcast::Receiver<AgentEvent>,
}

impl EventBridge {
    fn spawn(self) {
        tokio::spawn(async move {
            while let Ok(event) = self.rx.recv().await {
                self.cb_sink.send(Box::new(move |siv| {
                    handle_agent_event(siv, event);
                })).ok();
            }
        });
    }
}
```

### 5.2 Handler (`handler.rs`)
Map `AgentEvent` to UI updates:
- `TextDelta` → append to current assistant message (streaming)
- `ReasoningDelta` → show in dimmed style (optional)
- `ToolCallStart` → create tool message box (pending state)
- `ToolCallEnd` → update tool box with result (green/red)
- `Steering` → show as user message (gray box)
- `Error` → show error toast/status
- `Done` → finalize, enable input

---

## Phase 6: Main App (`app.rs`)

### 6.1 App Struct
```rust
pub struct App {
    siv: CursiveRunner,
    session: Session,
    agent_rx: broadcast::Receiver<AgentEvent>,
    steer_tx: Option<mpsc::Sender<SteerCommand>>,
    input_area: InputArea,
    output_area: OutputArea,
    palette: CommandPalette,
}
```

### 6.2 Initialization
1. Load theme (terminal-aware → config override)
2. Build Cursive root with theme
3. Create layout (output + input)
4. Register global keybindings
5. Spawn event bridge
6. Run agent (`Agent::run()` or `run_steerable()`)
7. `siv.run()`

### 6.3 Public API
```rust
pub async fn run_tui(session: Session) -> Result<()> {
    let mut app = App::new(session).await?;
    app.run().await
}
```

---

## Phase 7: Integration & Polish

### 7.1 bimo Binary (`crates/bimo/src/main.rs`)
```rust
use bimo_tui::run_tui;
use bimo_core::Session;

#[tokio::main]
async fn main() -> Result<()> {
    let session = Session::new(); // or load existing
    run_tui(session).await
}
```

### 7.2 Testing Checklist
- [ ] Theme loads correctly (dark/light terminal detection)
- [ ] Input expands on Ctrl+J up to 15 lines
- [ ] Autocomplete works for `/` commands and `@`/`./` paths
- [ ] Command palette opens with Ctrl+P, shows shortcuts
- [ ] User messages render in gray boxes
- [ ] Agent messages render as markdown
- [ ] Tool calls render in green/red boxes with titles
- [ ] run_command shows command in tool box
- [ ] Scroll works, auto-scroll on new messages
- [ ] Agent events stream correctly via bridge
- [ ] Config theme override works

---

## File Creation Order

1. `Cargo.toml` - Add dependencies
2. `src/theme.rs` - Theme system
3. `src/config/theme_config.rs` - Config loading
4. `src/layout.rs` - Main layout structure
5. `src/input/input_area.rs` - Input box
6. `src/input/autocomplete.rs` - Autocomplete logic
7. `src/input/keybindings.rs` - Keybinding definitions
8. `src/output/markdown.rs` - Markdown renderer
9. `src/output/message_view.rs` - Message box rendering
10. `src/output/scroll.rs` - Scrollable output
11. `src/palette/command.rs` - Command struct
12. `src/palette/registry.rs` - Command registry
13. `src/palette/view.rs` - Palette UI
14. `src/events/bridge.rs` - Async bridge
15. `src/events/handler.rs` - Event handling
16. `src/app.rs` - Main app
17. `src/lib.rs` - Public exports
18. Update `crates/bimo/src/main.rs` - Entry point

---

## Open Questions for Implementation

1. **Markdown parsing**: Use `pulldown-cmark` (adds dependency) or simple regex-based parser?
2. **File completion**: Use `walkdir` for async file listing, or sync in callback?
3. **Steerable vs non-steerable**: Default to `run_steerable()` for future steering UI?
4. **Session persistence**: Auto-save session on each message? Or manual?

---

## Estimated Effort

| Phase | Files | Complexity |
|-------|-------|------------|
| 1: Foundation | 4 | Medium |
| 2: Input | 4 | High (custom EditView) |
| 3: Output | 4 | High (markdown + layout) |
| 4: Palette | 4 | Medium |
| 5: Events | 2 | Medium |
| 6: App | 2 | Medium |
| 7: Integration | 2 | Low |
| **Total** | **~22** | **~3-5 days** |
