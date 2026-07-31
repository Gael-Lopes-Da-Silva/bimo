//! Git-backed filesystem snapshots for reverting agent file changes.
//!
//! A [`Snapshot`] records the full working-tree state of a git repository
//! (tracked, modified, and untracked non-ignored files) as a commit object in
//! that repository. [`Snapshot::restore`] rewinds the working tree to the
//! captured state: files modified after the snapshot are overwritten, files
//! deleted after it are recreated, and files created after it are removed.
//! Git-ignored paths such as build artifacts are left untouched.
//!
//! Snapshots depend on git and therefore only work inside git repositories.
//! When a snapshot cannot be captured — the project is not a repository, git
//! is unavailable, ... — callers degrade gracefully: the conversation can
//! still be rewound, only the filesystem cannot be restored.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::error::{BimoError, Result};

/// Snapshot commits are pinned under this ref namespace so `git gc` cannot
/// prune them between capture and restore.
const SNAPSHOT_REF_PREFIX: &str = "refs/bimo/snapshots/";

/// A point-in-time capture of a project's filesystem, backed by a commit in
/// the project's own git repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Unique snapshot id; also used as the git ref suffix.
    pub id: String,
    /// Absolute path to the git repository root that was captured.
    pub repo_root: PathBuf,
    /// Commit object holding the captured tree.
    pub commit: String,
    /// When the snapshot was captured.
    pub created_at: DateTime<Utc>,
}

