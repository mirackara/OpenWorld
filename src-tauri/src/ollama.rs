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
    eprintln!("[openworld] Searching for Ollama binary...");

    // 1. Check our bundled copy
    let bundled = get_ollama_bin_path();
    eprintln!("[openworld]   Checking bundled path: {}", bundled.display());
    if bundled.exists() {
        eprintln!("[openworld]   ✓ Found bundled binary");
        return Some(bundled.to_string_lossy().to_string());
    }

    // 2. Check common install locations (macOS app installs here)
    let common_paths = vec![
        "/usr/local/bin/ollama",
        "/opt/homebrew/bin/ollama",
        "/usr/bin/ollama",
    ];
    for path in &common_paths {
        eprintln!("[openworld]   Checking: {}", path);
        if std::path::Path::new(path).exists() {
            eprintln!("[openworld]   ✓ Found at {}", path);
            return Some(path.to_string());
        }
    }

    // 3. Check PATH via `which`
    eprintln!("[openworld]   Checking PATH via `which ollama`...");
    match Command::new("which").arg("ollama").output() {
        Ok(output) => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            eprintln!("[openworld]   `which` returned: '{}' (status: {})", path, output.status);
            if output.status.success() && !path.is_empty() && std::path::Path::new(&path).exists() {
                eprintln!("[openworld]   ✓ Found via PATH");
                return Some(path);
            }
        }
        Err(e) => {
            eprintln!("[openworld]   `which` failed: {}", e);
        }
    }

    eprintln!("[openworld]   ✗ Ollama binary not found anywhere");
    None
}

/// Get the GitHub release download URL and whether it needs extraction
fn get_download_info() -> Result<(String, bool), String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    // Use latest release redirect from GitHub
    match (os, arch) {
        ("macos", "aarch64") | ("macos", "x86_64") => {
            // macOS: .tgz archive containing the ollama binary
            Ok(("https://github.com/ollama/ollama/releases/latest/download/ollama-darwin.tgz".to_string(), true))
        }
        ("linux", "x86_64") => {
            Ok(("https://github.com/ollama/ollama/releases/latest/download/ollama-linux-amd64.tar.zst".to_string(), true))
        }
        ("linux", "aarch64") => {
            Ok(("https://github.com/ollama/ollama/releases/latest/download/ollama-linux-arm64.tar.zst".to_string(), true))
        }
        _ => Err(format!("Unsupported platform: {}-{}", os, arch)),
    }
}

