use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

use crate::config::{get_data_dir, load_config};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: u64,
    pub modified_at: String,
    pub digest: String,
    pub details: Option<ModelDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDetails {
    pub format: Option<String>,
    pub family: Option<String>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TagsResponse {
    models: Option<Vec<OllamaModel>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaModel {
    name: Option<String>,
    size: Option<u64>,
    modified_at: Option<String>,
    digest: Option<String>,
    details: Option<OllamaModelDetails>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaModelDetails {
    format: Option<String>,
    family: Option<String>,
    parameter_size: Option<String>,
    quantization_level: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PullProgress {
    pub status: String,
    pub digest: Option<String>,
    pub total: Option<u64>,
    pub completed: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OllamaStatus {
    pub stage: String,   // "checking" | "downloading" | "starting" | "ready" | "error"
    pub message: String,
    pub progress: Option<f64>,
}

// ── Global Ollama process handle ─────────────────────────────────────────

lazy_static::lazy_static! {
    static ref OLLAMA_PROCESS: Mutex<Option<Child>> = Mutex::new(None);
}

fn get_ollama_url() -> String {
    let config = load_config();
    config.ollama_host
}

fn get_ollama_bin_dir() -> PathBuf {
    let data_dir = get_data_dir();
    let bin_dir = data_dir.join("bin");
    std::fs::create_dir_all(&bin_dir).ok();
    bin_dir
}

fn get_ollama_bin_path() -> PathBuf {
    let name = if cfg!(target_os = "windows") {
        "ollama.exe"
    } else {
        "ollama"
    };
    get_ollama_bin_dir().join(name)
}

/// Search for Ollama binary in common locations + PATH
fn find_ollama_binary() -> Option<String> {
    // 1. Check our bundled copy
    let bundled = get_ollama_bin_path();
    if bundled.exists() {
        return Some(bundled.to_string_lossy().to_string());
    }

    // 2. Check common install locations (macOS app installs here)
    let common_paths = vec![
        "/usr/local/bin/ollama",
        "/opt/homebrew/bin/ollama",
        "/usr/bin/ollama",
    ];
    for path in &common_paths {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    // 3. Check PATH via `which`
    if let Ok(output) = Command::new("which").arg("ollama").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() && std::path::Path::new(&path).exists() {
                return Some(path);
            }
        }
    }

    None
}

/// Get the download URL for the current platform
fn get_download_url() -> Result<String, String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    match (os, arch) {
        ("macos", "aarch64") | ("macos", "x86_64") => {
            Ok("https://ollama.com/download/ollama-darwin".to_string())
        }
        ("linux", "x86_64") => {
            Ok("https://ollama.com/download/ollama-linux-amd64".to_string())
        }
        ("linux", "aarch64") => {
            Ok("https://ollama.com/download/ollama-linux-arm64".to_string())
        }
        _ => Err(format!("Unsupported platform: {}-{}", os, arch)),
    }
}

/// Download the Ollama binary
async fn download_ollama(app: &AppHandle) -> Result<String, String> {
    emit_status(app, "downloading", "Downloading AI engine...", Some(0.0));

    let url = get_download_url()?;
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(600)) // 10 min timeout for large download
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Download request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed with HTTP {}", resp.status()));
    }

    let total_size = resp.content_length().unwrap_or(0);
    let bin_path = get_ollama_bin_path();

    let mut file = std::fs::File::create(&bin_path)
        .map_err(|e| format!("Failed to create file: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut stream = resp;

    while let Ok(chunk) = stream.chunk().await {
        match chunk {
            Some(bytes) => {
                file.write_all(&bytes)
                    .map_err(|e| format!("Write error: {}", e))?;
                downloaded += bytes.len() as u64;
                if total_size > 0 {
                    let pct = downloaded as f64 / total_size as f64;
                    emit_status(
                        app,
                        "downloading",
                        &format!("Downloading AI engine... {}%", (pct * 100.0) as u32),
                        Some(pct),
                    );
                }
            }
            None => break,
        }
    }

    drop(file); // Close file before chmod

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(&bin_path)
            .map_err(|e| format!("Metadata error: {}", e))?;
        let mut perms = meta.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin_path, perms)
            .map_err(|e| format!("Permission error: {}", e))?;
    }

    emit_status(app, "downloading", "Download complete!", Some(1.0));
    Ok(bin_path.to_string_lossy().to_string())
}

/// Start `ollama serve` as a background process
fn start_ollama_server(binary_path: &str) -> Result<(), String> {
    let mut proc_guard = OLLAMA_PROCESS.lock().map_err(|e| e.to_string())?;

    // Already running?
    if let Some(ref mut child) = *proc_guard {
        if let Ok(None) = child.try_wait() {
            return Ok(());
        }
    }

    let child = Command::new(binary_path)
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start Ollama: {}", e))?;

    *proc_guard = Some(child);
    Ok(())
}

/// Poll Ollama's API until it responds
async fn wait_for_ready(max_seconds: u32) -> bool {
    let url = format!("{}/api/tags", get_ollama_url());
    let client = Client::new();
    let attempts = max_seconds * 2; // poll every 500ms

    for _ in 0..attempts {
        if client.get(&url).send().await.is_ok() {
            return true;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    false
}

fn emit_status(app: &AppHandle, stage: &str, message: &str, progress: Option<f64>) {
    let _ = app.emit(
        "ollama-setup-status",
        OllamaStatus {
            stage: stage.to_string(),
            message: message.to_string(),
            progress,
        },
    );
}

// ── Public API ───────────────────────────────────────────────────────────

/// Ensure Ollama is installed, running, and ready.
/// Handles: check running → find binary → download if needed → start → health-check.
pub async fn ensure_ollama_ready(app: AppHandle) -> Result<(), String> {
    // Step 1: Maybe it's already running (user installed Ollama themselves)
    emit_status(&app, "checking", "Checking AI engine...", None);

    if wait_for_ready(3).await {
        emit_status(&app, "ready", "AI engine ready!", None);
        return Ok(());
    }

    // Step 2: Find binary (bundled or system-installed)
    let binary_path = match find_ollama_binary() {
        Some(path) => {
            emit_status(&app, "starting", "Found AI engine, starting...", None);
            path
        }
        None => {
            // Need to download it
            match download_ollama(&app).await {
                Ok(path) => path,
                Err(e) => {
                    emit_status(&app, "error", &format!("Download failed: {}", e), None);
                    return Err(e);
                }
            }
        }
    };

    // Step 3: Start the server
    emit_status(&app, "starting", "Starting AI engine...", None);
    if let Err(e) = start_ollama_server(&binary_path) {
        emit_status(&app, "error", &format!("Failed to start: {}", e), None);
        return Err(e);
    }

    // Step 4: Wait for it to become responsive (up to 30 seconds)
    emit_status(&app, "starting", "Waiting for AI engine to be ready...", None);
    if wait_for_ready(30).await {
        emit_status(&app, "ready", "AI engine ready!", None);
        Ok(())
    } else {
        let msg = "AI engine is taking too long to start. Please restart the app.".to_string();
        emit_status(&app, "error", &msg, None);
        Err(msg)
    }
}

pub async fn check_ollama_running() -> bool {
    let url = format!("{}/api/tags", get_ollama_url());
    let client = Client::new();
    client.get(&url).send().await.is_ok()
}

pub async fn list_installed_models() -> Result<Vec<ModelInfo>, String> {
    let url = format!("{}/api/tags", get_ollama_url());
    let client = Client::new();

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;

    let tags: TagsResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let models = tags
        .models
        .unwrap_or_default()
        .into_iter()
        .map(|m| ModelInfo {
            name: m.name.unwrap_or_default(),
            size: m.size.unwrap_or(0),
            modified_at: m.modified_at.unwrap_or_default(),
            digest: m.digest.unwrap_or_default(),
            details: m.details.map(|d| ModelDetails {
                format: d.format,
                family: d.family,
                parameter_size: d.parameter_size,
                quantization_level: d.quantization_level,
            }),
        })
        .collect();

    Ok(models)
}

pub async fn pull_model(app: AppHandle, model_name: String) -> Result<(), String> {
    // Make sure Ollama is running before pulling
    if !check_ollama_running().await {
        return Err("AI engine is not running. Please restart the app.".to_string());
    }

    let url = format!("{}/api/pull", get_ollama_url());
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(3600)) // 1 hour for large models
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "name": model_name,
            "stream": true
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to start model pull: {}", e))?;

    let mut stream = resp;
    let mut buffer = Vec::new();

    while let Ok(chunk) = stream.chunk().await {
        match chunk {
            Some(bytes) => {
                buffer.extend_from_slice(&bytes);
                while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = buffer.drain(..=pos).collect();
                    let line_str = String::from_utf8_lossy(&line);
                    if let Ok(progress) = serde_json::from_str::<PullProgress>(&line_str) {
                        let _ = app.emit("model-pull-progress", &progress);
                    }
                }
            }
            None => break,
        }
    }

    if !buffer.is_empty() {
        let line_str = String::from_utf8_lossy(&buffer);
        if let Ok(progress) = serde_json::from_str::<PullProgress>(&line_str) {
            let _ = app.emit("model-pull-progress", &progress);
        }
    }

    Ok(())
}

pub async fn delete_model(model_name: &str) -> Result<(), String> {
    let url = format!("{}/api/delete", get_ollama_url());
    let client = Client::new();

    client
        .delete(&url)
        .json(&serde_json::json!({
            "name": model_name
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to delete model: {}", e))?;

    Ok(())
}

/// Stop the managed Ollama process on app exit
pub fn stop_ollama() {
    if let Ok(mut proc_guard) = OLLAMA_PROCESS.lock() {
        if let Some(ref mut child) = *proc_guard {
            let _ = child.kill();
        }
        *proc_guard = None;
    }
}
