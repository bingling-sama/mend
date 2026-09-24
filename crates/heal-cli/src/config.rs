use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub jev_endpoint: String,
    pub jev_api_key: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            jev_endpoint: "https://api.typesafe.ai/v1/jev/evaluate".to_string(),
            jev_api_key: "".to_string(),
        }
    }
}

pub fn get_config_path() -> PathBuf {
    if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".healc.json")
    } else {
        PathBuf::from(".healc.json")
    }
}

pub fn load_or_init_config() -> (AppConfig, bool) {
    let path = get_config_path();

    if !path.exists() {
        let default_config = AppConfig::default();
        if let Ok(serialized) = serde_json::to_string_pretty(&default_config) {
            let _ = fs::write(&path, serialized);
        }
        return (default_config, true);
    }

    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
            return (config, false);
        }
    }

    (AppConfig::default(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_serialization() {
        let cfg = AppConfig::default();
        let s = serde_json::to_string(&cfg).unwrap();
        assert!(s.contains("jev_endpoint"));
        let de: AppConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(de.jev_endpoint, cfg.jev_endpoint);
    }
}
