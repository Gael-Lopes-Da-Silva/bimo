use super::{Session, SessionInfo};
use crate::error::{BimoError, Result};
use std::collections::HashMap;
use tracing;

/// Manages multiple sessions in memory with auto-persistence to disk.
///
/// The "active" session is the one loaded into `Agent.session`.
/// All other sessions live in the `sessions` map and are persisted to disk.
pub struct SessionManager {
    sessions: HashMap<String, Session>,
    active_id: String,
}

impl SessionManager {
    /// Create a new manager with the given initial session set as active.
    pub fn new(initial: Session) -> Self {
        let id = initial.id.clone();
        let mut sessions = HashMap::new();
        sessions.insert(id.clone(), initial);
        tracing::info!(session_id = %id, "SessionManager initialized");
        Self {
            sessions,
            active_id: id,
        }
    }

    /// The id of the currently active session.
    pub fn active_id(&self) -> &str {
        &self.active_id
    }

    /// Get a reference to the active session.
    pub fn active(&self) -> Option<&Session> {
        self.sessions.get(&self.active_id)
    }

    /// Get a mutable reference to the active session.
    pub fn active_mut(&mut self) -> Option<&mut Session> {
        self.sessions.get_mut(&self.active_id)
    }

    /// List info for all sessions in the pool.
    pub fn list(&self) -> Vec<SessionInfo> {
        let mut infos: Vec<SessionInfo> = self.sessions.values().map(|s| s.info()).collect();
        infos.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        infos
    }

    /// Get a reference to a session by id.
    pub fn get(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    /// Get a mutable reference to a session by id.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }

    /// Insert a session into the pool. Returns its id.
    pub fn insert(&mut self, session: Session) -> String {
        let id = session.id.clone();
        tracing::info!(session_id = %id, "inserting session into pool");
        self.sessions.insert(id.clone(), session);
        id
    }

    /// Remove a session from the pool by id.
    /// Cannot remove the active session — switch first.
    pub fn remove(&mut self, id: &str) -> Result<Session> {
        if id == self.active_id {
            return Err(BimoError::Session(
                "cannot delete the active session; switch to another session first".into(),
            ));
        }
        self.sessions
            .remove(id)
            .ok_or_else(|| BimoError::Session(format!("session '{id}' not found in pool")))
    }

    /// Switch the active session. The caller is responsible for saving
    /// the current session and loading the new one into `Agent.session`.
    /// This only updates the manager's internal bookkeeping.
    pub fn set_active(&mut self, id: &str) -> Result<()> {
        if !self.sessions.contains_key(id) {
            return Err(BimoError::Session(format!(
                "session '{id}' not found in pool"
            )));
        }
        tracing::info!(from = %self.active_id, to = %id, "switching active session");
        self.active_id = id.to_string();
        Ok(())
    }

    /// Save all sessions in the pool to disk.
    pub fn save_all(&self) -> Result<()> {
        for session in self.sessions.values() {
            session.save()?;
        }
        tracing::debug!(count = self.sessions.len(), "saved all sessions to disk");
        Ok(())
    }

    /// Save a specific session to disk.
    pub fn save_session(&self, id: &str) -> Result<()> {
        if let Some(session) = self.sessions.get(id) {
            session.save()?;
        }
        Ok(())
    }

    /// Save the active session to disk.
    pub fn save_active(&self) -> Result<()> {
        self.save_session(&self.active_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(name: &str) -> Session {
        let mut s = Session::new();
        s.add_user_message(name);
        s
    }

    #[test]
    fn new_manager_has_one_session() {
        let s = make_session("hello");
        let id = s.id.clone();
        let mgr = SessionManager::new(s);
        assert_eq!(mgr.active_id(), &id);
        assert_eq!(mgr.list().len(), 1);
    }

    #[test]
    fn insert_and_get() {
        let s1 = make_session("first");
        let id1 = s1.id.clone();
        let mut mgr = SessionManager::new(s1);

        let s2 = make_session("second");
        let id2 = s2.id.clone();
        mgr.insert(s2);

        assert_eq!(mgr.list().len(), 2);
        assert!(mgr.get(&id1).is_some());
        assert!(mgr.get(&id2).is_some());
    }

    #[test]
    fn set_active() {
        let s1 = make_session("first");
        let id1 = s1.id.clone();
        let mut mgr = SessionManager::new(s1);

        let s2 = make_session("second");
        let id2 = s2.id.clone();
        mgr.insert(s2);

        mgr.set_active(&id2).unwrap();
        assert_eq!(mgr.active_id(), &id2);
        assert!(mgr.active().unwrap().messages[0].content == "second");
    }

    #[test]
    fn set_active_unknown_id_errors() {
        let s = make_session("first");
        let mut mgr = SessionManager::new(s);
        assert!(mgr.set_active("nonexistent").is_err());
    }

    #[test]
    fn remove_non_active() {
        let s1 = make_session("first");
        let mut mgr = SessionManager::new(s1);

        let s2 = make_session("second");
        let id2 = s2.id.clone();
        mgr.insert(s2);

        let removed = mgr.remove(&id2).unwrap();
        assert_eq!(removed.messages[0].content, "second");
        assert_eq!(mgr.list().len(), 1);
    }

    #[test]
    fn remove_active_errors() {
        let s = make_session("first");
        let id = s.id.clone();
        let mut mgr = SessionManager::new(s);
        assert!(mgr.remove(&id).is_err());
    }

    #[test]
    fn remove_unknown_errors() {
        let s = make_session("first");
        let mut mgr = SessionManager::new(s);
        assert!(mgr.remove("nonexistent").is_err());
    }

    #[test]
    fn list_sorted_by_updated_at() {
        let mut mgr = SessionManager::new(make_session("first"));
        let s2 = mgr.active().unwrap().clone();
        let mut new_session = Session::new();
        new_session.add_user_message("second");
        // Ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(10));
        mgr.insert(new_session);

        let list = mgr.list();
        assert_eq!(list.len(), 2);
        // Most recently updated should be first
        assert!(list[0].updated_at >= list[1].updated_at);
    }

    #[test]
    fn active_mut_modifies_session() {
        let mut mgr = SessionManager::new(make_session("before"));
        mgr.active_mut().unwrap().add_user_message("after");
        assert_eq!(mgr.active().unwrap().message_count(), 2);
    }

    #[test]
    fn save_all_persists_to_disk() {
        let mut mgr = SessionManager::new(make_session("test-save"));
        let id = mgr.active_id().to_string();
        mgr.save_all().unwrap();
        let loaded = Session::load(&id).unwrap();
        assert_eq!(loaded.messages[0].content, "test-save");
        // cleanup
        let _ = Session::delete_saved(&id);
    }
}
