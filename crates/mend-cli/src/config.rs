use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub jev_endpoint: String,
    pub jev_api_key: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_denylist: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_allowlist: Option<Vec<String>>,
}

impl AppConfig {
    pub fn parsed_denylist(&self) -> Vec<mend_core::ActionStrategy> {
        self.action_denylist
            .iter()
            .filter_map(|s| s.parse().ok())
            .filter(|a| *a != mend_core::ActionStrategy::Abort)
            .collect()
    }

    pub fn parsed_allowlist(&self) -> Option<Vec<mend_core::ActionStrategy>> {
        self.action_allowlist
            .as_ref()
            .map(|list| list.iter().filter_map(|s| s.parse().ok()).collect())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            jev_endpoint: "https://api.typesafe.ai/v1/jev/evaluate".to_string(),
            jev_api_key: "".to_string(),
            action_denylist: Vec::new(),
            action_allowlist: None,
        }
    }
}

pub fn get_config_path() -> PathBuf {
    if let Ok(home) = env::var("HOME") {
        let mend_path = PathBuf::from(&home).join(".mend.json");
        if mend_path.exists() {
            return mend_path;
        }
        let legacy_path = PathBuf::from(&home).join(".healc.json");
        if legacy_path.exists() {
            return legacy_path;
        }
        mend_path
    } else {
        PathBuf::from(".mend.json")
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

    #[test]
    fn test_app_config_policies() {
        let cfg = AppConfig {
            action_denylist: vec!["PREPEND_SUDO".to_string(), "INVALID_ACTION".to_string()],
            action_allowlist: Some(vec!["GIT_SET_UPSTREAM".to_string()]),
            ..Default::default()
        };

        let denylist = cfg.parsed_denylist();
        assert_eq!(denylist, vec![mend_core::ActionStrategy::PrependSudo]);

        let allowlist = cfg.parsed_allowlist().unwrap();
        assert_eq!(allowlist, vec![mend_core::ActionStrategy::GitSetUpstream]);
    }
}
