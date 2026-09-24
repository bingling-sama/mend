use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Action strategy representing discrete remediation operations.
/// All actions are strictly bounded enums without arbitrary shell generation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionStrategy {
    PrependSudo,
    GitSetUpstream,
    GitPullRebase,
    GitStashPop,
    GitCheckoutBranch,
    GitBranchCreate,
    AptInstallPackage,
    BrewInstallPackage,
    CargoInstallPackage,
    NpmRunScript,
    PnpmRunScript,
    YarnRunScript,
    BunRunScript,
    CargoAddDependency,
    PipInstallPackage,
    PathCorrection,
    SubcommandCorrection,
    MakeDirectory,
    ChmodExecutable,
    DockerStartDaemon,
    Abort,
}

impl ActionStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PrependSudo => "PREPEND_SUDO",
            Self::GitSetUpstream => "GIT_SET_UPSTREAM",
            Self::GitPullRebase => "GIT_PULL_REBASE",
            Self::GitStashPop => "GIT_STASH_POP",
            Self::GitCheckoutBranch => "GIT_CHECKOUT_BRANCH",
            Self::GitBranchCreate => "GIT_BRANCH_CREATE",
            Self::AptInstallPackage => "APT_INSTALL_PACKAGE",
            Self::BrewInstallPackage => "BREW_INSTALL_PACKAGE",
            Self::CargoInstallPackage => "CARGO_INSTALL_PACKAGE",
            Self::NpmRunScript => "NPM_RUN_SCRIPT",
            Self::PnpmRunScript => "PNPM_RUN_SCRIPT",
            Self::YarnRunScript => "YARN_RUN_SCRIPT",
            Self::BunRunScript => "BUN_RUN_SCRIPT",
            Self::CargoAddDependency => "CARGO_ADD_DEPENDENCY",
            Self::PipInstallPackage => "PIP_INSTALL_PACKAGE",
            Self::PathCorrection => "PATH_CORRECTION",
            Self::SubcommandCorrection => "SUBCOMMAND_CORRECTION",
            Self::MakeDirectory => "MAKE_DIRECTORY",
            Self::ChmodExecutable => "CHMOD_EXECUTABLE",
            Self::DockerStartDaemon => "DOCKER_START_DAEMON",
            Self::Abort => "ABORT",
        }
    }
}

/// Root cause categorization for the execution failure.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FailureReason {
    PermissionDenied,
    CommandNotFound,
    SubcommandNotFound,
    PackageScriptNotFound,
    GitNoUpstream,
    GitNonFastForward,
    GitUncommittedChanges,
    MissingPackageOrBinary,
    NoSuchFileOrDirectory,
    PermissionNotExecutable,
    DaemonNotRunning,
    Unknown,
}

/// Captured execution state before remediation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionState {
    /// Original raw command line executed by the user/agent
    pub command: String,
    /// Parsed command tokens (argv)
    pub argv: Vec<String>,
    /// Exit status code returned by the command
    pub exit_code: i32,
    /// Working directory during execution
    pub cwd: String,
    /// Sanitized, tail-truncated stderr/stdout lines
    pub sanitized_lines: Vec<String>,
    /// Environment variables snapshot (selected critical vars)
    pub env: HashMap<String, String>,
    /// Structured entities extracted by regex or fast-path heuristics
    pub entities: HashMap<String, String>,
}

impl ExecutionState {
    pub fn new(command: impl Into<String>, exit_code: i32) -> Self {
        let cmd = command.into();
        let argv = cmd.split_whitespace().map(String::from).collect();
        Self {
            command: cmd,
            argv,
            exit_code,
            cwd: std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| ".".into()),
            sanitized_lines: Vec::new(),
            env: HashMap::new(),
            entities: HashMap::new(),
        }
    }

    pub fn with_sanitized_lines(mut self, lines: Vec<String>) -> Self {
        self.sanitized_lines = lines;
        self
    }

    pub fn with_entities(mut self, entities: HashMap<String, String>) -> Self {
        self.entities = entities;
        self
    }
}

/// Evaluation question types matching TypeSafe Jev schema specifications
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QuestionSpec {
    Choice {
        id: String,
        prompt: String,
        options: Vec<String>,
    },
    Noul {
        id: String,
        prompt: String,
        min: f64,
        max: f64,
    },
}

/// Request payload sent to Jev for evaluation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionPlan {
    pub domain: String,
    pub context_text: String,
    pub questions: Vec<QuestionSpec>,
}

/// Unified response from Jev engine evaluations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JevResponse {
    pub failure_reason: FailureReason,
    pub failure_reason_confidence: f64,
    pub remediation_action: ActionStrategy,
    pub action_confidence: f64,
    /// Destructive risk score normalized between 0.0 (safe) and 1.0 (dangerous)
    pub destructive_risk: f64,
}

/// Detailed remediation candidate produced by reification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemediationCandidate {
    pub strategy: ActionStrategy,
    pub rendered_command: String,
    pub explanation: String,
    pub confidence: f64,
    pub destructive_risk: f64,
    pub is_fast_path: bool,
}

/// Safety validation errors
#[derive(Debug, Error, PartialEq)]
pub enum SafetyError {
    #[error("Remediation confidence {0:.2} is below required threshold {1:.2}")]
    LowConfidence(f64, f64),
    #[error("Action destructive risk {0:.2} exceeds maximum allowed risk {1:.2}")]
    DestructiveAction(f64, f64),
    #[error("Action strategy {0:?} was aborted or invalid")]
    Aborted(ActionStrategy),
    #[error("Missing required parameter '{0}' to render command template")]
    MissingEntity(String),
}

pub mod rule;
pub use rule::{Rule, RuleRegistry};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_strategy_serialization() {
        let action = ActionStrategy::GitSetUpstream;
        let serialized = serde_json::to_string(&action).unwrap();
        assert_eq!(serialized, "\"GIT_SET_UPSTREAM\"");
        let deserialized: ActionStrategy = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, action);
    }

    #[test]
    fn test_execution_state_creation() {
        let state = ExecutionState::new("git push origin main", 1)
            .with_sanitized_lines(vec!["error: src refspec main does not match any".into()]);
        assert_eq!(state.argv, vec!["git", "push", "origin", "main"]);
        assert_eq!(state.exit_code, 1);
        assert_eq!(state.sanitized_lines.len(), 1);
    }
}
