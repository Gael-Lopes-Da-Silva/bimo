# Bimo GUI Plan — GTK4 + Libadwaita

Status: TUI deprecated (`bimo-tui` kept in workspace, no active development). CLI (`bimo-cli`) kept. `bimo-gui` is the primary user-facing interface.

## Architecture

- **Framework**: GTK4 (`gtk4-rs`) + Libadwaita (`libadwaita-rs`) — target GTK 4.16+, libadwaita 1.6+
- **Crate**: `crates/bimo-gui` (new binary `bimo-gui`, launched via `bimo gui` or standalone)
- **Cross-platform**: Linux (system libs), Windows (MSYS2/gvsbuild + bundled DLLs), macOS (Homebrew + `.app` bundle). Equal priority.
- **Core integration**: `bimo-core` consumed directly — no rewrites.
  - `AgentBuilder` → `Agent::run()` / `run_steerable()` → `broadcast::ReceiverAgentEvent>`
  - `SessionManager`, `Session`, `SettingsConfig`, `ProvidersConfig`, `Snapshot`
- **Async bridge** (`src/bridge.rs`): tokio runtime thread receives `AgentEvent`. Events forwarded to GTK main thread via `glib::MainContext::channel` or `idle_add_local`. Prevents blocking the GTK loop.

## File Structure

```
crates/bimo-gui/
  Cargo.toml
  build.rs (optional: GTK/libadwaita version checks)
  PLAN.md (this file)
  src/
    lib.rs
    main.rs              // Entry: Adw.Application, main window
    bridge.rs            // Tokio -> GTK event bridge
    app.rs               // Adw.Application subclass
    state.rs             // Shared app state (session manager, agent channels, active session)
    windows/
      main.rs            // Main chat: Adw.NavigationSplitView (sidebar + chat)
      settings.rs        // Adw.PreferencesWindow (General, Providers, Defaults, Skills)
    widgets/
      chat.rs            // Message list + streaming
      sidebar.rs         // Session list, new/fork/delete buttons
      input.rs           // Multi-line user input (Gtk.TextView)
      session_item.rs    // Sidebar row widget
      tool_card.rs       // Expandable ToolCallStart/End display
      markdown_view.rs   // Syntax-highlighted assistant messages
    components/
      message_row.rs
```

## Milestones / Stages

### Stage 1: Foundation & Cross-Platform Build (2–3 weeks)
Objective: `cargo build --package bimo-gui` passes on Linux; skeleton runs; async bridge delivers events without blocking.

- [ ] `Cargo.toml`: `gtk4`, `glib`, `libadwaita`, `tokio`, `bimo-core`
- [ ] `build.rs` (optional): verify `pkg-config` for `gtk4`, `libadwaita`
- [ ] `src/lib.rs`: module exports
- [ ] `src/state.rs`: `AppState` holding `Arc<RwLockSessionManager>>`, active session, agent event channels
- [ ] `src/bridge.rs`: tokio thread subscribes to `AgentEvent`; pushes to GTK via bounded queue + `idle_add_local`
- [ ] `src/app.rs`: `Adw.Application` subclass with `bimo-gui` ID
- [ ] `src/main.rs`: initialize `Adw.Application`, create main window, load default settings
- [ ] `windows/main.rs`: empty `Adw.ApplicationWindow` with `Adw.NavigationSplitView` (sidebar + content)
- [ ] Linux CI build verified; Windows/macOS build scripts drafted (MSYS2/gvsbuild, Homebrew)
- [ ] Confirm `AgentEvent` flows: `AgentEvent::TextDelta` → GTK updates text buffer

### Stage 2: Session & Chat Integration (3–4 weeks)
Objective: Full chat loop: load/create/fork/delete sessions; send prompts; stream responses; render messages.

- [ ] Sidebar: load sessions from `SessionManager::list()`; display name/model/updated/time/messages
- [ ] Sidebar: click session → load conversation into chat; double-click/edit name
- [ ] Sidebar: "New Session" (`SessionManager::create()`), "Fork" (`Session::fork()`), "Delete" (confirm + `Session::delete()`)
- [ ] Chat panel: `Message` rendering — user (bubble left), assistant (bubble right), system, tool
- [ ] Input area: multi-line `Gtk.TextView`; Send button / `Ctrl+Enter`; `Shift+Enter` for newline
- [ ] Input history: `↑`/`↓` navigates previous prompts (stored in `AppState`)
- [ ] Agent run: `AgentBuilder` configured with session/provider/model/user prompt; `Agent::run()` called
- [ ] Streaming: `AgentEvent::TextDelta` appended to assistant message buffer incrementally; `AgentEvent::ReasoningDelta` shown in collapsible reasoning block
- [ ] Agent events: `AgentEvent::Done` finalizes message; `AgentEvent::Error` → toast/error label; `AgentEvent::Retrying` → retry indicator
- [ ] Stop button: drops `AgentEvent` receiver or aborts tokio task cleanly
- [ ] Auto-save: `Session::save()` called after agent events modify messages

