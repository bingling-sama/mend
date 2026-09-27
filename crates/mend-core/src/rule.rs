use crate::{ExecutionState, RemediationCandidate};

/// The Rule trait represents a single deterministic remediation matcher and generator.
/// Heavily inspired by the best patterns in `thefuck`, but implemented in zero-overhead,
/// memory-safe Rust with explicit confidence and risk scoring.
pub trait Rule: Send + Sync {
    /// Unique identifier for the rule (e.g. "git_set_upstream", "pnpm_run_script")
    fn name(&self) -> &'static str;

    /// Evaluates if the current execution state matches this failure pattern
    fn matches(&self, state: &ExecutionState) -> bool;

    /// Produces the remediation candidate if matches returned true
    fn produce_remediation(&self, state: &ExecutionState) -> Option<RemediationCandidate>;
}

/// Registry holding active remediation rules
pub struct RuleRegistry {
    rules: Vec<Box<dyn Rule>>,
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register(&mut self, rule: Box<dyn Rule>) {
        self.rules.push(rule);
    }

    pub fn evaluate(&self, state: &ExecutionState) -> Option<RemediationCandidate> {
        for rule in &self.rules {
            if rule.matches(state) {
                if let Some(cand) = rule.produce_remediation(state) {
                    return Some(cand);
                }
            }
        }
        None
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}
