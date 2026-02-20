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
use tauri::{Manager, Emitter};
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
    // Wait for Ollama to be ready (it starts in background on app launch)
    let url = format!("{}/api/tags", ollama::get_ollama_url());
    let client = reqwest::Client::new();
    let mut ready = false;

    for i in 0..30 {
        match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => {
                if i > 0 {
                    eprintln!("[openworld] list_models: Ollama became ready after {:.1}s", (i as f64) * 0.5);
                }
                ready = true;
                break;
            }
            _ => {
                if i == 0 {
                    eprintln!("[openworld] list_models: Ollama not ready yet, waiting...");
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
        }
    }

    if !ready {
        return Err("AI engine is still starting. Please try again in a moment.".to_string());
    }

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
    // Read memory context and existing memories for deduplication
    let (memory_context, existing_memories) = {
        let app_state = state.lock().map_err(|e| e.to_string())?;
        let ctx = app_state.storage.get_memory_context().unwrap_or_default();
        let mems: Vec<String> = app_state
            .storage
            .list_memories()
            .unwrap_or_default()
            .into_iter()
            .map(|(_, content, _)| content)
            .collect();
        (ctx, mems)
    };

    // Clone app handle before it's moved into send_chat_message
    let app_for_extraction = app.clone();

    // Send to Ollama and stream response (memory context is passed for system prompt injection)
    let full_response =
        chat::send_chat_message(app, conversation_id.clone(), messages.clone(), model.clone(), memory_context).await?;

    // Save the assistant response to storage
    {
        let app_state = state.lock().map_err(|e| e.to_string())?;
        app_state
            .storage
            .add_message(&conversation_id, "assistant", &full_response)?;
    }

    // Background fact extraction — don't block the response
    let messages_for_extraction = messages;
    let model_for_extraction = model;
    let response_for_extraction = full_response.clone();

    tauri::async_runtime::spawn(async move {
        // Build full message list including the assistant's reply
        let mut all_msgs = messages_for_extraction;
        all_msgs.push(ChatMessage {
            role: "assistant".to_string(),
            content: response_for_extraction,
        });

        // Generate dynamic title if this is the first exchange
        if all_msgs.len() <= 2 {
            eprintln!("[openworld] Generating dynamic title...");
            if let Ok(title) = chat::generate_conversation_title(&all_msgs, &model_for_extraction).await {
                eprintln!("[openworld] New title: {}", title);
                let managed_state = app_for_extraction.state::<Mutex<AppState>>();
                if let Ok(app_state) = managed_state.lock() {
                    let _ = app_state.storage.update_conversation_title(&conversation_id, &title);
                }
                // Tell the frontend to refresh the conversation list
                let _ = app_for_extraction.emit("conversation-title-updated", ());
            }
        }

        eprintln!("[openworld] Starting background fact extraction...");
        match chat::extract_facts_from_conversation(&all_msgs, &model_for_extraction, &existing_memories).await {
            Ok(facts) if facts.is_empty() => {
                eprintln!("[openworld] No new facts discovered.");
            }
            Ok(facts) => {
                eprintln!("[openworld] Discovered {} new fact(s):", facts.len());
                {
                    let managed_state = app_for_extraction.state::<Mutex<AppState>>();
                    let app_state = managed_state.lock().unwrap();
                    for fact in &facts {
                        eprintln!("[openworld]   + {}", fact);
                        if let Err(e) = app_state.storage.add_memory(fact) {
                            eprintln!("[openworld]   Failed to save fact: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[openworld] Fact extraction failed (non-fatal): {}", e);
            }
        }
    });

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

use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

// ── System Info ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct SystemMetrics {
    total_ram: u64,
    used_ram: u64,
    cpu_usage: f32, // Percentage 0-100
    db_size_bytes: u64,
}

#[tauri::command]
fn get_system_memory() -> Result<u64, String> {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    sys.refresh_memory();
    Ok(sys.total_memory())
}

#[tauri::command]
fn get_system_metrics() -> Result<SystemMetrics, String> {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_memory(MemoryRefreshKind::nothing().with_ram())
            .with_cpu(CpuRefreshKind::nothing().with_cpu_usage()),
    );
    
    // Refresh twice for CPU usage delta to be accurate
    sys.refresh_cpu_usage();
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_usage = sys.global_cpu_usage();
    
    // Get database size at ~/.openworld/data.db
    let db_size_bytes = if let Some(mut path) = dirs::home_dir() {
        path.push(".openworld");
        path.push("data.db");
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    Ok(SystemMetrics {
        total_ram: sys.total_memory(),
        used_ram: sys.used_memory(),
        cpu_usage,
        db_size_bytes,
    })
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
        .setup(|app| {
            // Auto-start Ollama on every app launch
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                eprintln!("[openworld] App startup: auto-starting Ollama...");
                match ollama::ensure_ollama_ready(handle).await {
                    Ok(()) => eprintln!("[openworld] App startup: Ollama is ready!"),
                    Err(e) => eprintln!("[openworld] App startup: Ollama failed to start: {}", e),
                }
            });
            Ok(())
        })
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
            get_system_metrics,
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
