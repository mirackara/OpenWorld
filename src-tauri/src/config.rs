use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub theme: String,
    pub default_model: String,
    pub setup_complete: bool,
    pub system_prompt: String,
    pub ollama_host: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            default_model: "llama3:8b".to_string(),
            setup_complete: false,
            system_prompt: String::new(),
            ollama_host: "http://localhost:11434".to_string(),
        }
    }
}

pub fn get_data_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Could not find home directory");
    let data_dir = home.join(".openworld");
    fs::create_dir_all(&data_dir).expect("Could not create data directory");
    data_dir
}

fn config_path() -> PathBuf {
    get_data_dir().join("config.json")
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        let config = AppConfig::default();
        save_config(&config).ok();
        config
    }
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let path = config_path();
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    fs::write(path, json).map_err(|e| format!("Failed to write config: {}", e))
}
