# Bimo TUI Implementation Plan

## Overview

Build a fully-featured Terminal User Interface (TUI) for the Bimo coding agent using **ratatui + crossterm**, with all configuration managed in the TUI, modern aesthetics (Catppuccin themes), and balanced mouse/keyboard interaction.

---

## Milestones (Priority Order)

### Milestone 1: Foundation & Core Infrastructure ✅

**Priority: Critical** | **Estimated: 1-2 weeks**

#### Scope

- Set up ratatui + crossterm in `bimo-tui` crate
- Application structure: event loop, state management, theme system
- Base widget library (Button, Input, List, Select, Modal, Toast, Scrollbar)
- Main layout: chat area + collapsible sidebar + status bar
- Theme system with Catppuccin Mocha as default

#### Definition of Done

- [x] `cargo build --package bimo-tui` compiles without errors
- [x] `bimo_tui::run()` launches TUI and exits cleanly on `Esc`/`Ctrl+C`
- [x] Main layout renders: chat panel (60%), sidebar (30%), status bar (10%)
- [x] Sidebar collapses/expands via click on divider or `Ctrl+B`
- [x] Theme system loads Catppuccin Mocha palette; styles applied to all widgets
- [x] Mouse capture enabled: click, scroll, drag events received
- [x] Keyboard navigation: `Tab` cycles focus, `Esc` closes modals
- [x] Base widgets render correctly: Button (hover/press), Input (focus/cursor), List (selection), Modal (overlay), Toast (auto-dismiss)
- [x] Proportional scrollbar with mouse-draggable thumb works

---

### Milestone 2: Core Chat Experience

**Priority: Critical** | **Estimated: 2-3 weeks**

#### Scope

- Session list in sidebar: load, create, delete, fork sessions
- Agent run integration: start/stop, event streaming via broadcast channel
- Message rendering: user/assistant/tool/system with syntax highlighting
- Streaming text animation with configurable speed
- Tool call visualization: expandable cards with args/results
- Multi-line input area with history (`↑`/`↓`), send on `Enter`

#### Definition of Done

- [ ] Sidebar shows session list with name, model, updated time, message count
- [ ] Click session → loads conversation in chat panel
- [ ] "New Session" button creates session via `SessionManager::create()`
- [ ] "Fork Session" button calls `SessionManager::fork()`
- [ ] Delete session confirms via modal, calls `Session::delete()`
- [ ] Agent starts via `Agent::builder()...build().run()` on send
- [ ] Streaming `TextDelta`/`ReasoningDelta` appear character-by-character
- [ ] `ToolCallStart` shows pending tool with spinner; `ToolCallEnd` shows result
- [ ] Tool calls expand/collapse on click; args/results syntax highlighted
- [ ] Input area: multi-line, `Enter` sends, `Shift+Enter` newline, `↑`/`↓` history
- [ ] Session persists automatically via `Session::save()` in runner
- [ ] Virtualized message list handles 1000+ messages without lag
- [ ] Markdown rendering with syntect for code blocks (Rust, Python, JS, etc.)

---

### Milestone 3: Steering & Run Control

**Priority: High** | **Estimated: 1-2 weeks**

#### Scope

- Steerable mode: pause before tool calls via `run_steerable()`
- Continue/Inject controls in UI during pause
- Real-time retry/error toasts with attempt count
- Max steps indicator and warning
- Token usage/cost estimation display

#### Definition of Done

- [ ] "Steerable" toggle in sidebar enables `run_steerable()` instead of `run()`
- [ ] When agent pauses before tool call: UI shows "Awaiting steering" banner
- [ ] "Continue" button sends `SteerCommand::Continue`
- [ ] "Inject" button opens input dialog, sends `SteerCommand::Inject(text)`
- [ ] Injected guidance appears as user message in chat
- [ ] `Retrying` event shows toast: "Retrying (2/10): <error>"
- [ ] `Error` event shows persistent error toast with "Dismiss" action
- [ ] Max steps: progress bar in status bar, warning at 80%, stop at limit
- [ ] Token usage: estimate from `ModelEntry` cost fields, display in status bar
- [ ] Stop button aborts agent run cleanly (drops steer sender)
- [ ] All steering works with mouse and keyboard (`Ctrl+Enter` = Continue)

---

### Milestone 4: Settings & Provider Management

**Priority: High** | **Estimated: 2-3 weeks**

