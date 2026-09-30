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
            jev_endpoint: "https://api.typesafe.ai/v1/systemone".to_string(),
            jev_api_key: "".to_string(),
            action_denylist: Vec::new(),
            action_allowlist: None,
        }
    }
}

pub fn get_config_path() -> PathBuf {
    if let Ok(home) = env::var("HOME") {
        let jsonc_path = PathBuf::from(&home).join(".mend.jsonc");
        if jsonc_path.exists() {
            return jsonc_path;
        }
        let json_path = PathBuf::from(&home).join(".mend.json");
        if json_path.exists() {
            return json_path;
        }
        jsonc_path
    } else {
        PathBuf::from(".mend.jsonc")
    }
}

pub fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;

    while i < len {
        let ch = chars[i];

        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if ch == '"' {
            in_string = true;
            escaped = false;
            out.push(ch);
            i += 1;
            continue;
        }

        if ch == '/' && i + 1 < len && chars[i + 1] == '/' {
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(len);
            continue;
        }

        out.push(ch);
        i += 1;
    }

    let mut cleaned = String::with_capacity(out.len());
    let out_chars: Vec<char> = out.chars().collect();
    let mut j = 0;
    let mut in_str = false;
    let mut esc = false;

    while j < out_chars.len() {
        let c = out_chars[j];
        if in_str {
            cleaned.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            j += 1;
            continue;
        }

        if c == '"' {
            in_str = true;
            esc = false;
            cleaned.push(c);
            j += 1;
            continue;
        }

        if c == ',' {
            let mut k = j + 1;
            while k < out_chars.len() && out_chars[k].is_whitespace() {
                k += 1;
            }
            if k < out_chars.len() && (out_chars[k] == '}' || out_chars[k] == ']') {
                j += 1;
                continue;
            }
        }

        cleaned.push(c);
        j += 1;
    }

    cleaned
}

pub fn load_or_init_config() -> (AppConfig, bool) {
    let path = get_config_path();

    if !path.exists() {
        let mut default_config = AppConfig::default();
        if let Ok(home) = env::var("HOME") {
            let legacy_healc = PathBuf::from(&home).join(".healc.json");
            let legacy_mend = PathBuf::from(&home).join(".mend.json");
            let source_path = if legacy_mend.exists() {
                Some(legacy_mend)
            } else if legacy_healc.exists() {
                Some(legacy_healc)
            } else {
                None
            };
            if let Some(src) = source_path {
                if let Ok(src_content) = fs::read_to_string(&src) {
                    let cleaned = strip_jsonc_comments(&src_content);
                    if let Ok(parsed) = serde_json::from_str::<AppConfig>(&cleaned) {
                        if !parsed.jev_api_key.trim().is_empty() {
                            default_config.jev_api_key = parsed.jev_api_key;
                        }
                    }
                }
            }
        }

        let initial_text = format!(
            "{{\n  // TypeSafe Jev Cloud Evaluation Endpoint\n  \"jev_endpoint\": \"{}\",\n  // Jev API key for Tier 2 autonomous self-healing\n  \"jev_api_key\": \"{}\"\n}}\n",
            default_config.jev_endpoint, default_config.jev_api_key
        );
        let _ = fs::write(&path, initial_text);
        return (default_config, true);
    }

    if let Ok(content) = fs::read_to_string(&path) {
        let cleaned = strip_jsonc_comments(&content);
        if let Ok(config) = serde_json::from_str::<AppConfig>(&cleaned) {
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

    #[test]
    fn test_strip_jsonc_comments_and_parse() {
        let jsonc = r#"
        {
            // Cloud endpoint
            "jev_endpoint": "https://api.typesafe.ai/v1/systemone",
            /* API Key */
            "jev_api_key": "test_key",
            "action_denylist": [
                "PREPEND_SUDO", // trailing comma test
            ],
        }
        "#;
        let cleaned = strip_jsonc_comments(jsonc);
        let parsed: AppConfig = serde_json::from_str(&cleaned).expect("Parse JSONC");
        assert_eq!(parsed.jev_endpoint, "https://api.typesafe.ai/v1/systemone");
        assert_eq!(parsed.jev_api_key, "test_key");
        assert_eq!(parsed.action_denylist, vec!["PREPEND_SUDO".to_string()]);
    }
}
