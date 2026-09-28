use crate::TemplateRenderer;
use mend_core::{ActionStrategy, ExecutionState, RemediationCandidate, Rule};

/// Rule: permission denied -> prepend sudo
pub struct SudoRule;

impl Rule for SudoRule {
    fn name(&self) -> &'static str {
        "sudo"
    }

    fn matches(&self, state: &ExecutionState) -> bool {
        state.exit_code == 126
            || state.sanitized_lines.iter().any(|l| {
                l.contains("Permission denied")
                    || l.contains("EACCES")
                    || l.contains("Operation not permitted")
                    || l.contains("are you root?")
            })
    }

    fn produce_remediation(&self, state: &ExecutionState) -> Option<RemediationCandidate> {
        TemplateRenderer::render(&ActionStrategy::PrependSudo, state, 1.0, 0.05, true).ok()
    }
}

/// Rule: missing directory -> mkdir -p <path>
pub struct MkdirRule;

impl Rule for MkdirRule {
    fn name(&self) -> &'static str {
        "mkdir_p"
    }

    fn matches(&self, state: &ExecutionState) -> bool {
        let first = state.argv.first().map(|s| s.as_str()).unwrap_or("");
        (first == "cd" || first == "cat" || first == "touch" || first == "cp")
            && state.entities.contains_key("path")
            && state
                .sanitized_lines
                .iter()
                .any(|l| l.contains("No such file or directory"))
    }

    fn produce_remediation(&self, state: &ExecutionState) -> Option<RemediationCandidate> {
        TemplateRenderer::render(&ActionStrategy::MakeDirectory, state, 0.90, 0.05, true).ok()
    }
}