#### Scope

- Settings modal with tabs: General, Defaults, Retry, Providers, Skills
- All `SettingsConfig` fields editable with validation
- Provider CRUD: list, add, remove, test connection, set default
- Model discovery: local auto-discover, cloud catalogue refresh
- API key management (masked input, show/hide toggle)
- Skill browser with per-session enable/disable

#### Definition of Done

- [ ] `Ctrl+S` or sidebar gear icon opens Settings modal
- [ ] **General tab**: session_ttl_hours, max_sessions, cleanup_interval_minutes, max_steps, debug, snapshots — all save via `SettingsConfig::save()`
- [ ] **Defaults tab**: default_provider, default_model dropdowns populated from config
- [ ] **Retry tab**: retry_attempts, retry_timeout_secs with number inputs
- [ ] **Providers tab**:
  - Table: id, name, kind (local/cloud badge), api_format, base_url, default ★
  - "Add Provider" dialog: type (local/cloud), id, name, base_url, api_key, api_format
  - Local providers: "Discover Models" button calls `LocalProviderRegistry::auto_discover_models()`
  - Cloud providers: "Refresh" button calls `CloudProviderRegistry::refresh_provider()`
  - "Test Connection" validates API key + base URL
  - "Set Default" updates `ProvidersConfig.default`
  - Delete confirms via modal
- [ ] **Skills tab**: list loaded skills (project + global), toggle enable/disable per session
- [ ] All changes persist immediately; no "Apply" button needed
- [ ] Validation: inline errors for invalid URLs, duplicate IDs, missing required fields

---

### Milestone 5: Advanced Session Features

**Priority: High** | **Estimated: 2 weeks**

#### Scope

- Undo/Redo with visual history tree
- Session compaction with preview dialog
- Snapshot list with diff preview and restore
- Export session (Markdown/JSON)

#### Definition of Done

- [ ] **Undo/Redo**:
  - Toolbar buttons: `Ctrl+Z`/`Ctrl+Shift+Z` + clickable icons
  - Click "Undo" → calls `Session::undo()`, shows toast with restored message count
  - Click "Redo" → calls `Session::redo()`
  - History panel (sidebar tab): tree view of undo stacks, click to jump
  - Filesystem restore: if snapshot exists, files revert; toast confirms
- [ ] **Compaction**:
  - "Compact" button opens preview dialog showing summary
  - Confirm → calls `Agent::compact()`, replaces messages with summary
  - Archived messages accessible via "Show Archived" toggle
- [ ] **Snapshots**:
  - Snapshot tab in sidebar: list per-run snapshots (before/after)
  - Click snapshot → diff preview (added/modified/deleted files)
  - "Restore" button calls `Snapshot::restore()` with confirmation
  - Fork session duplicates snapshots via `Session::fork()`
- [ ] **Export**:
  - "Export" button → format selector (Markdown/JSON) → file picker
  - Calls `Session::export_markdown()` or `export_json()`

---

### Milestone 6: Todo Board & Polish

**Priority: Medium** | **Estimated: 1-2 weeks**

#### Scope

- Kanban-style todo board synced with agent's `manage_todo` tool
- Context menus (right-click) on messages, sessions, tools
- Keyboard shortcuts help overlay (`?`)
- Smooth animations: pane resize, modal fade, expand/collapse
- Performance: virtualized lists, debounced renders

#### Definition of Done

- [ ] **Todo Board** (sidebar tab):
  - Three columns: Pending, In Progress, Done
  - Cards show priority color (red/high, yellow/med, green/low)
  - Drag-drop reorder within/between columns (mouse)
  - Click card → edit inline (title, priority, status)
  - Sync: agent `manage_todo` calls update board in real-time
  - Board updates via `tools::todo_list_snapshot()`
- [ ] **Context Menus** (right-click):
  - Message: Copy, Copy as Markdown, Delete, Undo to here
  - Session: Rename, Fork, Delete, Export
  - Tool call: Copy args, Copy result, Retry
- [ ] **Help Overlay**: `?` shows searchable shortcut list with descriptions
- [ ] **Animations** (60fps target):
  - Sidebar collapse: 200ms ease-out
  - Modal fade: 150ms
  - Tool call expand: 150ms height transition
  - Streaming cursor: blink 530ms
