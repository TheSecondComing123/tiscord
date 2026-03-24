use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use twilight_model::id::{marker::UserMarker, Id};

use crate::config::Config;

/// Persistent user notes stored as a JSON file in the data directory.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserNotes {
    notes: HashMap<String, String>,
}

impl UserNotes {
    fn file_path() -> PathBuf {
        Config::data_dir().join("notes.json")
    }

    /// Load notes from disk, returning empty notes on any error.
    pub fn load() -> Self {
        let path = Self::file_path();
        if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Persist notes to disk.
    pub fn save(&self) {
        let path = Self::file_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn get(&self, user_id: Id<UserMarker>) -> Option<&str> {
        self.notes.get(&user_id.get().to_string()).map(|s| s.as_str())
    }

    pub fn set(&mut self, user_id: Id<UserMarker>, note: String) {
        if note.is_empty() {
            self.notes.remove(&user_id.get().to_string());
        } else {
            self.notes.insert(user_id.get().to_string(), note);
        }
        self.save();
    }

    pub fn remove(&mut self, user_id: Id<UserMarker>) {
        self.notes.remove(&user_id.get().to_string());
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let mut notes = UserNotes::default();
        let user_id = Id::new(12345);
        assert!(notes.get(user_id).is_none());
        notes.notes.insert(user_id.get().to_string(), "test note".to_string());
        assert_eq!(notes.get(user_id), Some("test note"));
    }

    #[test]
    fn test_set_empty_removes() {
        let mut notes = UserNotes::default();
        let user_id = Id::new(12345);
        notes.notes.insert(user_id.get().to_string(), "note".to_string());
        notes.set(user_id, String::new());
        assert!(notes.get(user_id).is_none());
    }
}
