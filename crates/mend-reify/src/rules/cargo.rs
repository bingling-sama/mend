use mend_core::{ActionStrategy, ExecutionState, RemediationCandidate, Rule};
use crate::TemplateRenderer;

/// Rule: cargo typo subcommand (e.g. cargo bulid -> cargo build)
pub struct CargoSubcommandTypoRule;

impl Rule for CargoSubcommandTypoRule {
    fn name(&self) -> &'static str {
        "cargo_subcommand_typo"
    }

    fn matches(&self, state: &ExecutionState) -> bool {
        state.argv.first().map(|s| s.as_str()) == Some("cargo")
            && state.entities.contains_key("suggested_subcommand")
    }

    fn produce_remediation(&self, state: &ExecutionState) -> Option<RemediationCandidate> {
        TemplateRenderer::render(&ActionStrategy::SubcommandCorrection, state, 0.98, 0.01, true).ok()
    }
}
