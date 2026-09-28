use crate::TemplateRenderer;
use mend_core::{ActionStrategy, ExecutionState, RemediationCandidate, Rule};

/// Rule: python missing module -> pip install <module>
pub struct PythonMissingModuleRule;

impl Rule for PythonMissingModuleRule {
    fn name(&self) -> &'static str {
        "python_missing_module"
    }

    fn matches(&self, state: &ExecutionState) -> bool {
        let first = state.argv.first().map(|s| s.as_str()).unwrap_or("");
        (first.starts_with("python") || first == "pip")
            && state.entities.contains_key("python_module")
    }

    fn produce_remediation(&self, state: &ExecutionState) -> Option<RemediationCandidate> {
        TemplateRenderer::render(&ActionStrategy::PipInstallPackage, state, 0.95, 0.05, true).ok()
    }
}