- [ ] **Performance**:
  - Message list: only render visible + 10 buffer
  - Sidebar session list: virtualized
  - Debounce resize/render: 16ms (one frame)

---

### Milestone 7: Themes & Accessibility

**Priority: Medium** | **Estimated: 1 week**

#### Scope

- Catppuccin variants: Mocha, Latte, Frappe, Macchiato
- Light theme (Catppuccin Latte)
- High contrast mode
- Reduced motion setting
- Theme selector in Settings → Appearance

#### Definition of Done

- [ ] Theme enum: `Mocha`, `Latte`, `Frappe`, `Macchiato`, `Custom`
- [ ] Settings → Appearance tab: radio group for theme, live preview
- [ ] Theme persists in `SettingsConfig` (add `theme` field)
- [ ] High contrast: increases contrast ratios to 7:1 (WCAG AAA)
- [ ] Reduced motion: disables all animations, instant transitions
- [ ] Truecolor detection: falls back to 256-color if unsupported
- [ ] All themes tested on: alacritty, kitty, wezterm, gnome-terminal, tmux

---

### Milestone 8: Testing, Bug Fixes & Release Prep

**Priority: Medium** | **Estimated: 1 week**

#### Scope

- Integration testing across terminals
- Edge case handling
- Performance profiling
- Documentation updates
- CLI integration: `bimo tui` command

#### Definition of Done

- [ ] Tested on: alacritty, kitty, wezterm, gnome-terminal, foot, tmux, ssh
- [ ] No panics on: empty sessions, network failures, invalid config, git errors
- [ ] Memory stable over 1-hour session with 500+ messages
- [ ] CPU < 5% idle, < 20% during streaming
- [ ] `bimo tui` launches TUI (via `crates/bimo-cli/src/handlers/tui.rs`)
- [ ] README updated with TUI screenshots and keybindings
- [ ] All `cargo clippy --workspace` warnings resolved
- [ ] All `cargo fmt --check` passes

---

## Cross-Cutting Concerns

### Error Handling

- All core errors (`CustomError`) surfaced as toasts with "Copy Error" action
- Network timeouts: retry with exponential backoff indicator
- Graceful degradation: missing git → snapshots disabled with notice

### Configuration Persistence

- All settings save immediately via core's `save()` methods
- No separate TUI config file
- Theme persisted in `SettingsConfig` (new field)

### Testing Strategy

- Manual test matrix: 6 terminals × 3 themes × 2 motion settings

---

## Dependencies to Add

### `crates/bimo-tui/Cargo.toml`

```toml
[dependencies]
bimo_core = { path = "../bimo-core" }
ratatui = { version = "0.28", features = ["crossterm"] }
crossterm = "0.28"
tokio = { version = "1", features = ["full", "macros"] }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
syntect = "5"  # syntax highlighting
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
directories = "5"  # config dirs
```

---

## File Structure (Target)

```
crates/bimo-tui/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── app.rs
│   ├── event.rs
│   ├── theme.rs
│   ├── components/
│   │   ├── mod.rs
│   │   ├── chat.rs
│   │   ├── sidebar.rs
│   │   ├── settings.rs
│   │   ├── tools.rs
│   │   ├── todo.rs
│   │   ├── snapshots.rs
│   │   ├── skills.rs
│   │   └── common.rs
│   ├── layouts/
│   │   ├── mod.rs
│   │   ├── main.rs
│   │   ├── settings.rs
│   │   └── dialogs.rs
│   ├── state/
│   │   ├── mod.rs
│   │   ├── app_state.rs
│   │   ├── session_state.rs
│   │   ├── agent_state.rs
│   │   ├── config_state.rs
│   │   └── ui_state.rs
│   └── widgets/
│       ├── mod.rs
│       ├── message.rs
│       ├── markdown.rs
│       ├── streaming.rs
│       └── scrollbar.rs
```

---

## Success Criteria (Project Level)

1. **All bimo-core features accessible in TUI**: providers, models, sessions, tools, skills, snapshots, settings
2. **Mouse-first but keyboard-complete**: every action discoverable via click, every action has shortcut
3. **Modern polish**: smooth animations, syntax highlighting, Catppuccin themes, responsive layout
4. **Zero config file**: everything configured in TUI, persisted via core
5. **Performance**: 60fps rendering, handles large sessions, low idle CPU
6. **Terminal compatibility**: works on 6+ terminal emulators + tmux + ssh