/// Download the Ollama binary from GitHub releases
async fn download_ollama(app: &AppHandle) -> Result<String, String> {
    emit_status(app, "downloading", "Downloading AI engine...", Some(0.0));

    let (url, needs_extract) = get_download_info()?;
    eprintln!("[openworld] Downloading Ollama from: {}", url);
    eprintln!("[openworld] Needs extraction: {}", needs_extract);

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| { eprintln!("[openworld] HTTP client error: {}", e); format!("HTTP client error: {}", e) })?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| { eprintln!("[openworld] Download request failed: {}", e); format!("Download request failed: {}", e) })?;

    eprintln!("[openworld] Download response: HTTP {} (final URL after redirects)", resp.status());
    if !resp.status().is_success() {
        let msg = format!("Download failed with HTTP {}", resp.status());
        eprintln!("[openworld] {}", msg);
        return Err(msg);
    }

    let total_size = resp.content_length().unwrap_or(0);
    eprintln!("[openworld] Download size: {} bytes ({:.1} MB)", total_size, total_size as f64 / 1_048_576.0);

    // Save to a temp file (archive or binary)
    let bin_dir = get_ollama_bin_dir();
    let download_path = if needs_extract {
        bin_dir.join("ollama-download.tgz")
    } else {
        get_ollama_bin_path()
    };

    let mut file = std::fs::File::create(&download_path)
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

    drop(file);
    eprintln!("[openworld] Downloaded {} bytes to {}", downloaded, download_path.display());

    // Integrity check: file must be > 1MB (a real binary/archive is ~70MB)
    let file_size = std::fs::metadata(&download_path)
        .map(|m| m.len())
        .unwrap_or(0);
    if file_size < 1_000_000 {
        // Probably an error page, not a real binary
        let content = std::fs::read_to_string(&download_path).unwrap_or_default();
        let preview = content.chars().take(200).collect::<String>();
        eprintln!("[openworld] ✗ Downloaded file too small ({} bytes). Content preview: {}", file_size, preview);
        let _ = std::fs::remove_file(&download_path);
        return Err(format!("Download corrupted: got {} bytes instead of expected ~70MB", file_size));
    }

    // Extract if needed (macOS .tgz)
    let bin_path = get_ollama_bin_path();
    if needs_extract {
        eprintln!("[openworld] Extracting archive...");
        emit_status(app, "downloading", "Extracting AI engine...", Some(0.95));

        let output = Command::new("tar")
            .args(["xzf", &download_path.to_string_lossy(), "-C", &bin_dir.to_string_lossy()])
            .output()
            .map_err(|e| format!("Failed to extract: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("[openworld] ✗ tar extraction failed: {}", stderr);
            return Err(format!("Extraction failed: {}", stderr));
        }

        // Clean up the archive
        let _ = std::fs::remove_file(&download_path);

        // The tgz contains 'ollama' binary at the top level (or in a bin/ subfolder)
        // Check both possibilities
        if !bin_path.exists() {
            // Maybe it extracted to a subfolder like bin/ollama
            let alt_path = bin_dir.join("bin").join("ollama");
            if alt_path.exists() {
                std::fs::rename(&alt_path, &bin_path)
                    .map_err(|e| format!("Failed to move binary: {}", e))?;
                let _ = std::fs::remove_dir_all(bin_dir.join("bin"));
            }
        }

        if !bin_path.exists() {
            // List what was actually extracted
            if let Ok(entries) = std::fs::read_dir(&bin_dir) {
                eprintln!("[openworld] Files in bin dir after extraction:");
                for entry in entries.flatten() {
                    eprintln!("[openworld]   {}", entry.path().display());
                }
            }
            return Err("Extraction succeeded but Ollama binary not found in archive".to_string());
        }

        eprintln!("[openworld] ✓ Extracted binary to: {}", bin_path.display());
    }

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

    // Validate the binary actually runs
    eprintln!("[openworld] Validating binary...");
    match Command::new(&bin_path).arg("--version").output() {
        Ok(output) => {
            let version = String::from_utf8_lossy(&output.stdout);
            eprintln!("[openworld] ✓ Binary valid: {}", version.trim());
        }
        Err(e) => {
            eprintln!("[openworld] ✗ Binary validation failed: {}", e);
            return Err(format!("Downloaded binary is not executable: {}", e));
        }
    }

    emit_status(app, "downloading", "Download complete!", Some(1.0));
    Ok(bin_path.to_string_lossy().to_string())
}

/// Start `ollama serve` as a background process
fn start_ollama_server(binary_path: &str) -> Result<(), String> {
    eprintln!("[openworld] Starting Ollama server: {} serve", binary_path);
    let mut proc_guard = OLLAMA_PROCESS.lock().map_err(|e| e.to_string())?;

    // Already running?
    if let Some(ref mut child) = *proc_guard {
        if let Ok(None) = child.try_wait() {
            eprintln!("[openworld] Ollama process already running (pid exists)");
            return Ok(());
        }
    }

    let child = Command::new(binary_path)
        .arg("serve")
        .stdout(Stdio::inherit())  // Show Ollama output in terminal
        .stderr(Stdio::inherit())  // Show Ollama errors in terminal
        .spawn()
        .map_err(|e| {
            eprintln!("[openworld] ✗ Failed to spawn Ollama: {}", e);
            format!("Failed to start Ollama: {}", e)
        })?;

    eprintln!("[openworld] ✓ Ollama process spawned (pid: {})", child.id());
    *proc_guard = Some(child);
    Ok(())
}