/// Metadata for a single agent run, linking its triggering user message to the
/// filesystem snapshots captured around it so the UI can undo/redo file
/// changes per message.
///
/// One record is recorded per run, even when no filesystem snapshot could be
/// captured; the [`id`](Self::id) is then `None` and undo/redo still rewinds
/// the conversation, just not the files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRecord {
    /// Snapshot id captured before the run (the undo target). `None` when no
    /// snapshot was captured (snapshots disabled, not a git repository, ...).
    #[serde(default)]
    pub id: Option<String>,
    /// Optional snapshot id captured after the run (the redo target).
    #[serde(default)]
    pub after: Option<String>,
    /// Id of the user message that triggered the run, if known. `None` for
    /// records written before message ids existed; those are inert for
    /// undo/redo targeting.
    #[serde(default)]
    pub message_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Snapshot {
    /// Captures the current state of `project_dir`.
    ///
    /// # Errors
    ///
    /// Returns an error when `project_dir` is not inside a git repository,
    /// git is unavailable, or the repository cannot be read.
    pub fn capture(project_dir: &Path) -> Result<Self> {
        let repo_root = PathBuf::from(git_ok(project_dir, &["rev-parse", "--show-toplevel"])?);
        let tree = capture_tree(&repo_root)?;
        let commit = commit_tree(&repo_root, &tree)?;
        let id = Uuid::new_v4().to_string();
        // Pin the commit so background `git gc` cannot prune it.
        git_ok(
            &repo_root,
            &["update-ref", &format!("{SNAPSHOT_REF_PREFIX}{id}"), &commit],
        )?;
        Ok(Self {
            id,
            repo_root,
            commit,
            created_at: Utc::now(),
        })
    }

    /// Restores the working tree to the captured state.
    ///
    /// The git index is left untouched; only working-tree files are changed.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot commit is no longer available or git
    /// operations fail.
    pub fn restore(&self) -> Result<()> {
        // Fail fast if the snapshot commit was pruned.
        git_ok(&self.repo_root, &["cat-file", "-e", &self.commit])?;

        git_ok(
            &self.repo_root,
            &["restore", "--source", &self.commit, "--worktree", "--", "."],
        )?;

        let snapshot_files = self.file_set()?;
        self.remove_extra_files(&snapshot_files)
    }

    /// Creates an independent copy of this snapshot: a fresh id pinned to the
    /// same captured tree and persisted as a new metadata file. No new git
    /// objects are created — only a ref and a metadata file. The original and
    /// the copy never share refs or files, so deleting either is safe.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot commit can no longer be pinned (e.g.
    /// it was pruned) or git operations fail.
    pub fn duplicate(&self) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        git_ok(
            &self.repo_root,
            &[
                "update-ref",
                &format!("{SNAPSHOT_REF_PREFIX}{id}"),
                &self.commit,
            ],
        )?;
        let copy = Self {
            id: id.clone(),
            repo_root: self.repo_root.clone(),
            commit: self.commit.clone(),
            created_at: Utc::now(),
        };
        copy.save()?;
        Ok(copy)
    }

    /// Returns the directory where snapshot metadata files are stored.
    pub fn snapshots_dir() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"));
        base.join("bimo").join("snapshots")
    }

    /// Returns the metadata file path for this snapshot.
    pub fn path(&self) -> PathBuf {
        Self::snapshots_dir().join(format!("{}.json", self.id))
    }

    /// Persists this snapshot's metadata to disk.
    pub fn save(&self) -> Result<()> {
        let path = self.path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Loads a snapshot by id from disk.
    pub fn load(id: &str) -> Result<Self> {
        let path = Self::snapshots_dir().join(format!("{id}.json"));
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Removes this snapshot: deletes its metadata file and drops the git ref
    /// that pinned the commit (best-effort).
    pub fn delete(&self) -> Result<()> {
        let _ = git_output(
            &self.repo_root,
            &[
                "update-ref",
                "-d",
                &format!("{SNAPSHOT_REF_PREFIX}{}", self.id),
            ],
        );
        let path = self.path();
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// The set of files (repo-root-relative paths) captured in the snapshot.
    fn file_set(&self) -> Result<HashSet<PathBuf>> {
        let out = git_output(
            &self.repo_root,
            &["ls-tree", "-r", "--name-only", "-z", &self.commit],
        )?;
        if !out.status.success() {
            return Err(git_failed(&["ls-tree"], &out));
        }
        let mut files = HashSet::new();
        for path in out.stdout.split(|b| *b == 0) {
            if path.is_empty() {
                continue;
            }
            files.insert(PathBuf::from(String::from_utf8_lossy(path).into_owned()));
        }
        Ok(files)
    }

    /// Deletes files present in the working tree but not in the snapshot, so
    /// the tree ends up matching the captured state exactly. Git-ignored paths
    /// are preserved.
    fn remove_extra_files(&self, snapshot_files: &HashSet<PathBuf>) -> Result<()> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        let mut dirs: Vec<PathBuf> = Vec::new();
        collect_extra_files(
            &self.repo_root,
            &self.repo_root,
            snapshot_files,
            &mut candidates,
            &mut dirs,
        )?;

        let ignored = self.ignored_paths(&candidates)?;

        for rel in &candidates {
            if ignored.contains(rel) {
                continue;
            }
            let full = self.repo_root.join(rel);
            match std::fs::remove_file(&full) {
                Ok(()) => debug!("Removed {} (not in snapshot)", full.display()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => warn!("Failed to remove {}: {e}", full.display()),
            }
        }

        // Drop directories the agent created, deepest first. Non-empty
        // directories (or removal failures) are simply left in place.
        dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
        for dir in dirs {
            if let Err(e) = std::fs::remove_dir(&dir)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                debug!("Kept non-empty directory {}: {e}", dir.display());
            }
        }
        Ok(())
    }

    /// Returns the subset of `candidates` (repo-root-relative paths) that are
    /// matched by the repository's ignore rules.
    fn ignored_paths(&self, candidates: &[PathBuf]) -> Result<HashSet<PathBuf>> {
        let mut ignored = HashSet::new();
        if candidates.is_empty() {
            return Ok(ignored);
        }

        let mut input = Vec::with_capacity(candidates.len() * 64);
        for rel in candidates {
            input.extend_from_slice(rel.as_os_str().as_encoded_bytes());
            input.push(0);
        }

        let mut child = Command::new("git")
            .current_dir(&self.repo_root)
            .args(["check-ignore", "-z", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| BimoError::Other(format!("failed to run `git check-ignore`: {e}")))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| BimoError::Other("failed to open git stdin".into()))?;
        stdin
            .write_all(&input)
            .map_err(|e| BimoError::Other(format!("failed to write to git check-ignore: {e}")))?;
        drop(stdin);

        let output = child
            .wait_with_output()
            .map_err(|e| BimoError::Other(format!("failed to run `git check-ignore`: {e}")))?;

        for path in output.stdout.split(|b| *b == 0) {
            if path.is_empty() {
                continue;
            }
            ignored.insert(PathBuf::from(String::from_utf8_lossy(path).into_owned()));
        }
        Ok(ignored)
    }
}

/// Captures and persists a filesystem snapshot of `project_dir`.
pub fn capture_snapshot(project_dir: &Path) -> Result<Snapshot> {
    let snapshot = Snapshot::capture(project_dir)?;
    snapshot.save()?;
    Ok(snapshot)
}

/// Captures the working tree of `repo_root` into a git tree object.
///
/// A temporary index is used so the user's real index is left untouched. The
/// index is seeded from `HEAD` so tracked-file deletions are captured too;
/// fresh repositories without a `HEAD` start from an empty index.
fn capture_tree(repo_root: &Path) -> Result<String> {
    let index_path = std::env::temp_dir().join(format!("bimo-index-{}.idx", Uuid::new_v4()));
    let index = index_path
        .to_str()
        .ok_or_else(|| BimoError::Other("temporary index path is not valid UTF-8".into()))?;

    let read = git_output(repo_root, &["read-tree", "HEAD"]);
    if !matches!(read, Ok(out) if out.status.success()) {
        // No HEAD yet — start from an empty index.
        let _ = std::fs::remove_file(&index_path);
    }

    let add = Command::new("git")
        .env("GIT_INDEX_FILE", index)
        .current_dir(repo_root)
        .args(["add", "-A"])
        .output()
        .map_err(|e| BimoError::Other(format!("failed to run `git add -A`: {e}")))?;
    if !add.status.success() {
        let _ = std::fs::remove_file(&index_path);
        return Err(git_failed(&["add", "-A"], &add));
    }

    let write = Command::new("git")
        .env("GIT_INDEX_FILE", index)
        .current_dir(repo_root)
        .args(["write-tree"])
        .output()
        .map_err(|e| BimoError::Other(format!("failed to run `git write-tree`: {e}")))?;
    let _ = std::fs::remove_file(&index_path);

    if !write.status.success() {
        return Err(git_failed(&["write-tree"], &write));
    }
    Ok(String::from_utf8_lossy(&write.stdout).trim().to_string())
}

/// Turns a captured tree into a commit object so it can be pinned by a ref.
fn commit_tree(repo_root: &Path, tree: &str) -> Result<String> {
    let out = git_output_env(
        repo_root,
        &["commit-tree", tree, "-m", "bimo snapshot"],
        &[
            ("GIT_AUTHOR_NAME", "bimo"),
            ("GIT_AUTHOR_EMAIL", "bimo@localhost"),
            ("GIT_COMMITTER_NAME", "bimo"),
            ("GIT_COMMITTER_EMAIL", "bimo@localhost"),
        ],
    )?;
    if !out.status.success() {
        return Err(git_failed(&["commit-tree"], &out));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Recursively collects working-tree files not present in the snapshot and
/// every directory under `dir` (for empty-directory cleanup). Nested git
/// repositories / submodules are not descended into.
fn collect_extra_files(
    repo_root: &Path,
    dir: &Path,
    snapshot_files: &HashSet<PathBuf>,
    candidates: &mut Vec<PathBuf>,
    dirs: &mut Vec<PathBuf>,
) -> Result<()> {
    let entries = std::fs::read_dir(dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name() == ".git" {
            continue;
        }

        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if path.join(".git").exists() {
                continue;
            }
            collect_extra_files(repo_root, &path, snapshot_files, candidates, dirs)?;
            dirs.push(path);
        } else {
            let rel = path.strip_prefix(repo_root).map_err(|e| {
                BimoError::Other(format!("path {path:?} is outside {repo_root:?}: {e}"))
            })?;
            if !snapshot_files.contains(rel) {
                candidates.push(rel.to_path_buf());
            }
        }
    }
    Ok(())
}

fn git_ok(repo_root: &Path, args: &[&str]) -> Result<String> {
    let out = git_output(repo_root, args)?;
    if !out.status.success() {
        return Err(git_failed(args, &out));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn git_output(repo_root: &Path, args: &[&str]) -> Result<Output> {
    git_output_env(repo_root, args, &[])
}

fn git_output_env(repo_root: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<Output> {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_root).args(args);
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.output()
        .map_err(|e| BimoError::Other(format!("failed to run `git {}`: {e}", args.join(" "))))
}

fn git_failed(args: &[&str], out: &Output) -> BimoError {
    BimoError::Other(format!(
        "`git {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr).trim()
    ))
}
