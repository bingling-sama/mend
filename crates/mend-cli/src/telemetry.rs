use mend_core::ActionStrategy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TelemetryCache {
    /// Mapping of normalized command signature -> (action_name -> failure_count)
    pub failures: HashMap<String, HashMap<String, u32>>,
}

impl TelemetryCache {
    pub fn default_path() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".mend_telemetry.json")
        } else {
            PathBuf::from(".mend_telemetry.json")
        }
    }

    pub fn load() -> Self {
        Self::load_from_path(&Self::default_path())
    }

    pub fn load_from_path(path: &Path) -> Self {
        if let Ok(content) = fs::read_to_string(path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        self.save_to_path(&Self::default_path());
    }

    pub fn save_to_path(&self, path: &Path) {
        if let Ok(serialized) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, serialized);
        }
    }

    pub fn record_failure(&mut self, command: &str, action: &ActionStrategy) {
        let key = Self::normalize_command(command);
        let entry = self.failures.entry(key).or_default();
        let count = entry.entry(action.as_str().to_string()).or_insert(0);
        *count += 1;
        self.save();
    }

    pub fn record_success(&mut self, command: &str, _action: &ActionStrategy) {
        let key = Self::normalize_command(command);
        if self.failures.remove(&key).is_some() {
            self.save();
        }
    }

    pub fn is_penalized(&self, command: &str, action: &ActionStrategy) -> bool {
        let key = Self::normalize_command(command);
        if let Some(entry) = self.failures.get(&key) {
            if let Some(&count) = entry.get(action.as_str()) {
                return count >= 1;
            }
        }
        false
    }

    pub fn get_penalized_actions(&self, command: &str) -> Vec<ActionStrategy> {
        let key = Self::normalize_command(command);
        let mut penalized = Vec::new();
        if let Some(entry) = self.failures.get(&key) {
            for (action_str, &count) in entry {
                if count >= 1 {
                    let action: ActionStrategy =
                        action_str.parse().unwrap_or(ActionStrategy::Abort);
                    if action != ActionStrategy::Abort && !penalized.contains(&action) {
                        penalized.push(action);
                    }
                }
            }
        }
        penalized
    }

    pub fn normalize_command(cmd: &str) -> String {
        let tokens: Vec<&str> = cmd.split_whitespace().collect();
        if tokens.len() >= 2 {
            format!("{} {}", tokens[0], tokens[1])
        } else {
            cmd.trim().to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_telemetry_failure_recording_and_penalization() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("telemetry.json");

        let mut cache = TelemetryCache::default();
        assert!(!cache.is_penalized("git push origin main", &ActionStrategy::GitSetUpstream));

        cache.record_failure("git push origin main", &ActionStrategy::GitSetUpstream);
        cache.save_to_path(&path);

        let loaded = TelemetryCache::load_from_path(&path);
        assert!(loaded.is_penalized("git push origin other", &ActionStrategy::GitSetUpstream));
        let penalized = loaded.get_penalized_actions("git push");
        assert_eq!(penalized, vec![ActionStrategy::GitSetUpstream]);

        // Success clears the penalty
        let mut loaded = loaded;
        loaded.record_success("git push origin other", &ActionStrategy::GitSetUpstream);
        loaded.save_to_path(&path);

        let final_cache = TelemetryCache::load_from_path(&path);
        assert!(!final_cache.is_penalized("git push", &ActionStrategy::GitSetUpstream));
        assert!(final_cache.get_penalized_actions("git push").is_empty());
    }
}
