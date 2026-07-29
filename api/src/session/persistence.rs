use super::Session;
use crate::error::{BimoError, Result};
use std::fs;
use std::path::PathBuf;
use tracing;

impl Session {
    /// Returns the sessions directory (`~/.config/bimo/sessions/`).
    fn sessions_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| BimoError::Session("cannot determine home directory".into()))?;
        let dir = home.join(".config").join("bimo").join("sessions");
        fs::create_dir_all(&dir)
            .map_err(|e| BimoError::Session(format!("failed to create sessions dir: {e}")))?;
        Ok(dir)
    }

    /// Path to this session's JSON file.
    fn session_path(&self) -> Result<PathBuf> {
        Ok(Self::sessions_dir()?.join(format!("{}.json", self.id)))
    }

    /// Save the session to disk.
    pub fn save(&self) -> Result<()> {
        let path = self.session_path()?;
        tracing::debug!(session_id = %self.id, path = %path.display(), "saving session");
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| BimoError::Session(format!("failed to serialize session: {e}")))?;
        fs::write(&path, data)
            .map_err(|e| BimoError::Session(format!("failed to write session file: {e}")))?;
        tracing::debug!(session_id = %self.id, "session saved");
        Ok(())
    }

    /// Load a session from disk by id.
    pub fn load(id: &str) -> Result<Self> {
        let path = Self::sessions_dir()?.join(format!("{id}.json"));
        tracing::debug!(session_id = id, path = %path.display(), "loading session");
        if !path.exists() {
            tracing::warn!(session_id = id, "session file not found");
            return Err(BimoError::Session(format!("session '{id}' not found")));
        }
        let data = fs::read_to_string(&path)
            .map_err(|e| BimoError::Session(format!("failed to read session file: {e}")))?;
        let session: Session = serde_json::from_str(&data)
            .map_err(|e| BimoError::Session(format!("failed to parse session file: {e}")))?;
        tracing::debug!(
            session_id = id,
            message_count = session.message_count(),
            "session loaded"
        );
        Ok(session)
    }

    /// List all saved sessions (returns info summaries, newest first).
    pub fn list_saved() -> Result<Vec<super::SessionInfo>> {
        let dir = Self::sessions_dir()?;
        tracing::debug!(dir = %dir.display(), "listing saved sessions");
        let mut sessions = Vec::new();

        let entries = fs::read_dir(&dir)
            .map_err(|e| BimoError::Session(format!("failed to read sessions dir: {e}")))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json")
                && let Ok(data) = fs::read_to_string(&path)
                && let Ok(session) = serde_json::from_str::<Session>(&data)
            {
                sessions.push(session.info());
            }
        }

        sessions.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        tracing::debug!(count = sessions.len(), "listed saved sessions");
        Ok(sessions)
    }

    /// Delete a saved session from disk.
    pub fn delete_saved(id: &str) -> Result<()> {
        let path = Self::sessions_dir()?.join(format!("{id}.json"));
        tracing::info!(session_id = id, "deleting saved session");
        if !path.exists() {
            tracing::warn!(session_id = id, "session file not found for deletion");
            return Err(BimoError::Session(format!("session '{id}' not found")));
        }
        fs::remove_file(&path)
            .map_err(|e| BimoError::Session(format!("failed to delete session file: {e}")))?;
        tracing::info!(session_id = id, "session deleted");
        Ok(())
    }

    /// Delete all saved sessions from disk.
    pub fn delete_all_saved() -> Result<()> {
        tracing::info!("deleting all saved sessions");
        let dir = Self::sessions_dir()?;
        let entries = fs::read_dir(&dir)
            .map_err(|e| BimoError::Session(format!("failed to read sessions dir: {e}")))?;

        let mut count = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let _ = fs::remove_file(&path);
                count += 1;
            }
        }
        tracing::info!(count, "all saved sessions purged");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_session(prefix: &str) -> Session {
        let mut s = Session::new();
        s.add_user_message(&format!("{prefix} message"));
        s.save().unwrap();
        s
    }

    #[test]
    fn save_and_load_session() {
        let mut s = Session::new();
        s.add_user_message("save test");
        s.save().unwrap();
        let loaded = Session::load(&s.id).unwrap();
        assert_eq!(loaded.id, s.id);
        assert_eq!(loaded.message_count(), 1);
        assert_eq!(loaded.messages[0].content, "save test");
        Session::delete_saved(&s.id).unwrap();
    }

    #[test]
    fn list_saved_returns_all_saved_sessions() {
        let s1 = create_test_session("first");
        let s2 = create_test_session("second");
        let list = Session::list_saved().unwrap();
        assert!(list.len() >= 2);
        assert!(list.iter().any(|info| info.id == s1.id));
        assert!(list.iter().any(|info| info.id == s2.id));
        Session::delete_saved(&s1.id).unwrap();
        Session::delete_saved(&s2.id).unwrap();
    }

    #[test]
    fn list_saved_returns_newest_first() {
        let s1 = create_test_session("older");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let s2 = create_test_session("newer");
        let list = Session::list_saved().unwrap();
        let idx1 = list.iter().position(|i| i.id == s1.id).unwrap();
        let idx2 = list.iter().position(|i| i.id == s2.id).unwrap();
        assert!(idx2 < idx1, "newer session should appear first");
        Session::delete_saved(&s1.id).unwrap();
        Session::delete_saved(&s2.id).unwrap();
    }

    #[test]
    fn delete_all_saved_purges_sessions() {
        let s1 = create_test_session("purge1");
        let s2 = create_test_session("purge2");
        Session::delete_all_saved().unwrap();
        assert!(Session::load(&s1.id).is_err());
        assert!(Session::load(&s2.id).is_err());
    }

    #[test]
    fn save_and_load_empty_session() {
        let s = Session::new();
        s.save().unwrap();
        let loaded = Session::load(&s.id).unwrap();
        assert_eq!(loaded.message_count(), 0);
        Session::delete_saved(&s.id).unwrap();
    }
}
