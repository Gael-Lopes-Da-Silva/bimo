//! Session lifecycle manager — in-memory cache, persistence, periodic cleanup.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, interval};
use tracing::{info, warn};

use super::Session;
use crate::config::Settings;

/// Manages session lifecycle: creation, retrieval, persistence, and cleanup.
///
/// Sessions are held in memory and periodically persisted to disk.  Expired
/// sessions are removed by a background task.
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    settings: Settings,
    cleanup_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl SessionManager {
    /// Creates a new manager, loads existing sessions from disk, and starts
    /// the background cleanup task.
    pub async fn new(settings: Settings) -> crate::Result<Self> {
        let manager = Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            settings,
            cleanup_handle: Arc::new(Mutex::new(None)),
        };
        manager.load_existing().await?;
        manager.start_cleanup_task();
        Ok(manager)
    }

    async fn load_existing(&self) -> crate::Result<()> {
        let dir = Session::sessions_dir();
        if !dir.exists() {
            return Ok(());
        }

        let max_load = self.settings.max_sessions;
        let mut reader = tokio::fs::read_dir(&dir).await?;
        let mut sessions = self.sessions.write().await;
        let mut loaded = 0;

        while let Some(entry) = reader.next_entry().await? {
            if loaded >= max_load {
                warn!(
                    "Reached maximum session limit ({}), stopping load",
                    max_load
                );
                break;
            }

            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => match serde_json::from_str::<Session>(&content) {
                        Ok(session) => {
                            sessions.insert(session.id.clone(), session);
                            loaded += 1;
                        }
                        Err(e) => {
                            warn!("Failed to parse session file {:?}: {}", path, e);
                        }
                    },
                    Err(e) => {
                        warn!("Failed to read session file {:?}: {}", path, e);
                    }
                }
            }
        }

        info!("Loaded {} sessions", sessions.len());
        Ok(())
    }

    fn start_cleanup_task(&self) {
        let sessions = self.sessions.clone();
        let ttl = self.settings.session_ttl_hours;
        let max_sessions = self.settings.max_sessions;
        let interval_minutes = self.settings.cleanup_interval_minutes;

        let handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_minutes * 60));
            loop {
                ticker.tick().await;
                Self::run_cleanup(&sessions, ttl, max_sessions).await;
            }
        });

        let mut cleanup_handle = self.cleanup_handle.blocking_lock();
        *cleanup_handle = Some(handle);
    }

    async fn run_cleanup(
        sessions: &Arc<RwLock<HashMap<String, Session>>>,
        ttl_hours: u64,
        max_sessions: usize,
    ) {
        let expired_ids: Vec<String>;
        let to_remove_excess: Vec<(String, Session)>;

        {
            let map = sessions.read().await;
            expired_ids = map
                .iter()
                .filter(|(_, s)| s.is_expired(ttl_hours))
                .map(|(id, _)| id.clone())
                .collect();
        }

        for id in &expired_ids {
            let session = {
                let mut map = sessions.write().await;
                map.remove(id)
            };
            if let Some(session) = session {
                if let Err(e) = session.delete() {
                    warn!("Failed to delete session {}: {}", id, e);
                } else {
                    info!("Cleaned up expired session {}", id);
                }
            }
        }

        {
            let mut map = sessions.write().await;
            if map.len() > max_sessions {
                let mut sessions_sorted: Vec<(String, Session)> = map.drain().collect();
                sessions_sorted.sort_by_key(|(_, s)| Reverse(s.updated_at));

                to_remove_excess = sessions_sorted.split_off(max_sessions);

                for (id, session) in sessions_sorted {
                    map.insert(id, session);
                }
            } else {
                to_remove_excess = Vec::new();
            }
        }

        for (id, session) in to_remove_excess {
            if let Err(e) = session.delete() {
                warn!("Failed to delete session {}: {}", id, e);
            }
            info!("Removed excess session {}", id);
        }
    }

    /// Creates a new session, persists it, and returns it.
    /// Returns an error if the session cannot be saved.
    pub async fn create(&self) -> crate::Result<Session> {
        let session = Session::new();
        let mut map = self.sessions.write().await;
        let id = session.id.clone();
        map.insert(id.clone(), session.clone());

        session.save()?;

        Ok(session)
    }

    /// Retrieves a session by id.
    pub async fn get(&self, id: &str) -> Option<Session> {
        let map = self.sessions.read().await;
        map.get(id).cloned()
    }

    /// Updates and persists a session.
    pub async fn update(&self, session: &Session) -> crate::Result<()> {
        let mut map = self.sessions.write().await;
        map.insert(session.id.clone(), session.clone());
        session.save()?;
        Ok(())
    }

    /// Deletes a session by id.
    pub async fn delete(&self, id: &str) -> crate::Result<()> {
        let mut map = self.sessions.write().await;
        if let Some(session) = map.remove(id) {
            session.delete()?;
        }
        Ok(())
    }

    /// Lists all sessions, most recently updated first.
    pub async fn list(&self) -> Vec<Session> {
        let map = self.sessions.read().await;
        let mut sessions: Vec<Session> = map.values().cloned().collect();
        sessions.sort_by_key(|s| Reverse(s.updated_at));
        sessions
    }

    /// Runs the cleanup logic immediately (used for testing).
    pub async fn run_cleanup_now(&self) {
        let ttl = self.settings.session_ttl_hours;
        let max = self.settings.max_sessions;
        Self::run_cleanup(&self.sessions, ttl, max).await;
    }
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        if let Ok(mut handle) = self.cleanup_handle.try_lock()
            && let Some(h) = handle.take()
        {
            h.abort();
        }
    }
}
