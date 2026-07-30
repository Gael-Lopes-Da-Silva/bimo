use crate::error::{BimoError, Result};
use crate::session::Session;
use std::fs;
use std::path::PathBuf;

impl Session {
    fn sessions_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| BimoError::Session("cannot determine home directory".into()))?;
        let dir = home.join(".config").join("bimo").join("sessions");
        fs::create_dir_all(&dir)
            .map_err(|e| BimoError::Session(format!("failed to create sessions dir: {e}")))?;
        Ok(dir)
    }

    fn session_path(&self) -> Result<PathBuf> {
        Ok(Self::sessions_dir()?.join(format!("{}.json", self.id)))
    }

    pub fn save(&self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let path = self.session_path()?;
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| BimoError::Session(format!("failed to serialize session: {e}")))?;
        fs::write(&path, data)
            .map_err(|e| BimoError::Session(format!("failed to write session file: {e}")))?;
        Ok(())
    }

    pub fn load(id: &str) -> Result<Self> {
        let path = Self::sessions_dir()?.join(format!("{id}.json"));
        if !path.exists() {
            return Err(BimoError::Session(format!("session '{id}' not found")));
        }
        let data = fs::read_to_string(&path)
            .map_err(|e| BimoError::Session(format!("failed to read session file: {e}")))?;
        let session: Session = serde_json::from_str(&data)
            .map_err(|e| BimoError::Session(format!("failed to parse session file: {e}")))?;
        Ok(session)
    }

    pub fn list_saved() -> Result<Vec<crate::session::SessionInfo>> {
        let dir = Self::sessions_dir()?;
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
        Ok(sessions)
    }

    pub fn delete_saved(id: &str) -> Result<()> {
        let path = Self::sessions_dir()?.join(format!("{id}.json"));
        if !path.exists() {
            return Err(BimoError::Session(format!("session '{id}' not found")));
        }
        fs::remove_file(&path)
            .map_err(|e| BimoError::Session(format!("failed to delete session file: {e}")))?;
        Ok(())
    }

    pub fn delete_all_saved() -> Result<()> {
        let dir = Self::sessions_dir()?;
        let entries = fs::read_dir(&dir)
            .map_err(|e| BimoError::Session(format!("failed to read sessions dir: {e}")))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let _ = fs::remove_file(&path);
            }
        }
        Ok(())
    }
}
