use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolInputPayload {
    pub command: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentHookInput {
    pub hook_event_name: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input: Option<ToolInputPayload>,
    pub tool_result: Option<Value>,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub stderr: Option<String>,
    pub stdout: Option<String>,
}

impl AgentHookInput {
    pub fn extract_command(&self) -> Option<String> {
        self.command
            .clone()
            .or_else(|| self.tool_input.as_ref().and_then(|ti| ti.command.clone()))
    }

    pub fn extract_exit_code(&self) -> i32 {
        if let Some(ec) = self.exit_code {
            return ec;
        }
        if let Some(Value::Object(map)) = &self.tool_result {
            if let Some(val) = map.get("exit_code").and_then(|v| v.as_i64()) {
                return val as i32;
            }
        }
        1
    }

    pub fn extract_stderr(&self) -> String {
        if let Some(ref err) = self.stderr {
            return err.clone();
        }
        if let Some(ref res) = self.tool_result {
            match res {
                Value::String(s) => return s.clone(),
                Value::Object(map) => {
                    if let Some(Value::String(s)) = map.get("stderr") {
                        return s.clone();
                    }
                    if let Some(Value::String(s)) = map.get("output") {
                        return s.clone();
                    }
                }
                _ => {}
            }
        }
        String::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookDecision {
    Remediated,
    Suggested,
    CircuitBroken,
    Ignored,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationSummary {
    pub strategy: String,
    pub command: String,
    pub confidence: f64,
    pub destructive_risk: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHookOutput {
    pub decision: HookDecision,
    pub original_command: String,
    pub original_exit_code: i32,
    pub new_exit_code: Option<i32>,
    pub suggested_command: Option<String>,
    pub system_message: Option<String>,
    pub updated_output: Option<String>,
    pub remediation: Option<RemediationSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_from_claude_code_post_tool_use() {
        let json_data = serde_json::json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_input": {
                "command": "rtk git push"
            },
            "tool_result": {
                "exit_code": 128,
                "stderr": "fatal: The current branch feat has no upstream branch."
            }
        });

        let input: AgentHookInput = serde_json::from_value(json_data).unwrap();
        assert_eq!(input.extract_command(), Some("rtk git push".to_string()));
        assert_eq!(input.extract_exit_code(), 128);
        assert_eq!(
            input.extract_stderr(),
            "fatal: The current branch feat has no upstream branch."
        );
    }

    #[test]
    fn test_extract_from_generic_flat_input() {
        let json_data = serde_json::json!({
            "command": "cargo build",
            "exit_code": 101,
            "stderr": "error[E0432]: unresolved import"
        });

        let input: AgentHookInput = serde_json::from_value(json_data).unwrap();
        assert_eq!(input.extract_command(), Some("cargo build".to_string()));
        assert_eq!(input.extract_exit_code(), 101);
        assert_eq!(input.extract_stderr(), "error[E0432]: unresolved import");
    }
}
