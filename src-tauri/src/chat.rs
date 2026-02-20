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

/// Analyze the latest messages and extract new personal facts about the user.
/// Returns a list of concise fact strings. This uses a non-streaming LLM call.
pub async fn extract_facts_from_conversation(
    messages: &[ChatMessage],
    model: &str,
    existing_memories: &[String],
) -> Result<Vec<String>, String> {
    let config = load_config();
    let url = format!("{}/api/chat", config.ollama_host);
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    // Only look at the last few messages for efficiency
    let recent: Vec<&ChatMessage> = messages.iter().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();

    // Build the conversation excerpt
    let mut excerpt = String::new();
    for msg in &recent {
        let label = if msg.role == "user" { "User" } else { "Assistant" };
        excerpt.push_str(&format!("{}: {}\n", label, msg.content));
    }

    // Build existing memories context so we don't duplicate
    let existing_str = if existing_memories.is_empty() {
        "None yet.".to_string()
    } else {
        existing_memories.iter().map(|m| format!("- {}", m)).collect::<Vec<_>>().join("\n")
    };

    let system_prompt = format!(
        r#"You are a strict personal fact extractor. Analyze the conversation below and extract ONLY permanent, long-term personal facts about the user.

Rules:
- ONLY extract core identity facts: Name, occupation, location, family members, pets, allergies, or major long-term hobbies.
- Do NOT extract short-term interests, current goals, shopping preferences, budget, opinions, or conversational context. (e.g., "Interested in OLED TVs" or "Wants to buy a TV" are short-term context, NOT long-term facts).
- Each fact must be a single short sentence (under 100 characters).
- Do NOT repeat facts already known (listed below).
- If there are NO new permanent facts, respond with exactly: NONE.
- Output one fact per line, no bullets, no numbering.

Already known facts:
{}

Conversation:
{}"#,
        existing_str, excerpt
    );

    let ollama_messages = vec![
        serde_json::json!({"role": "system", "content": system_prompt}),
        serde_json::json!({"role": "user", "content": "Extract new personal facts from the conversation above."}),
    ];

    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "model": model,
            "messages": ollama_messages,
            "stream": false,
            "options": {
                "temperature": 0.1
            }
        }))
        .send()
        .await
        .map_err(|e| format!("Fact extraction request failed: {}", e))?;

    #[derive(Deserialize)]
    struct OllamaResponse {
        message: Option<OllamaChatMsg>,
    }

    let body: OllamaResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse fact extraction response: {}", e))?;

    let response_text = body
        .message
        .and_then(|m| m.content)
        .unwrap_or_default()
        .trim()
        .to_string();

    eprintln!("[openworld] Fact extraction raw response: {}", response_text);

    // Parse facts: one per line, skip empty/NONE
    if response_text.is_empty() || response_text.to_uppercase().contains("NONE") {
        return Ok(vec![]);
    }

    let facts: Vec<String> = response_text
        .lines()
        .map(|l| l.trim().trim_start_matches('-').trim_start_matches('•').trim().to_string())
        .filter(|l| !l.is_empty() && l.len() <= 150 && !l.to_uppercase().contains("NONE"))
        .collect();

    Ok(facts)
}
