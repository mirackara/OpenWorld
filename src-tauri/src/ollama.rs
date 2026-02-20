use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Child, Command};
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
    pub progress: Option<f64>, // 0.0 - 1.0 for download
}

// ── Global Ollama process handle ─────────────────────────────────────────

lazy_static::lazy_static! {
    static ref OLLAMA_PROCESS: Mutex<Option<Child>> = Mutex::new(None);
}

fn get_ollama_url() -> String {
    let config = load_config();
    config.ollama_host
}

/// Get the path to the bundled Ollama binary
fn get_ollama_bin_dir() -> std::path::PathBuf {
    let data_dir = get_data_dir();
    let bin_dir = data_dir.join("bin");
    std::fs::create_dir_all(&bin_dir).ok();
    bin_dir
}

fn get_ollama_bin_path() -> std::path::PathBuf {
    get_ollama_bin_dir().join("ollama")
}

/// Find a working Ollama binary: check our bundled copy first, then system PATH
fn find_ollama_binary() -> Option<String> {
    let bundled = get_ollama_bin_path();
    if bundled.exists() {
        return Some(bundled.to_string_lossy().to_string());
    }
    // Check system PATH
    if let Ok(output) = Command::new("which").arg("ollama").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }
    None
}

/// Download the Ollama binary for the current platform
async fn download_ollama(app: &AppHandle) -> Result<String, String> {
    let _ = app.emit("ollama-setup-status", OllamaStatus {
        stage: "downloading".to_string(),
        message: "Downloading AI engine...".to_string(),
        progress: Some(0.0),
    });

    // Determine download URL for current platform
    let (url, _filename) = get_ollama_download_url()?;

    let client = Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to download Ollama: {}", e))?;

    let total_size = resp.content_length().unwrap_or(0);
    let bin_path = get_ollama_bin_path();

    let mut file = std::fs::File::create(&bin_path)
        .map_err(|e| format!("Failed to create binary file: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut stream = resp;

    while let Ok(chunk) = stream.chunk().await {
        match chunk {
            Some(bytes) => {
                file.write_all(&bytes)
                    .map_err(|e| format!("Failed to write binary: {}", e))?;
                downloaded += bytes.len() as u64;
                if total_size > 0 {
                    let progress = downloaded as f64 / total_size as f64;
                    let _ = app.emit("ollama-setup-status", OllamaStatus {
                        stage: "downloading".to_string(),
                        message: format!("Downloading AI engine... {}%", (progress * 100.0) as u32),
                        progress: Some(progress),
                    });
                }
            }
            None => break,
        }
    }

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&bin_path)
            .map_err(|e| format!("Failed to read permissions: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin_path, perms)
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    let _ = app.emit("ollama-setup-status", OllamaStatus {
        stage: "downloading".to_string(),
        message: "Download complete!".to_string(),
        progress: Some(1.0),
    });

    Ok(bin_path.to_string_lossy().to_string())
}

/// Get the download URL for the current platform
fn get_ollama_download_url() -> Result<(String, String), String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    match (os, arch) {
        ("macos", "aarch64") => Ok((
            "https://ollama.com/download/ollama-darwin".to_string(),
            "ollama".to_string(),
        )),
        ("macos", "x86_64") => Ok((
            "https://ollama.com/download/ollama-darwin".to_string(),
            "ollama".to_string(),
        )),
        ("linux", "x86_64") => Ok((
            "https://ollama.com/download/ollama-linux-amd64".to_string(),
            "ollama".to_string(),
        )),
        ("linux", "aarch64") => Ok((
            "https://ollama.com/download/ollama-linux-arm64".to_string(),
            "ollama".to_string(),
        )),
        ("windows", _) => Ok((
            "https://ollama.com/download/ollama-windows-amd64.exe".to_string(),
            "ollama.exe".to_string(),
        )),
        _ => Err(format!("Unsupported platform: {} {}", os, arch)),
    }
}

/// Start the Ollama server as a background process
fn start_ollama_server(binary_path: &str) -> Result<(), String> {
    let mut proc_guard = OLLAMA_PROCESS.lock().map_err(|e| e.to_string())?;

    // Don't start if already running
    if let Some(ref mut child) = *proc_guard {
        if let Ok(None) = child.try_wait() {
            return Ok(()); // Still running
        }
    }

    let child = Command::new(binary_path)
        .arg("serve")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start Ollama: {}", e))?;

    *proc_guard = Some(child);
    Ok(())
}

/// Wait for Ollama to be responsive
async fn wait_for_ollama_ready(max_attempts: u32) -> bool {
    let url = format!("{}/api/tags", get_ollama_url());
    let client = Client::new();

    for _ in 0..max_attempts {
        if client.get(&url).send().await.is_ok() {
            return true;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    false
}

// ── Public API ───────────────────────────────────────────────────────────

/// Ensure Ollama is installed, running, and ready.
/// This is the main entry point called from the frontend.
/// It handles: finding binary → downloading if missing → starting server → health check.
pub async fn ensure_ollama_ready(app: AppHandle) -> Result<(), String> {
    // Step 1: Check if Ollama is already running
    let _ = app.emit("ollama-setup-status", OllamaStatus {
        stage: "checking".to_string(),
        message: "Checking AI engine...".to_string(),
        progress: None,
    });

    if check_ollama_running().await {
        let _ = app.emit("ollama-setup-status", OllamaStatus {
            stage: "ready".to_string(),
            message: "AI engine ready!".to_string(),
            progress: None,
        });
        return Ok(());
    }

    // Step 2: Find or download the binary
    let binary_path = match find_ollama_binary() {
        Some(path) => path,
        None => {
            // Download it
            download_ollama(&app).await?
        }
    };

    // Step 3: Start the server
    let _ = app.emit("ollama-setup-status", OllamaStatus {
        stage: "starting".to_string(),
        message: "Starting AI engine...".to_string(),
        progress: None,
    });

    start_ollama_server(&binary_path)?;

    // Step 4: Wait for it to be ready
    let ready = wait_for_ollama_ready(30).await; // 15 seconds max

    if ready {
        let _ = app.emit("ollama-setup-status", OllamaStatus {
            stage: "ready".to_string(),
            message: "AI engine ready!".to_string(),
            progress: None,
        });
        Ok(())
    } else {
        let _ = app.emit("ollama-setup-status", OllamaStatus {
            stage: "error".to_string(),
            message: "AI engine failed to start. Please try restarting the app.".to_string(),
            progress: None,
        });
        Err("Ollama failed to start after 15 seconds".to_string())
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
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

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
    let url = format!("{}/api/pull", get_ollama_url());
    let client = Client::new();

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

/// Stop the managed Ollama process when the app exits
pub fn stop_ollama() {
    if let Ok(mut proc_guard) = OLLAMA_PROCESS.lock() {
        if let Some(ref mut child) = *proc_guard {
            let _ = child.kill();
        }
        *proc_guard = None;
    }
}
