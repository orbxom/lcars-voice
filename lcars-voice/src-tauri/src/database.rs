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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingRecord {
    pub id: i64,
    pub filename: String,
    pub timestamp: String,
    pub duration_ms: i64,
    pub size_bytes: i64,
    pub transcript: Option<String>,
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

        conn.execute(
            "CREATE TABLE IF NOT EXISTS meetings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                filename TEXT NOT NULL,
                audio_data BLOB NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                duration_ms INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL
            )",
            [],
        )?;

        // Migration: add transcript column if it doesn't exist
        conn.execute_batch("ALTER TABLE meetings ADD COLUMN transcript TEXT;")
            .ok();

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

    pub fn add_meeting(
        &self,
        filename: &str,
        audio_data: &[u8],
        duration_ms: i64,
    ) -> Result<i64> {
        let size_bytes = audio_data.len() as i64;
        self.conn.execute(
            "INSERT INTO meetings (filename, audio_data, duration_ms, size_bytes)
             VALUES (?1, ?2, ?3, ?4)",
            params![filename, audio_data, duration_ms, size_bytes],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_meetings(&self, limit: usize) -> Result<Vec<MeetingRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, filename, timestamp, duration_ms, size_bytes, transcript
             FROM meetings
             ORDER BY timestamp DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map([limit], |row| {
            Ok(MeetingRecord {
                id: row.get(0)?,
                filename: row.get(1)?,
                timestamp: row.get(2)?,
                duration_ms: row.get(3)?,
                size_bytes: row.get(4)?,
                transcript: row.get(5)?,
            })
        })?;

        rows.collect()
    }

    pub fn get_meeting_audio(&self, id: i64) -> Result<Vec<u8>> {
        self.conn.query_row(
            "SELECT audio_data FROM meetings WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
    }

    pub fn save_meeting_transcript(&self, id: i64, transcript: &str) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE meetings SET transcript = ?1 WHERE id = ?2",
            params![transcript, id],
        )?;
        if rows == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(())
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
        conn.execute(
            "CREATE TABLE IF NOT EXISTS meetings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                filename TEXT NOT NULL,
                audio_data BLOB NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                duration_ms INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL,
                transcript TEXT
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

    #[test]
    fn test_add_and_get_meeting() {
        let db = Database::new_in_memory().unwrap();
        let audio = vec![0u8; 100];
        let id = db
            .add_meeting("meeting-2025-02-27-143045.wav", &audio, 5000)
            .unwrap();
        assert!(id > 0);
        let meetings = db.get_meetings(10).unwrap();
        assert_eq!(meetings.len(), 1);
        assert_eq!(meetings[0].filename, "meeting-2025-02-27-143045.wav");
        assert_eq!(meetings[0].duration_ms, 5000);
        assert_eq!(meetings[0].size_bytes, 100);
    }

    #[test]
    fn test_get_meetings_ordering() {
        let db = Database::new_in_memory().unwrap();
        db.add_meeting("first.wav", &[0u8; 10], 1000).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        db.add_meeting("second.wav", &[0u8; 10], 2000).unwrap();
        let meetings = db.get_meetings(10).unwrap();
        assert_eq!(meetings[0].filename, "second.wav");
        assert_eq!(meetings[1].filename, "first.wav");
    }

    #[test]
    fn test_get_meetings_limit() {
        let db = Database::new_in_memory().unwrap();
        for i in 0..5 {
            db.add_meeting(&format!("meeting-{}.wav", i), &[0u8; 10], 1000)
                .unwrap();
        }
        let meetings = db.get_meetings(3).unwrap();
        assert_eq!(meetings.len(), 3);
    }

    #[test]
    fn test_meeting_does_not_affect_transcriptions() {
        let db = Database::new_in_memory().unwrap();
        db.add_meeting("test.wav", &[0u8; 10], 1000).unwrap();
        db.add_transcription("hello", None, "base").unwrap();
        let meetings = db.get_meetings(10).unwrap();
        let transcriptions = db.get_history(10).unwrap();
        assert_eq!(meetings.len(), 1);
        assert_eq!(transcriptions.len(), 1);
    }

    #[test]
    fn test_meeting_record_has_transcript_field() {
        let db = Database::new_in_memory().unwrap();
        let audio = vec![0u8; 50];
        db.add_meeting("test.wav", &audio, 3000).unwrap();
        let meetings = db.get_meetings(10).unwrap();
        assert_eq!(meetings.len(), 1);
        assert_eq!(meetings[0].transcript, None);
    }

    #[test]
    fn test_save_and_get_meeting_transcript() {
        let db = Database::new_in_memory().unwrap();
        let audio = vec![0u8; 50];
        let id = db.add_meeting("test.wav", &audio, 3000).unwrap();
        db.save_meeting_transcript(id, "This is the transcript text").unwrap();
        let meetings = db.get_meetings(10).unwrap();
        assert_eq!(meetings.len(), 1);
        assert_eq!(meetings[0].transcript, Some("This is the transcript text".to_string()));
    }

    #[test]
    fn test_get_meeting_audio() {
        let db = Database::new_in_memory().unwrap();
        let audio = vec![1u8, 2, 3, 4, 5];
        let id = db.add_meeting("test.wav", &audio, 1000).unwrap();
        let retrieved = db.get_meeting_audio(id).unwrap();
        assert_eq!(retrieved, vec![1u8, 2, 3, 4, 5]);
    }

    #[test]
    fn test_get_meeting_audio_not_found() {
        let db = Database::new_in_memory().unwrap();
        let result = db.get_meeting_audio(999);
        assert!(result.is_err());
    }

    #[test]
    fn test_save_transcript_not_found() {
        let db = Database::new_in_memory().unwrap();
        let result = db.save_meeting_transcript(999, "text");
        assert!(result.is_err());
    }

    #[test]
    fn test_transcript_persists_across_queries() {
        let db = Database::new_in_memory().unwrap();
        let audio = vec![0u8; 50];
        let id = db.add_meeting("test.wav", &audio, 3000).unwrap();
        db.save_meeting_transcript(id, "persistent transcript").unwrap();

        // Query multiple times to verify persistence
        let meetings1 = db.get_meetings(10).unwrap();
        let meetings2 = db.get_meetings(10).unwrap();
        let meetings3 = db.get_meetings(10).unwrap();

        assert_eq!(meetings1[0].transcript, Some("persistent transcript".to_string()));
        assert_eq!(meetings2[0].transcript, Some("persistent transcript".to_string()));
        assert_eq!(meetings3[0].transcript, Some("persistent transcript".to_string()));
    }
}
