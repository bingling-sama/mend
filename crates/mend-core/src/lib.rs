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
    pub const ALL: &'static [ActionStrategy] = &[
        ActionStrategy::PrependSudo,
        ActionStrategy::GitSetUpstream,
        ActionStrategy::GitPullRebase,
        ActionStrategy::GitStashPop,
        ActionStrategy::GitCheckoutBranch,
        ActionStrategy::GitBranchCreate,
        ActionStrategy::AptInstallPackage,
        ActionStrategy::BrewInstallPackage,
        ActionStrategy::CargoInstallPackage,
        ActionStrategy::NpmRunScript,
        ActionStrategy::PnpmRunScript,
        ActionStrategy::YarnRunScript,
        ActionStrategy::BunRunScript,
        ActionStrategy::CargoAddDependency,
        ActionStrategy::PipInstallPackage,
        ActionStrategy::PathCorrection,
        ActionStrategy::SubcommandCorrection,
        ActionStrategy::MakeDirectory,
        ActionStrategy::ChmodExecutable,
        ActionStrategy::DockerStartDaemon,
        ActionStrategy::Abort,
    ];

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

impl std::str::FromStr for ActionStrategy {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim().trim_matches('"');
        let res = match trimmed {
            "PREPEND_SUDO" => Self::PrependSudo,
            "GIT_SET_UPSTREAM" => Self::GitSetUpstream,
            "GIT_PULL_REBASE" => Self::GitPullRebase,
            "GIT_STASH_POP" => Self::GitStashPop,
            "GIT_CHECKOUT_BRANCH" => Self::GitCheckoutBranch,
            "GIT_BRANCH_CREATE" => Self::GitBranchCreate,
            "APT_INSTALL_PACKAGE" => Self::AptInstallPackage,
            "BREW_INSTALL_PACKAGE" => Self::BrewInstallPackage,
            "CARGO_INSTALL_PACKAGE" => Self::CargoInstallPackage,
            "NPM_RUN_SCRIPT" => Self::NpmRunScript,
            "PNPM_RUN_SCRIPT" => Self::PnpmRunScript,
            "YARN_RUN_SCRIPT" => Self::YarnRunScript,
            "BUN_RUN_SCRIPT" => Self::BunRunScript,
            "CARGO_ADD_DEPENDENCY" => Self::CargoAddDependency,
            "PIP_INSTALL_PACKAGE" => Self::PipInstallPackage,
            "PATH_CORRECTION" => Self::PathCorrection,
            "SUBCOMMAND_CORRECTION" => Self::SubcommandCorrection,
            "MAKE_DIRECTORY" => Self::MakeDirectory,
            "CHMOD_EXECUTABLE" => Self::ChmodExecutable,
            "DOCKER_START_DAEMON" => Self::DockerStartDaemon,
            "ABORT" => Self::Abort,
            _ => Self::Abort,
        };
        Ok(res)
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

impl FailureReason {
    pub const ALL: &'static [FailureReason] = &[
        FailureReason::PermissionDenied,
        FailureReason::CommandNotFound,
        FailureReason::SubcommandNotFound,
        FailureReason::PackageScriptNotFound,
        FailureReason::GitNoUpstream,
        FailureReason::GitNonFastForward,
        FailureReason::GitUncommittedChanges,
        FailureReason::MissingPackageOrBinary,
        FailureReason::NoSuchFileOrDirectory,
        FailureReason::PermissionNotExecutable,
        FailureReason::DaemonNotRunning,
        FailureReason::Unknown,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::CommandNotFound => "COMMAND_NOT_FOUND",
            Self::SubcommandNotFound => "SUBCOMMAND_NOT_FOUND",
            Self::PackageScriptNotFound => "PACKAGE_SCRIPT_NOT_FOUND",
            Self::GitNoUpstream => "GIT_NO_UPSTREAM",
            Self::GitNonFastForward => "GIT_NON_FAST_FORWARD",
            Self::GitUncommittedChanges => "GIT_UNCOMMITTED_CHANGES",
            Self::MissingPackageOrBinary => "MISSING_PACKAGE_OR_BINARY",
            Self::NoSuchFileOrDirectory => "NO_SUCH_FILE_OR_DIRECTORY",
            Self::PermissionNotExecutable => "PERMISSION_NOT_EXECUTABLE",
            Self::DaemonNotRunning => "DAEMON_NOT_RUNNING",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl std::str::FromStr for FailureReason {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim().trim_matches('"');
        let res = match trimmed {
            "PERMISSION_DENIED" => Self::PermissionDenied,
            "COMMAND_NOT_FOUND" => Self::CommandNotFound,
            "SUBCOMMAND_NOT_FOUND" => Self::SubcommandNotFound,
            "PACKAGE_SCRIPT_NOT_FOUND" => Self::PackageScriptNotFound,
            "GIT_NO_UPSTREAM" => Self::GitNoUpstream,
            "GIT_NON_FAST_FORWARD" => Self::GitNonFastForward,
            "GIT_UNCOMMITTED_CHANGES" => Self::GitUncommittedChanges,
            "MISSING_PACKAGE_OR_BINARY" => Self::MissingPackageOrBinary,
            "NO_SUCH_FILE_OR_DIRECTORY" => Self::NoSuchFileOrDirectory,
            "PERMISSION_NOT_EXECUTABLE" => Self::PermissionNotExecutable,
            "DAEMON_NOT_RUNNING" => Self::DaemonNotRunning,
            "UNKNOWN" => Self::Unknown,
            _ => Self::Unknown,
        };
        Ok(res)
    }
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
    fn test_all_action_strategy_roundtrip() {
        for action in ActionStrategy::ALL {
            let s = action.as_str();
            let parsed: ActionStrategy = s.parse().unwrap();
            assert_eq!(&parsed, action, "Mismatch for ActionStrategy {}", s);
        }
    }

    #[test]
    fn test_all_failure_reason_roundtrip() {
        for reason in FailureReason::ALL {
            let s = reason.as_str();
            let parsed: FailureReason = s.parse().unwrap();
            assert_eq!(&parsed, reason, "Mismatch for FailureReason {}", s);
        }
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
