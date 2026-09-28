use crate::TemplateRenderer;
use mend_core::{ActionStrategy, ExecutionState, RemediationCandidate, Rule};

/// Rule: git push with no upstream -> git push --set-upstream origin <branch>
pub struct GitSetUpstreamRule;

impl Rule for GitSetUpstreamRule {
    fn name(&self) -> &'static str {
        "git_set_upstream"
    }

    fn matches(&self, state: &ExecutionState) -> bool {
        state.argv.first().map(|s| s.as_str()) == Some("git")
            && (state.entities.contains_key("remote") || state.entities.contains_key("branch"))
            && state.sanitized_lines.iter().any(|l| {
                l.contains("has no upstream branch")
                    || l.contains("set-upstream")
                    || l.contains("fatal: The current branch")
            })
    }

    fn produce_remediation(&self, state: &ExecutionState) -> Option<RemediationCandidate> {
        TemplateRenderer::render(&ActionStrategy::GitSetUpstream, state, 0.98, 0.05, true).ok()
    }
}

/// Rule: git pull needed before push -> git pull --rebase
pub struct GitPullRebaseRule;

impl Rule for GitPullRebaseRule {
    fn name(&self) -> &'static str {
        "git_pull_rebase"
    }

    fn matches(&self, state: &ExecutionState) -> bool {
        state.argv.first().map(|s| s.as_str()) == Some("git")
            && state.sanitized_lines.iter().any(|l| {
                l.contains("Updates were rejected because the remote contains work")
                    || l.contains("hint: Updates were rejected")
                    || l.contains("fetch first")
                    || l.contains("non-fast-forward")
            })
    }

    fn produce_remediation(&self, state: &ExecutionState) -> Option<RemediationCandidate> {
        TemplateRenderer::render(&ActionStrategy::GitPullRebase, state, 0.95, 0.05, true).ok()
    }
}

/// Rule: git branch typos (e.g., git brnach -> git branch)
pub struct GitSubcommandTypoRule;

impl Rule for GitSubcommandTypoRule {
    fn name(&self) -> &'static str {
        "git_subcommand_typo"
    }

    fn matches(&self, state: &ExecutionState) -> bool {
        state.argv.first().map(|s| s.as_str()) == Some("git")
            && state.entities.contains_key("suggested_subcommand")
    }

    fn produce_remediation(&self, state: &ExecutionState) -> Option<RemediationCandidate> {
        TemplateRenderer::render(
            &ActionStrategy::SubcommandCorrection,
            state,
            0.97,
            0.01,
            true,
        )
        .ok()
    }
}
