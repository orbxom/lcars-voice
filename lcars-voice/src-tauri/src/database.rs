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
