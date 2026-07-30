use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, interval};
use tracing::{info, warn};

use super::Session;
use crate::config::Settings;

pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    settings: Settings,
    cleanup_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl SessionManager {
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

        let mut reader = tokio::fs::read_dir(&dir).await?;
        let mut sessions = self.sessions.write().await;

        while let Some(entry) = reader.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => match serde_json::from_str::<Session>(&content) {
                        Ok(session) => {
                            sessions.insert(session.id.clone(), session);
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
        let mut map = sessions.write().await;

        // Remove expired sessions
        let expired_ids: Vec<String> = map
            .iter()
            .filter(|(_, s)| s.is_expired(ttl_hours))
            .map(|(id, _)| id.clone())
            .collect();

        for id in &expired_ids {
            if let Some(session) = map.remove(id) {
                if let Err(e) = session.delete() {
                    warn!("Failed to delete session {}: {}", id, e);
                } else {
                    info!("Cleaned up expired session {}", id);
                }
            }
        }

        // Enforce max sessions (oldest first)
        if map.len() > max_sessions {
            let mut sessions_sorted: Vec<(String, Session)> = map.drain().collect();
            sessions_sorted.sort_by_key(|(_, s)| Reverse(s.updated_at));

            let to_remove = sessions_sorted.split_off(max_sessions);
            for (id, session) in to_remove {
                if let Err(e) = session.delete() {
                    warn!("Failed to delete session {}: {}", id, e);
                }
                info!("Removed excess session {}", id);
            }

            for (id, session) in sessions_sorted {
                map.insert(id, session);
            }
        }
    }

    pub async fn create(&self) -> Session {
        let session = Session::new();
        let mut map = self.sessions.write().await;
        let id = session.id.clone();
        map.insert(id.clone(), session.clone());

        if let Err(e) = session.save() {
            warn!("Failed to save new session {}: {}", id, e);
        }

        session
    }

    pub async fn get(&self, id: &str) -> Option<Session> {
        let map = self.sessions.read().await;
        map.get(id).cloned()
    }

    pub async fn update(&self, session: &Session) -> crate::Result<()> {
        let mut map = self.sessions.write().await;
        map.insert(session.id.clone(), session.clone());
        session.save()?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> crate::Result<()> {
        let mut map = self.sessions.write().await;
        if let Some(session) = map.remove(id) {
            session.delete()?;
        }
        Ok(())
    }

    pub async fn list(&self) -> Vec<Session> {
        let map = self.sessions.read().await;
        let mut sessions: Vec<Session> = map.values().cloned().collect();
        sessions.sort_by_key(|s| Reverse(s.updated_at));
        sessions
    }

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
