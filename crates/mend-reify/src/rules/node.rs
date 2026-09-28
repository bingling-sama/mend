use crate::TemplateRenderer;
use mend_core::{ActionStrategy, ExecutionState, RemediationCandidate, Rule};

/// Rule: Node package managers missing `run` before script (e.g. `pn dev` -> `pn run dev`)
pub struct PnpmMissingRunRule;

impl Rule for PnpmMissingRunRule {
    fn name(&self) -> &'static str {
        "pnpm_missing_run"
    }

    fn matches(&self, state: &ExecutionState) -> bool {
        let first = state.argv.first().map(|s| s.as_str()).unwrap_or("");
        (first == "pn" || first == "pnpm")
            && (state.entities.contains_key("package_script")
                || state.sanitized_lines.iter().any(|l| {
                    l.contains("Command \"") && l.contains("\" not found")
                        || l.contains("ERR_PNPM_RECURSIVE_EXEC_FIRST_FAIL")
                }))
    }

    fn produce_remediation(&self, state: &ExecutionState) -> Option<RemediationCandidate> {
        TemplateRenderer::render(&ActionStrategy::PnpmRunScript, state, 0.98, 0.01, true).ok()
    }
}

pub struct NpmMissingRunRule;

impl Rule for NpmMissingRunRule {
    fn name(&self) -> &'static str {
        "npm_missing_run"
    }

    fn matches(&self, state: &ExecutionState) -> bool {
        let first = state.argv.first().map(|s| s.as_str()).unwrap_or("");
        first == "npm"
            && (state.entities.contains_key("package_script")
                || state.sanitized_lines.iter().any(|l| {
                    l.contains("Missing script:") || l.contains("npm error Missing script")
                }))
    }

    fn produce_remediation(&self, state: &ExecutionState) -> Option<RemediationCandidate> {
        TemplateRenderer::render(&ActionStrategy::NpmRunScript, state, 0.98, 0.01, true).ok()
    }
}

pub struct YarnMissingRunRule;

impl Rule for YarnMissingRunRule {
    fn name(&self) -> &'static str {
        "yarn_missing_run"
    }

    fn matches(&self, state: &ExecutionState) -> bool {
        let first = state.argv.first().map(|s| s.as_str()).unwrap_or("");
        first == "yarn"
            && state
                .sanitized_lines
                .iter()
                .any(|l| l.contains("error Command") && l.contains("not found"))
    }

    fn produce_remediation(&self, state: &ExecutionState) -> Option<RemediationCandidate> {
        TemplateRenderer::render(&ActionStrategy::YarnRunScript, state, 0.98, 0.01, true).ok()
    }
}
