use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

use crate::config::get_data_dir;
use crate::crypto::CryptoEngine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,       // "user" | "assistant" | "system"
    pub content: String,    // plaintext (decrypted before sending to frontend)
    pub timestamp: String,
}

pub struct StorageEngine {
    conn: Mutex<Connection>,
    crypto: CryptoEngine,
}

impl StorageEngine {
    pub fn new() -> Result<Self, String> {
        let data_dir = get_data_dir();
        let db_path = data_dir.join("data.db");

        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                model TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content_encrypted TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
            );
            PRAGMA foreign_keys = ON;",
        )
        .map_err(|e| format!("Failed to create tables: {}", e))?;

        let master_secret = crate::crypto::get_or_create_master_secret(&data_dir)?;
        let crypto = CryptoEngine::new(&master_secret)?;

        Ok(Self {
            conn: Mutex::new(conn),
            crypto,
        })
    }

    pub fn create_conversation(&self, title: &str, model: &str) -> Result<Conversation, String> {
        let id = Uuid::new_v4().to_string();
        let now: DateTime<Utc> = Utc::now();
        let now_str = now.to_rfc3339();

        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO conversations (id, title, created_at, updated_at, model) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, title, now_str, now_str, model],
        )
        .map_err(|e| format!("Failed to create conversation: {}", e))?;

        Ok(Conversation {
            id,
            title: title.to_string(),
            created_at: now_str.clone(),
            updated_at: now_str,
            model: model.to_string(),
        })
    }

    pub fn list_conversations(&self) -> Result<Vec<Conversation>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, title, created_at, updated_at, model FROM conversations ORDER BY updated_at DESC")
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let convos = stmt
            .query_map([], |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    model: row.get(4)?,
                })
            })
            .map_err(|e| format!("Failed to query conversations: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(convos)
    }

    pub fn delete_conversation(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM messages WHERE conversation_id = ?1", params![id])
            .map_err(|e| format!("Failed to delete messages: {}", e))?;
        conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete conversation: {}", e))?;
        Ok(())
    }

    pub fn update_conversation_title(&self, id: &str, title: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now: DateTime<Utc> = Utc::now();
        conn.execute(
            "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, now.to_rfc3339(), id],
        )
        .map_err(|e| format!("Failed to update conversation: {}", e))?;
        Ok(())
    }

    pub fn add_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
    ) -> Result<Message, String> {
        let id = Uuid::new_v4().to_string();
        let now: DateTime<Utc> = Utc::now();
        let now_str = now.to_rfc3339();
        let encrypted = self.crypto.encrypt(content)?;

        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content_encrypted, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, conversation_id, role, encrypted, now_str],
        )
        .map_err(|e| format!("Failed to add message: {}", e))?;

        // Update conversation's updated_at
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now_str, conversation_id],
        )
        .map_err(|e| format!("Failed to update conversation timestamp: {}", e))?;

        Ok(Message {
            id,
            conversation_id: conversation_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            timestamp: now_str,
        })
    }

    pub fn get_messages(&self, conversation_id: &str) -> Result<Vec<Message>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, conversation_id, role, content_encrypted, timestamp FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC")
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let messages: Vec<Message> = stmt
            .query_map(params![conversation_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| format!("Failed to query messages: {}", e))?
            .filter_map(|r| r.ok())
            .map(|(id, conv_id, role, encrypted, timestamp)| {
                let content = self.crypto.decrypt(&encrypted).unwrap_or_else(|_| "[Decryption failed]".to_string());
                Message {
                    id,
                    conversation_id: conv_id,
                    role,
                    content,
                    timestamp,
                }
            })
            .collect();

        Ok(messages)
    }
}
