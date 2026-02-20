use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::config::load_config;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StreamToken {
    pub conversation_id: String,
    pub content: String,
    pub done: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaChatChunk {
    message: Option<OllamaChatMsg>,
    done: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatMsg {
    content: Option<String>,
}

pub async fn send_chat_message(
    app: AppHandle,
    conversation_id: String,
    messages: Vec<ChatMessage>,
    model: String,
    memory_context: String,
) -> Result<String, String> {
    let config = load_config();
    let url = format!("{}/api/chat", config.ollama_host);
    let client = Client::new();

    // Build system prompt: memory context + user's custom system prompt
    let mut system_parts = Vec::new();
    if !memory_context.is_empty() {
        system_parts.push(memory_context);
    }
    if !config.system_prompt.is_empty() {
        system_parts.push(config.system_prompt.clone());
    }

    // Build message history including system prompt
    let mut ollama_messages = Vec::new();
    if !system_parts.is_empty() {
        ollama_messages.push(serde_json::json!({
            "role": "system",
            "content": system_parts.join("\n\n")
        }));
    }
    for msg in &messages {
        ollama_messages.push(serde_json::json!({
            "role": msg.role,
            "content": msg.content
        }));
    }

    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "model": model,
            "messages": ollama_messages,
            "stream": true
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to send chat message: {}", e))?;

    let mut full_response = String::new();
    let mut buffer = Vec::new();
    let mut stream = resp;

    while let Ok(chunk) = stream.chunk().await {
        match chunk {
            Some(bytes) => {
                buffer.extend_from_slice(&bytes);

                while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = buffer.drain(..=pos).collect();
                    let line_str = String::from_utf8_lossy(&line);

                    if let Ok(chat_chunk) = serde_json::from_str::<OllamaChatChunk>(&line_str) {
                        let content = chat_chunk
                            .message
                            .and_then(|m| m.content)
                            .unwrap_or_default();
                        let done = chat_chunk.done.unwrap_or(false);

                        full_response.push_str(&content);

                        let token = StreamToken {
                            conversation_id: conversation_id.clone(),
                            content,
                            done,
                        };
                        let _ = app.emit("chat-stream-token", &token);
                    }
                }
            }
            None => break,
        }
    }

    // Handle remaining buffer
    if !buffer.is_empty() {
        let line_str = String::from_utf8_lossy(&buffer);
        if let Ok(chat_chunk) = serde_json::from_str::<OllamaChatChunk>(&line_str) {
            let content = chat_chunk
                .message
                .and_then(|m| m.content)
                .unwrap_or_default();
            full_response.push_str(&content);

            let token = StreamToken {
                conversation_id: conversation_id.clone(),
                content,
                done: true,
            };
            let _ = app.emit("chat-stream-token", &token);
        }
    }

    Ok(full_response)
}