### Stage 3: Tools, Settings & Advanced Features (2–3 weeks)
Objective: Tool visualization, settings management, undo/redo, snapshots, export.

- [ ] `widgets/tool_card.rs`: `AgentEvent::ToolCallStart` → spinner card with args (`serde_json::Value`); `AgentEvent::ToolCallEnd` → expand/collapse result; retry button per tool
- [ ] `windows/settings.rs`: `Adw.PreferencesWindow` with tabs/page mapping to `SettingsConfig` / `ProvidersConfig`
  - General: session TTL, max sessions, max steps, debug, snapshots toggle
  - Defaults: default provider/model dropdowns
  - Retry: attempts, timeout
  - Providers: CRUD table; add/edit dialog; test connection; set default; masked API key input
  - Skills: list loaded skills; toggle per-session (`Session::disable_skill`/`enable_skill`)
- [ ] Provider discovery: `LocalProviderRegistry::auto_discover_models()`; `CloudProviderRegistry::refresh_provider()`
- [ ] Undo/Redo: toolbar buttons / `Ctrl+Z` / `Ctrl+Shift+Z`; `Session::undo()` / `redo()`; snapshot restore toast
- [ ] Compaction: button opens preview dialog (`Agent::compact()`); archive toggle
- [ ] Snapshots: list per session; click → diff preview; restore (`Snapshot::restore()`)
- [ ] Export: Markdown / JSON (`Session::export_markdown()` / `export_json()`)
- [ ] Todo board widget: `SharedTodoList` synced with `AgentEvent` / agent `manage_todo` calls; Kanban columns (Pending / In Progress / Done); drag-drop reorder; inline edit (title, priority, status)

### Stage 4: Cross-Platform Packaging & Release Prep (2–3 weeks)
Objective: Equal-priority builds for Linux, Windows, macOS; installers/bundles ready.

- [ ] Linux: `.deb` packaging script; optional Flatpak manifest; `AppImage` build
- [ ] Windows: MSYS2 build environment; `gvsbuild` for GTK4/libadwaita; DLL bundling; `.msi` or portable `.zip`; GitHub Actions Windows runner verification
- [ ] macOS: `.app` bundle with embedded `gtk4`/`libadwaita` frameworks; `.dmg` via `create-dmg`; Homebrew formula draft; CI via macOS runner
- [ ] Binary size: verify compressed releases stay reasonable (<50MB target)
- [ ] Documentation: `README.md` updated with build instructions per platform; screenshots; keybindings (`?` overlay or help page)
- [ ] Performance: message list virtualized (only render visible + buffer); debounced redraw (16ms); idle CPU <5%, streaming CPU <20%

## Concrete Todo List

- [ ] Confirm `gtk4-rs` and `libadwaita-rs` versions in `Cargo.toml` (target 0.9 / 0.7+)
- [ ] Write `crates/bimo-gui/Cargo.toml`
- [ ] Create `src/lib.rs` with module structure
- [ ] Implement `src/state.rs` with `AppState`
- [ ] Implement `src/bridge.rs` with tokio -> GTK event forwarding
- [ ] Implement `src/app.rs` (`Adw.Application`)
- [ ] Implement `windows/main.rs` skeleton (`NavigationSplitView`)
- [ ] Add sidebar session list widget
- [ ] Add chat message list widget
- [ ] Add multi-line input widget
- [ ] Integrate `AgentBuilder` and `Agent::run()` with event streaming
- [ ] Add settings window with preferences pages
- [ ] Add tool card widget (expandable)
- [ ] Implement undo/redo actions
- [ ] Implement snapshot list and restore
- [ ] Implement compaction preview
- [ ] Implement todo board widget
- [ ] Write Linux build/CI script
- [ ] Write Windows MSYS2/gvsbuild script
- [ ] Write macOS bundle/dmg script
- [ ] Update `README.md` and `crates/bimo-gui/PLAN.md`
- [ ] Final review: no TUI active development; CLI preserved; GUI is primary interface
