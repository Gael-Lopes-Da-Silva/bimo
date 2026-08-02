//! Session model — messages, persistence, and lifecycle management.

mod manager;

pub use manager::SessionManager;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

use crate::tools::{TodoList, is_builtin};

/// A single message in a session conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message id; used to target undo/redo and run records.
    #[serde(default = "default_message_id")]
    pub id: String,
    /// Message author: `"user"`, `"assistant"`, `"system"`, or `"tool"`.
    pub role: String,
    /// Message body.
    pub content: String,
    /// When the message was created.
    pub timestamp: DateTime<Utc>,
}

/// Generates a fresh id for messages loaded from files written before message
/// ids existed.
fn default_message_id() -> String {
    Uuid::new_v4().to_string()
}

/// A batch of conversation messages and run metadata removed by
/// [`Session::undo`], re-applied by [`Session::redo`].
///
/// Each batch corresponds to one undo operation: everything from the cut user
/// prompt to the end of the conversation at that time. Batches are
/// non-overlapping, so popping them in LIFO order always re-appends the
/// messages chronologically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoBatch {
    /// The conversation messages removed by the undo, in order (the first
    /// message is the cut user prompt).
    pub messages: Vec<Message>,
    /// The run snapshot records removed by the undo, in order. Restored on
    /// redo so the session's snapshot history stays in step with its
    /// conversation.
    #[serde(default)]
    pub snapshots: Vec<crate::snapshot::SnapshotRecord>,
}

/// An agent session — persists conversation history and todo state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<Message>,
    /// Messages removed by compaction, one batch per compaction. Kept for
    /// display (e.g. TUI/GUI) but excluded from the agent context.
    #[serde(default)]
    pub archived_messages: Vec<Vec<Message>>,
    pub todo_list: TodoList,
    /// Names of tools disabled for this session.
    #[serde(default)]
    pub disabled_tools: BTreeSet<String>,
    /// Ids of skills disabled for this session.
    #[serde(default)]
    pub disabled_skills: BTreeSet<String>,
    /// Reasoning effort per model id (raw models.dev value, e.g. `"low"`),
    /// applied to runs of that model unless overridden.
    #[serde(default)]
    pub reasoning_efforts: BTreeMap<String, String>,
    /// One record per agent run (oldest first), linking the run's prompt to
    /// the filesystem snapshots captured around it (undo/redo targets).
    #[serde(default)]
    pub snapshots: Vec<crate::snapshot::SnapshotRecord>,
    /// Undo history: one [`UndoBatch`] per undo operation, newest last.
    /// [`Session::redo`] pops from the end. Cleared when a new user prompt is
    /// sent without a redo.
    #[serde(default)]
    pub undo_stack: Vec<UndoBatch>,
    pub metadata: serde_json::Value,
}

