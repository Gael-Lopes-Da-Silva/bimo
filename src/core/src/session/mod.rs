//! Session model — messages, persistence, and lifecycle management.

mod manager;

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
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
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
    /// Filesystem snapshots captured before agent runs, oldest first. Used to
    /// revert file changes when rewinding the conversation (undo/redo).
    #[serde(default)]
    pub snapshots: Vec<crate::snapshot::SnapshotRecord>,
    /// Id of the filesystem snapshot captured alongside the last checkpoint.
    #[serde(default)]
    pub checkpoint_snapshot: Option<String>,
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
            checkpoint_snapshot: None,
            metadata: serde_json::json!({}),
        }
    }

    /// Adds a message and updates the timestamp.
    pub fn add_message(&mut self, role: String, content: String) {
        self.messages.push(Message {
            role,
            content,
            timestamp: Utc::now(),
        });
        self.updated_at = Utc::now();
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
            return Err(crate::error::BimoError::Tool(format!(
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
            return Err(crate::error::BimoError::Tool(format!(
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
        let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("~/.config"));
        base.join("bimo").join("sessions")
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
        let mut snapshot_ids: Vec<String> = self.snapshots.iter().map(|s| s.id.clone()).collect();
        if let Some(id) = &self.checkpoint_snapshot {
            snapshot_ids.push(id.clone());
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

    /// Best-effort capture of a filesystem snapshot of `project_dir`,
    /// recorded against this session for later revert and persisted with it.
    ///
    /// Returns the snapshot id, or `None` when no snapshot could be captured
    /// (no project directory, the project is not a git repository, or a
    /// persistence failure). Snapshots depend on git; the conversation can
    /// still be reverted even when this returns `None`, only the filesystem
    /// cannot.
    pub fn capture_snapshot(
        &mut self,
        project_dir: Option<&Path>,
        prompt: Option<String>,
    ) -> Option<String> {
        let dir = project_dir?;
        match crate::snapshot::capture_snapshot(dir) {
            Ok(snapshot) => {
                let id = snapshot.id.clone();
                let created_at = snapshot.created_at;
                self.snapshots.push(crate::snapshot::SnapshotRecord {
                    id: id.clone(),
                    after: None,
                    prompt,
                    created_at,
                });
                self.updated_at = Utc::now();
                if let Err(e) = self.save() {
                    warn!("Failed to persist session {}: {}", self.id, e);
                }
                Some(id)
            }
            Err(e) => {
                warn!("Filesystem snapshot skipped for session {}: {}", self.id, e);
                None
            }
        }
    }

    /// Links an after-run snapshot to the most recent recorded run (the redo
    /// target for that run).
    pub fn set_after_snapshot(&mut self, id: String) {
        if let Some(last) = self.snapshots.last_mut() {
            last.after = Some(id);
        }
        self.updated_at = Utc::now();
    }

    /// Creates a branched checkpoint from the current session state.
    ///
    /// When `project_dir` is given and is a git repository, a filesystem
    /// snapshot is captured alongside the checkpoint so
    /// [`restore_checkpoint`](Self::restore_checkpoint) can revert the
    /// project's files as well as the conversation.
    pub fn branch_checkpoint(
        &self,
        branch_id: &str,
        project_dir: Option<&Path>,
    ) -> crate::Result<Self> {
        let mut branch = self.clone();
        branch.id = format!("{}_{}", self.id, branch_id);
        if let Some(dir) = project_dir {
            match crate::snapshot::Snapshot::capture(dir) {
                Ok(snapshot) => {
                    let id = snapshot.id.clone();
                    if let Err(e) = snapshot.save() {
                        warn!("Failed to persist filesystem snapshot {id}: {e}");
                    } else {
                        branch.checkpoint_snapshot = Some(id);
                    }
                }
                Err(e) => {
                    warn!("No filesystem snapshot captured for checkpoint {branch_id}: {e}");
                }
            }
        }
        branch.save()?;
        Ok(branch)
    }

    /// Restores a session from a checkpoint file by id.
    ///
    /// When the checkpoint captured a filesystem snapshot, the project working
    /// tree is reverted to that snapshot before the session is returned.
    /// Filesystem restore is best-effort: if the snapshot is missing or fails
    /// to restore, the conversation is still restored and a warning is logged.
    pub fn restore_checkpoint(checkpoint_id: &str) -> crate::Result<Self> {
        let session = Self::load(checkpoint_id)?;
        if let Some(snapshot_id) = session.checkpoint_snapshot.as_deref() {
            match crate::snapshot::Snapshot::load(snapshot_id) {
                Ok(snapshot) => match snapshot.restore() {
                    Ok(()) => info!("Restored filesystem from snapshot {snapshot_id}"),
                    Err(e) => warn!(
                        "Failed to restore filesystem snapshot {snapshot_id} for checkpoint {checkpoint_id}: {e}"
                    ),
                },
                Err(e) => warn!(
                    "Failed to load filesystem snapshot {snapshot_id} for checkpoint {checkpoint_id}: {e}"
                ),
            }
        }
        Ok(session)
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
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

pub use manager::SessionManager;
