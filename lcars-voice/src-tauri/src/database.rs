//! SQLite storage for transcription history.

use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcription {
    pub id: i64,
    pub text: String,
    pub timestamp: String,
    pub duration_ms: Option<i64>,
    pub model: String,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new() -> Result<Self> {
        let db_path = Self::get_db_path();

        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(&db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS transcriptions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                duration_ms INTEGER,
                model TEXT DEFAULT 'base'
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    fn get_db_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("lcars-voice")
            .join("history.db")
    }

    pub fn add_transcription(
        &self,
        text: &str,
        duration_ms: Option<i64>,
        model: &str,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO transcriptions (text, duration_ms, model) VALUES (?1, ?2, ?3)",
            params![text, duration_ms, model],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_history(&self, limit: usize) -> Result<Vec<Transcription>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, text, timestamp, duration_ms, model
             FROM transcriptions
             ORDER BY timestamp DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map([limit], |row| {
            Ok(Transcription {
                id: row.get(0)?,
                text: row.get(1)?,
                timestamp: row.get(2)?,
                duration_ms: row.get(3)?,
                model: row.get(4)?,
            })
        })?;

        rows.collect()
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Transcription>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT id, text, timestamp, duration_ms, model
             FROM transcriptions
             WHERE text LIKE ?1
             ORDER BY timestamp DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![pattern, limit], |row| {
            Ok(Transcription {
                id: row.get(0)?,
                text: row.get(1)?,
                timestamp: row.get(2)?,
                duration_ms: row.get(3)?,
                model: row.get(4)?,
            })
        })?;

        rows.collect()
    }
}

#[cfg(test)]
impl Database {
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS transcriptions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                duration_ms INTEGER,
                model TEXT DEFAULT 'base'
            )",
            [],
        )?;
        Ok(Self { conn })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_transcription() {
        let db = Database::new_in_memory().unwrap();
        let id = db.add_transcription("hello world", None, "base").unwrap();
        assert!(id > 0);
        let history = db.get_history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].text, "hello world");
        assert_eq!(history[0].model, "base");
    }

    #[test]
    fn test_get_history_ordering() {
        let db = Database::new_in_memory().unwrap();
        db.add_transcription("first", None, "base").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        db.add_transcription("second", None, "base").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        db.add_transcription("third", None, "base").unwrap();
        let history = db.get_history(10).unwrap();
        // Most recent should be first (DESC order)
        assert_eq!(history[0].text, "third");
        assert_eq!(history[2].text, "first");
    }

    #[test]
    fn test_get_history_limit() {
        let db = Database::new_in_memory().unwrap();
        for i in 0..5 {
            db.add_transcription(&format!("item {}", i), None, "base")
                .unwrap();
        }
        let history = db.get_history(3).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_search_matching() {
        let db = Database::new_in_memory().unwrap();
        db.add_transcription("the quick brown fox", None, "base")
            .unwrap();
        db.add_transcription("lazy dog", None, "base").unwrap();
        db.add_transcription("quick silver", None, "base").unwrap();
        let results = db.search("quick", 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_no_results() {
        let db = Database::new_in_memory().unwrap();
        db.add_transcription("hello world", None, "base").unwrap();
        let results = db.search("nonexistent", 10).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_add_with_duration_and_model() {
        let db = Database::new_in_memory().unwrap();
        db.add_transcription("test text", Some(5000), "medium")
            .unwrap();
        let history = db.get_history(1).unwrap();
        assert_eq!(history[0].duration_ms, Some(5000));
        assert_eq!(history[0].model, "medium");
    }
}