impl Session {
    /// Creates a new session with a random id.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            messages: Vec::new(),
            archived_messages: Vec::new(),
            todo_list: TodoList::new(),
            disabled_tools: BTreeSet::new(),
            disabled_skills: BTreeSet::new(),
            reasoning_efforts: BTreeMap::new(),
            snapshots: Vec::new(),
            undo_stack: Vec::new(),
            metadata: serde_json::json!({}),
        }
    }

    /// Adds a message and updates the timestamp. Returns the message id.
    pub fn add_message(&mut self, role: String, content: String) -> String {
        let id = default_message_id();
        self.messages.push(Message {
            id: id.clone(),
            role,
            content,
            timestamp: Utc::now(),
        });
        self.updated_at = Utc::now();
        id
    }

    /// Returns `true` if the session has not been updated within `ttl_hours`.
    pub fn is_expired(&self, ttl_hours: u64) -> bool {
        let elapsed = Utc::now() - self.updated_at;
        elapsed.num_hours() > ttl_hours as i64
    }

    /// Clears all messages in the session and updates the timestamp.
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.updated_at = Utc::now();
    }

    /// Restores a single archived message into the conversation, removing it
    /// from the archive.
    ///
    /// Returns `None` if `batch` or `index` is out of range.
    pub fn restore_archived(&mut self, batch: usize, index: usize) -> Option<Message> {
        if index >= self.archived_messages.get(batch)?.len() {
            return None;
        }
        let msg = self.archived_messages[batch].remove(index);
        if self.archived_messages[batch].is_empty() {
            self.archived_messages.remove(batch);
        }
        self.messages.push(msg.clone());
        self.updated_at = Utc::now();
        Some(msg)
    }

    /// Restores all messages of an archived batch into the conversation,
    /// removing the batch from the archive.
    ///
    /// Returns `None` if `batch` is out of range.
    pub fn restore_archived_batch(&mut self, batch: usize) -> Option<Vec<Message>> {
        if batch >= self.archived_messages.len() {
            return None;
        }
        let restored = self.archived_messages.remove(batch);
        self.messages.extend(restored.clone());
        self.updated_at = Utc::now();
        Some(restored)
    }

    /// Restores every archived message into the conversation, clearing the
    /// archive.
    pub fn restore_all_archived(&mut self) -> Vec<Message> {
        let restored: Vec<Message> = self.archived_messages.drain(..).flatten().collect();
        self.messages.extend(restored.clone());
        self.updated_at = Utc::now();
        restored
    }

    /// Returns the names of tools disabled for this session.
    pub fn disabled_tools(&self) -> &BTreeSet<String> {
        &self.disabled_tools
    }

    /// Returns `true` if the named tool is available in this session.
    pub fn is_tool_enabled(&self, name: &str) -> bool {
        !self.disabled_tools.contains(name)
    }

    /// Disables a tool for this session, returning `true` if it was newly disabled.
    ///
    /// # Errors
    ///
    /// Returns a `BimoError::Tool` if `name` is not a known built-in tool.
    pub fn disable_tool(&mut self, name: &str) -> crate::Result<bool> {
        if !is_builtin(name) {
            return Err(crate::error::CustomError::Tool(format!(
                "Unknown tool '{name}'"
            )));
        }
        self.updated_at = Utc::now();
        Ok(self.disabled_tools.insert(name.to_string()))
    }

    /// Enables a tool for this session, returning `true` if it was newly enabled.
    ///
    /// # Errors
    ///
    /// Returns a `BimoError::Tool` if `name` is not a known built-in tool.
    pub fn enable_tool(&mut self, name: &str) -> crate::Result<bool> {
        if !is_builtin(name) {
            return Err(crate::error::CustomError::Tool(format!(
                "Unknown tool '{name}'"
            )));
        }
        self.updated_at = Utc::now();
        Ok(self.disabled_tools.remove(name))
    }

    /// Returns the ids of skills disabled for this session.
    pub fn disabled_skills(&self) -> &BTreeSet<String> {
        &self.disabled_skills
    }

    /// Returns `true` if the skill with the given `id` is not disabled in this session.
    pub fn is_skill_enabled(&self, id: &str) -> bool {
        !self.disabled_skills.contains(id)
    }

    /// Disables a skill for this session, returning `true` if it was newly disabled.
    ///
    /// The id is recorded regardless of whether the skill is currently loaded
    /// (skills are loaded per project at build time).
    pub fn disable_skill(&mut self, id: &str) -> bool {
        self.updated_at = Utc::now();
        self.disabled_skills.insert(id.to_string())
    }

    /// Enables a skill for this session, returning `true` if it was newly enabled.
    pub fn enable_skill(&mut self, id: &str) -> bool {
        self.updated_at = Utc::now();
        self.disabled_skills.remove(id)
    }

    /// Stores a reasoning effort for the given model id.
    pub fn set_reasoning_effort(&mut self, model: &str, effort: String) {
        self.reasoning_efforts.insert(model.to_string(), effort);
        self.updated_at = Utc::now();
    }

    /// Removes the stored reasoning effort for the given model id, restoring
    /// the provider default for that model.
    pub fn remove_reasoning_effort(&mut self, model: &str) {
        self.reasoning_efforts.remove(model);
        self.updated_at = Utc::now();
    }

    /// Returns the stored reasoning effort for the given model id, if any.
    pub fn reasoning_effort_for(&self, model: &str) -> Option<&str> {
        self.reasoning_efforts.get(model).map(String::as_str)
    }

    /// Returns the directory where session files are stored.
    pub fn sessions_dir() -> std::path::PathBuf {
        crate::paths::config_dir().join("sessions")
    }

    /// Returns the filesystem path for this session.
    pub fn path(&self) -> std::path::PathBuf {
        Self::sessions_dir().join(format!("{}.json", self.id))
    }

    /// Persists this session to disk.
    pub fn save(&self) -> crate::Result<()> {
        let path = self.path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Loads a session by id from disk.
    pub fn load(id: &str) -> crate::Result<Self> {
        let path = Self::sessions_dir().join(format!("{id}.json"));
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Deletes the session file from disk, along with any filesystem
    /// snapshots it owns (best-effort cleanup).
    pub fn delete(&self) -> crate::Result<()> {
        let path = self.path();
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        let mut snapshot_ids: Vec<String> = Vec::new();
        let mut collect = |record: &crate::snapshot::SnapshotRecord| {
            if let Some(id) = &record.id {
                snapshot_ids.push(id.clone());
            }
            if let Some(id) = &record.after {
                snapshot_ids.push(id.clone());
            }
        };
        for record in &self.snapshots {
            collect(record);
        }
        for batch in &self.undo_stack {
            for record in &batch.snapshots {
                collect(record);
            }
        }
        for id in snapshot_ids {
            match crate::snapshot::Snapshot::load(&id) {
                Ok(snapshot) => {
                    if let Err(e) = snapshot.delete() {
                        warn!("Failed to clean up filesystem snapshot {id}: {e}");
                    }
                }
                Err(e) => warn!("Failed to load filesystem snapshot {id} for cleanup: {e}"),
            }
        }
        Ok(())
    }

    /// Best-effort capture of a filesystem snapshot of `project_dir` for the
    /// run triggered by the user message `message_id`, recorded against this
    /// session and persisted with it.
    ///
    /// A run record is always appended, so run tracking (and therefore
    /// undo/redo) works even when no snapshot could be captured (no project
    /// directory, the project is not a git repository, or a persistence
    /// failure); only the filesystem restore is unavailable in that case.
    ///
    /// Returns the snapshot id, or `None` when no snapshot was captured.
    pub fn capture_snapshot(
        &mut self,
        project_dir: Option<&Path>,
        message_id: Option<String>,
    ) -> Option<String> {
        let mut record = crate::snapshot::SnapshotRecord {
            id: None,
            after: None,
            message_id,
            created_at: Utc::now(),
        };
        let captured = project_dir.and_then(|dir| match crate::snapshot::capture_snapshot(dir) {
            Ok(snapshot) => {
                let id = snapshot.id.clone();
                record.id = Some(id.clone());
                Some(id)
            }
            Err(e) => {
                warn!("Filesystem snapshot skipped for session {}: {}", self.id, e);
                None
            }
        });
        self.snapshots.push(record);
        self.updated_at = Utc::now();
        if let Err(e) = self.save() {
            warn!("Failed to persist session {}: {}", self.id, e);
        }
        captured
    }

    /// Starts a new agent run for the user message `message_id`.
    ///
    /// A new user prompt without a redo invalidates the pending undo history,
    /// so it is discarded (and its filesystem snapshots cleaned up) before the
    /// run is recorded. See [`capture_snapshot`](Self::capture_snapshot).
    pub fn begin_run(&mut self, message_id: &str, project_dir: Option<&Path>) -> Option<String> {
        self.clear_undo_stack();
        self.capture_snapshot(project_dir, Some(message_id.to_string()))
    }

    /// Discards the undo history, deleting any filesystem snapshots it still
    /// references (best-effort). Called when a new user prompt is sent without
    /// a redo.
    pub fn clear_undo_stack(&mut self) {
        if self.undo_stack.is_empty() {
            return;
        }
        for batch in self.undo_stack.drain(..) {
            for record in batch.snapshots {
                for id in record.id.iter().chain(record.after.iter()) {
                    match crate::snapshot::Snapshot::load(id) {
                        Ok(snapshot) => {
                            if let Err(e) = snapshot.delete() {
                                warn!("Failed to clean up filesystem snapshot {id}: {e}");
                            }
                        }
                        Err(e) => warn!("Failed to load filesystem snapshot {id} for cleanup: {e}"),
                    }
                }
            }
        }
        self.updated_at = Utc::now();
    }

    /// Links an after-run snapshot to the most recent recorded run (the redo
    /// target for that run).
    pub fn set_after_snapshot(&mut self, id: String) {
        if let Some(last) = self.snapshots.last_mut() {
            last.after = Some(id);
        }
        self.updated_at = Utc::now();
    }

    /// Rewinds the conversation to `target` — the last user message by default,
    /// or the user message with that id — removing it and everything after it.
    ///
    /// The removed messages and their run metadata are pushed onto the undo
    /// stack for [`redo`](Self::redo). When the target is the user message that
    /// triggered a recorded run, the snapshot captured when that message was
    /// sent is applied to the project files; otherwise (a steering message, a
    /// message without a run record) only the conversation is rewound.
    /// Filesystem restore is best-effort: a missing or failing snapshot only
    /// logs a warning and the conversation is still rewound.
    ///
    /// # Errors
    ///
    /// Returns a `BimoError::Session` when there is nothing to undo or `target`
    /// does not match any user message in the conversation.
    pub fn undo(&mut self, target: Option<&str>) -> crate::Result<()> {
        let positions = self.run_message_positions();

        let cut = match target {
            Some(id) => self
                .messages
                .iter()
                .position(|m| m.role == "user" && m.id == id)
                .ok_or_else(|| {
                    crate::error::CustomError::Session(format!(
                        "Cannot undo: no user message with id {id}"
                    ))
                })?,
            None => self
                .messages
                .iter()
                .rposition(|m| m.role == "user")
                .ok_or_else(|| crate::error::CustomError::Session("Nothing to undo".to_string()))?,
        };

        let removed_messages: Vec<Message> = self.messages[cut..].to_vec();
        let first_removed = positions
            .iter()
            .position(|p| p.is_some_and(|pos| pos >= cut));
        let removed_snapshots: Vec<crate::snapshot::SnapshotRecord> = match first_removed {
            Some(ri) => self.snapshots[ri..].to_vec(),
            None => Vec::new(),
        };

        // Apply the snapshot captured when the target message was sent, when
        // the target is the user message of a recorded run.
        if let Some(ri) = positions.iter().position(|p| *p == Some(cut))
            && let Some(id) = self.snapshots.get(ri).and_then(|r| r.id.clone())
        {
            Self::restore_filesystem(&id, "undo");
        }

        self.messages.truncate(cut);
        if let Some(ri) = first_removed {
            self.snapshots.truncate(ri);
        }

        self.undo_stack.push(UndoBatch {
            messages: removed_messages,
            snapshots: removed_snapshots,
        });
        self.updated_at = Utc::now();
        self.save()?;
        Ok(())
    }

    /// Restores the conversation and files removed by the most recent
    /// [`undo`](Self::undo) — or, when `target` is given, by every undo down to
    /// the operation that removed the message with that id.
    ///
    /// The filesystem is reverted to the snapshot captured at the agent loop
    /// end of the last restored run (best-effort). Popping the undo history is
    /// LIFO, so messages always reappear in chronological order.
    ///
    /// # Errors
    ///
    /// Returns a `BimoError::Session` when there is nothing to redo or `target`
    /// does not match any undone message.
    pub fn redo(&mut self, target: Option<&str>) -> crate::Result<()> {
        if self.undo_stack.is_empty() {
            return Err(crate::error::CustomError::Session(
                "Nothing to redo".to_string(),
            ));
        }

        let pop_count = match target {
            None => 1,
            Some(id) => {
                let idx = self
                    .undo_stack
                    .iter()
                    .rposition(|b| b.messages.iter().any(|m| m.id == id))
                    .ok_or_else(|| {
                        crate::error::CustomError::Session(format!(
                            "Cannot redo: no undone message with id {id}"
                        ))
                    })?;
                self.undo_stack.len() - idx
            }
        };

        let mut popped: Vec<UndoBatch> = Vec::with_capacity(pop_count);
        for _ in 0..pop_count {
            popped.push(self.undo_stack.pop().expect("pop_count within bounds"));
        }

        // The last batch popped holds the target message (or the top of the
        // stack); its final run's after-run snapshot is the agent loop end.
        let redo_snapshot = popped
            .last()
            .and_then(|b| b.snapshots.last().and_then(|r| r.after.clone()));
        if let Some(id) = redo_snapshot {
            Self::restore_filesystem(&id, "redo");
        }

        // Batches are popped newest-undo-first, which is chronological order.
        for batch in &popped {
            self.messages.extend(batch.messages.clone());
            self.snapshots.extend(batch.snapshots.clone());
        }

        self.updated_at = Utc::now();
        self.save()?;
        Ok(())
    }

    /// Copies the session into a new, independent session — a full clone from
    /// the last sent message, whether that is the agent loop end or the last
    /// steering message.
    ///
    /// The fork copies the whole context: messages, archived messages, todo
    /// list, run/snapshot metadata and undo history, so undoing or redoing in
    /// the fork never affects the parent session (and vice versa). The
    /// referenced filesystem snapshots are duplicated (new ids, same captured
    /// trees), so deleting the parent — or clearing its undo history with a new
    /// prompt — never breaks the fork's undo/redo file restore, and vice versa.
    /// The fork is persisted with a new id.
    pub fn fork(&self) -> crate::Result<Session> {
        let mut fork = self.clone();
        fork.id = Uuid::new_v4().to_string();
        fork.created_at = Utc::now();
        fork.updated_at = Utc::now();

        Self::duplicate_snapshot_records(&mut fork.snapshots);
        for batch in &mut fork.undo_stack {
            Self::duplicate_snapshot_records(&mut batch.snapshots);
        }

        fork.save()?;
        Ok(fork)
    }

    /// Exports session to Markdown file.
    pub fn export_markdown(&self, path: &std::path::Path) -> crate::Result<()> {
        let mut md = format!(
            "# Session {}\n\nCreated: {}\nUpdated: {}\n\n",
            self.id, self.created_at, self.updated_at
        );
        md.push_str("## Messages\n\n");
        for msg in &self.messages {
            md.push_str(&format!(
                "- **[{}] {}**: {}\n",
                msg.role, msg.timestamp, msg.content
            ));
        }
        if !self.archived_messages.is_empty() {
            md.push_str("\n## Archived messages\n\n");
            for batch in &self.archived_messages {
                for msg in batch {
                    md.push_str(&format!(
                        "- **[{}] {}**: {}\n",
                        msg.role, msg.timestamp, msg.content
                    ));
                }
                md.push('\n');
            }
        }
        std::fs::write(path, md)?;
        Ok(())
    }

    /// Exports session to JSON file.
    pub fn export_json(&self, path: &std::path::Path) -> crate::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Rewrites the snapshot ids referenced by `records` to duplicated copies,
    /// so the owning session never shares snapshot files with another session.
    /// Best-effort: a snapshot that cannot be loaded or duplicated is left
    /// unchanged and only logs a warning.
    fn duplicate_snapshot_records(records: &mut [crate::snapshot::SnapshotRecord]) {
        for record in records {
            for slot in [&mut record.id, &mut record.after] {
                let Some(id) = slot.take() else { continue };
                match crate::snapshot::Snapshot::load(&id).and_then(|s| s.duplicate()) {
                    Ok(copy) => *slot = Some(copy.id),
                    Err(e) => {
                        warn!("Failed to duplicate snapshot {id} for fork: {e}");
                        *slot = Some(id);
                    }
                }
            }
        }
    }

    /// Returns the message index of each run's user message in `messages`,
    /// `None` when the message cannot be located (e.g. the run was archived by
    /// compaction, or the record predates message ids).
    fn run_message_positions(&self) -> Vec<Option<usize>> {
        let index_by_id: std::collections::HashMap<&str, usize> = self
            .messages
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id.as_str(), i))
            .collect();
        self.snapshots
            .iter()
            .map(|r| {
                r.message_id
                    .as_deref()
                    .and_then(|id| index_by_id.get(id).copied())
            })
            .collect()
    }

    /// Best-effort restore of the project filesystem from a snapshot.
    fn restore_filesystem(snapshot_id: &str, operation: &str) {
        match crate::snapshot::Snapshot::load(snapshot_id) {
            Ok(snapshot) => match snapshot.restore() {
                Ok(()) => info!("Restored filesystem from snapshot {snapshot_id} ({operation})"),
                Err(e) => warn!(
                    "Failed to restore filesystem snapshot {snapshot_id} for {operation}: {e}"
                ),
            },
            Err(e) => {
                warn!("Failed to load filesystem snapshot {snapshot_id} for {operation}: {e}")
            }
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