/// Poll Ollama's API until it responds
async fn wait_for_ready(max_seconds: u32) -> bool {
    let url = format!("{}/api/tags", get_ollama_url());
    eprintln!("[openworld] Waiting up to {}s for Ollama at {}...", max_seconds, url);
    let client = Client::new();
    let attempts = max_seconds * 2; // poll every 500ms

    for i in 0..attempts {
        match client.get(&url).send().await {
            Ok(resp) => {
                eprintln!("[openworld] ✓ Ollama responded (HTTP {}) after {}ms", resp.status(), i * 500);
                return true;
            }
            Err(e) => {
                if i % 4 == 0 { // Log every 2 seconds
                    eprintln!("[openworld]   ...still waiting ({:.1}s): {}", (i * 500) as f64 / 1000.0, e);
                }
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    eprintln!("[openworld] ✗ Ollama did not respond after {}s", max_seconds);
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
    eprintln!("[openworld] ═══════════════════════════════════════");
    eprintln!("[openworld] ensure_ollama_ready: starting");
    eprintln!("[openworld] ═══════════════════════════════════════");

    // Step 1: Maybe it's already running
    eprintln!("[openworld] Step 1: Check if Ollama is already running...");
    emit_status(&app, "checking", "Checking AI engine...", None);

    if wait_for_ready(3).await {
        eprintln!("[openworld] ✓ Ollama already running!");
        emit_status(&app, "ready", "AI engine ready!", None);
        return Ok(());
    }
    eprintln!("[openworld] Ollama not running, need to find/download and start it");

    // Step 2: Find binary
    eprintln!("[openworld] Step 2: Find Ollama binary...");
    let binary_path = match find_ollama_binary() {
        Some(path) => {
            eprintln!("[openworld] ✓ Found binary at: {}", path);
            emit_status(&app, "starting", "Found AI engine, starting...", None);
            path
        }
        None => {
            eprintln!("[openworld] Binary not found, downloading...");
            match download_ollama(&app).await {
                Ok(path) => {
                    eprintln!("[openworld] ✓ Downloaded to: {}", path);
                    path
                }
                Err(e) => {
                    eprintln!("[openworld] ✗ Download failed: {}", e);
                    emit_status(&app, "error", &format!("Download failed: {}", e), None);
                    return Err(e);
                }
            }
        }
    };

    // Step 3: Start the server
    eprintln!("[openworld] Step 3: Start Ollama server...");
    emit_status(&app, "starting", "Starting AI engine...", None);
    if let Err(e) = start_ollama_server(&binary_path) {
        eprintln!("[openworld] ✗ Start failed: {}", e);
        emit_status(&app, "error", &format!("Failed to start: {}", e), None);
        return Err(e);
    }

    // Step 4: Wait for it to become responsive
    eprintln!("[openworld] Step 4: Wait for Ollama to be responsive...");
    emit_status(&app, "starting", "Waiting for AI engine to be ready...", None);
    if wait_for_ready(30).await {
        eprintln!("[openworld] ✓ Ollama is ready!");
        emit_status(&app, "ready", "AI engine ready!", None);
        Ok(())
    } else {
        let msg = "AI engine timed out after 30s. Check terminal for details.".to_string();
        eprintln!("[openworld] ✗ {}", msg);
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
    eprintln!("[openworld] list_installed_models: GET {}", url);
    let client = Client::new();

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| { eprintln!("[openworld] list_models failed: {}", e); format!("Failed to connect to Ollama: {}", e) })?;

    eprintln!("[openworld] list_models response: HTTP {}", resp.status());

    let body = resp.text().await.map_err(|e| format!("Failed to read response: {}", e))?;
    eprintln!("[openworld] list_models body: {}", &body[..body.len().min(500)]);

    let tags: TagsResponse = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let models: Vec<ModelInfo> = tags
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

    eprintln!("[openworld] list_models found {} models:", models.len());
    for m in &models {
        eprintln!("[openworld]   - '{}' ({} bytes)", m.name, m.size);
    }

    Ok(models)
}

pub async fn pull_model(app: AppHandle, model_name: String) -> Result<(), String> {
    eprintln!("[openworld] pull_model: pulling '{}'", model_name);

    // Make sure Ollama is running before pulling
    if !check_ollama_running().await {
        eprintln!("[openworld] pull_model: Ollama not running!");
        return Err("AI engine is not running. Please restart the app.".to_string());
    }
    eprintln!("[openworld] pull_model: Ollama is running, starting pull...");

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
