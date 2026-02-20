mod chat;
mod config;
mod crypto;
mod ollama;
mod storage;

use chat::ChatMessage;
use config::AppConfig;
use ollama::ModelInfo;
use serde::{Deserialize, Serialize};
use storage::{Conversation, Message, StorageEngine};
use std::sync::Mutex;
use tauri::State;

pub struct AppState {
    storage: StorageEngine,
}

// ── Config Commands ──────────────────────────────────────────────────────

#[tauri::command]
fn get_config() -> Result<AppConfig, String> {
    Ok(config::load_config())
}

#[tauri::command]
fn save_config_cmd(cfg: AppConfig) -> Result<(), String> {
    config::save_config(&cfg)
}

// ── Ollama Commands ──────────────────────────────────────────────────────

#[tauri::command]
async fn check_ollama() -> Result<bool, String> {
    Ok(ollama::check_ollama_running().await)
}

#[tauri::command]
async fn ensure_ollama(app: tauri::AppHandle) -> Result<(), String> {
    ollama::ensure_ollama_ready(app).await
}

#[tauri::command]
async fn list_models() -> Result<Vec<ModelInfo>, String> {
    ollama::list_installed_models().await
}

#[tauri::command]
async fn pull_model(app: tauri::AppHandle, model_name: String) -> Result<(), String> {
    ollama::pull_model(app, model_name).await
}

#[tauri::command]
async fn delete_model(model_name: String) -> Result<(), String> {
    ollama::delete_model(&model_name).await
}

// ── Chat Commands ────────────────────────────────────────────────────────

#[tauri::command]
async fn send_message(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
    conversation_id: String,
    messages: Vec<ChatMessage>,
    model: String,
) -> Result<String, String> {
    // Read memory context to inject into the conversation
    let memory_context = {
        let app_state = state.lock().map_err(|e| e.to_string())?;
        app_state.storage.get_memory_context().unwrap_or_default()
    };

    // Send to Ollama and stream response (memory context is passed for system prompt injection)
    let full_response =
        chat::send_chat_message(app, conversation_id.clone(), messages, model, memory_context).await?;

    // Save the assistant response to storage
    let app_state = state.lock().map_err(|e| e.to_string())?;
    app_state
        .storage
        .add_message(&conversation_id, "assistant", &full_response)?;

    Ok(full_response)
}

// ── Storage Commands ─────────────────────────────────────────────────────

#[tauri::command]
fn create_conversation(
    state: State<'_, Mutex<AppState>>,
    title: String,
    model: String,
) -> Result<Conversation, String> {
    let app_state = state.lock().map_err(|e| e.to_string())?;
    app_state.storage.create_conversation(&title, &model)
}

#[tauri::command]
fn list_conversations(state: State<'_, Mutex<AppState>>) -> Result<Vec<Conversation>, String> {
    let app_state = state.lock().map_err(|e| e.to_string())?;
    app_state.storage.list_conversations()
}

#[tauri::command]
fn delete_conversation(state: State<'_, Mutex<AppState>>, id: String) -> Result<(), String> {
    let app_state = state.lock().map_err(|e| e.to_string())?;
    app_state.storage.delete_conversation(&id)
}

#[tauri::command]
fn update_conversation_title(
    state: State<'_, Mutex<AppState>>,
    id: String,
    title: String,
) -> Result<(), String> {
    let app_state = state.lock().map_err(|e| e.to_string())?;
    app_state.storage.update_conversation_title(&id, &title)
}

#[tauri::command]
fn add_message(
    state: State<'_, Mutex<AppState>>,
    conversation_id: String,
    role: String,
    content: String,
) -> Result<Message, String> {
    let app_state = state.lock().map_err(|e| e.to_string())?;
    app_state.storage.add_message(&conversation_id, &role, &content)
}

#[tauri::command]
fn get_messages(
    state: State<'_, Mutex<AppState>>,
    conversation_id: String,
) -> Result<Vec<Message>, String> {
    let app_state = state.lock().map_err(|e| e.to_string())?;
    app_state.storage.get_messages(&conversation_id)
}

// ── System Info ──────────────────────────────────────────────────────────

#[tauri::command]
fn get_system_memory() -> Result<u64, String> {
    Ok(16 * 1024 * 1024 * 1024) // default 16GB
}

// ── Memory Commands ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct MemoryItem {
    id: String,
    content: String,
    created_at: String,
}

#[tauri::command]
fn add_memory_cmd(
    state: State<'_, Mutex<AppState>>,
    content: String,
) -> Result<String, String> {
    let app_state = state.lock().map_err(|e| e.to_string())?;
    app_state.storage.add_memory(&content)
}

#[tauri::command]
fn list_memories_cmd(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<MemoryItem>, String> {
    let app_state = state.lock().map_err(|e| e.to_string())?;
    let raw = app_state.storage.list_memories()?;
    Ok(raw.into_iter().map(|(id, content, created_at)| MemoryItem { id, content, created_at }).collect())
}

#[tauri::command]
fn delete_memory_cmd(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), String> {
    let app_state = state.lock().map_err(|e| e.to_string())?;
    app_state.storage.delete_memory(&id)
}

#[tauri::command]
fn get_memory_context_cmd(
    state: State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    let app_state = state.lock().map_err(|e| e.to_string())?;
    app_state.storage.get_memory_context()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let storage = StorageEngine::new().expect("Failed to initialize storage");
    let app_state = Mutex::new(AppState { storage });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config_cmd,
            check_ollama,
            ensure_ollama,
            list_models,
            pull_model,
            delete_model,
            send_message,
            create_conversation,
            list_conversations,
            delete_conversation,
            update_conversation_title,
            add_message,
            get_messages,
            get_system_memory,
            add_memory_cmd,
            list_memories_cmd,
            delete_memory_cmd,
            get_memory_context_cmd,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                ollama::stop_ollama();
            }
        });
}
