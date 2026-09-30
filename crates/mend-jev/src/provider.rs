use mend_core::{ActionStrategy, ExecutionState, FailureReason};

/// Trait defining a pluggable domain strategy provider for generating
/// discrete, context-pruned Choice question options.
pub trait DomainCriteriaProvider: Send + Sync {
    /// Whether this provider handles the current execution state
    fn matches(&self, state: &ExecutionState) -> bool;

    /// Domain name reported to Jev
    fn domain_name(&self) -> &'static str;

    /// Available failure reasons for this state, pruned for context
    fn available_reasons(&self, state: &ExecutionState) -> Vec<FailureReason>;

    /// Available remediation actions for this state, pruned for context
    fn available_actions(&self, state: &ExecutionState) -> Vec<ActionStrategy>;
}

/// Provider for Git operations
pub struct GitCriteriaProvider;

impl DomainCriteriaProvider for GitCriteriaProvider {
    fn matches(&self, state: &ExecutionState) -> bool {
        state.argv.first().map(|s| s.as_str()) == Some("git")
    }

    fn domain_name(&self) -> &'static str {
        "git_operations"
    }

    fn available_reasons(&self, state: &ExecutionState) -> Vec<FailureReason> {
        let stderr_lower = state.sanitized_lines.join("\n").to_lowercase();

        if stderr_lower.contains("no upstream") || stderr_lower.contains("set-upstream") {
            vec![FailureReason::GitNoUpstream, FailureReason::Unknown]
        } else if stderr_lower.contains("non-fast-forward")
            || stderr_lower.contains("fetch first")
            || stderr_lower.contains("updates were rejected")
        {
            vec![
                FailureReason::GitNonFastForward,
                FailureReason::GitUncommittedChanges,
                FailureReason::Unknown,
            ]
        } else if stderr_lower.contains("is not a git command")
            || state.entities.contains_key("suggested_subcommand")
        {
            vec![FailureReason::SubcommandNotFound, FailureReason::Unknown]
        } else if state.exit_code == 126 || stderr_lower.contains("permission denied") {
            vec![FailureReason::PermissionDenied, FailureReason::Unknown]
        } else {
            vec![
                FailureReason::GitNoUpstream,
                FailureReason::GitNonFastForward,
                FailureReason::GitUncommittedChanges,
                FailureReason::SubcommandNotFound,
                FailureReason::PermissionDenied,
                FailureReason::Unknown,
            ]
        }
    }

    fn available_actions(&self, state: &ExecutionState) -> Vec<ActionStrategy> {
        let stderr_lower = state.sanitized_lines.join("\n").to_lowercase();
        let has_branch = state.entities.contains_key("branch") || has_git_branch_hint(state);

        let mut actions = Vec::new();

        if stderr_lower.contains("no upstream") || stderr_lower.contains("set-upstream") {
            if has_branch {
                actions.push(ActionStrategy::GitSetUpstream);
            }
        } else if stderr_lower.contains("non-fast-forward")
            || stderr_lower.contains("fetch first")
            || stderr_lower.contains("updates were rejected")
        {
            actions.push(ActionStrategy::GitPullRebase);
            actions.push(ActionStrategy::GitStashPop);
        } else if stderr_lower.contains("is not a git command")
            || state.entities.contains_key("suggested_subcommand")
        {
            if state.entities.contains_key("suggested_subcommand") {
                actions.push(ActionStrategy::SubcommandCorrection);
            }
        } else if state.exit_code == 126 || stderr_lower.contains("permission denied") {
            actions.push(ActionStrategy::PrependSudo);
        } else {
            if has_branch {
                actions.push(ActionStrategy::GitSetUpstream);
            }
            actions.push(ActionStrategy::GitPullRebase);
            actions.push(ActionStrategy::GitStashPop);
            if state.entities.contains_key("suggested_subcommand") {
                actions.push(ActionStrategy::SubcommandCorrection);
            }
            actions.push(ActionStrategy::PrependSudo);
        }

        if !actions.contains(&ActionStrategy::Abort) {
            actions.push(ActionStrategy::Abort);
        }

        actions
    }
}

/// Provider for package manager commands (npm, pnpm, yarn, bun)
pub struct PackageManagerCriteriaProvider;

impl DomainCriteriaProvider for PackageManagerCriteriaProvider {
    fn matches(&self, state: &ExecutionState) -> bool {
        let first_cmd = state.argv.first().map(|s| s.as_str()).unwrap_or("");
        matches!(first_cmd, "npm" | "pnpm" | "pn" | "yarn" | "bun")
    }

    fn domain_name(&self) -> &'static str {
        "package_managers"
    }

    fn available_reasons(&self, state: &ExecutionState) -> Vec<FailureReason> {
        let stderr_lower = state.sanitized_lines.join("\n").to_lowercase();
        if state.exit_code == 126 || stderr_lower.contains("permission denied") {
            vec![FailureReason::PermissionDenied, FailureReason::Unknown]
        } else {
            vec![
                FailureReason::PackageScriptNotFound,
                FailureReason::MissingPackageOrBinary,
                FailureReason::PermissionDenied,
                FailureReason::Unknown,
            ]
        }
    }

    fn available_actions(&self, state: &ExecutionState) -> Vec<ActionStrategy> {
        let first_cmd = state.argv.first().map(|s| s.as_str()).unwrap_or("");
        let stderr_lower = state.sanitized_lines.join("\n").to_lowercase();

        let mut actions = Vec::new();

        if state.exit_code == 126 || stderr_lower.contains("permission denied") {
            actions.push(ActionStrategy::PrependSudo);
        } else {
            match first_cmd {
                "pnpm" | "pn" => actions.push(ActionStrategy::PnpmRunScript),
                "npm" => actions.push(ActionStrategy::NpmRunScript),
                "yarn" => actions.push(ActionStrategy::YarnRunScript),
                "bun" => actions.push(ActionStrategy::BunRunScript),
                _ => {
                    actions.push(ActionStrategy::PnpmRunScript);
                    actions.push(ActionStrategy::NpmRunScript);
                }
            }
            actions.push(ActionStrategy::PrependSudo);
        }

        if !actions.contains(&ActionStrategy::Abort) {
            actions.push(ActionStrategy::Abort);
        }

        actions
    }
}

/// Provider for Rust / Cargo commands
pub struct CargoCriteriaProvider;

impl DomainCriteriaProvider for CargoCriteriaProvider {
    fn matches(&self, state: &ExecutionState) -> bool {
        state.argv.first().map(|s| s.as_str()) == Some("cargo")
    }

    fn domain_name(&self) -> &'static str {
        "rust_cargo"
    }

    fn available_reasons(&self, state: &ExecutionState) -> Vec<FailureReason> {
        let stderr_lower = state.sanitized_lines.join("\n").to_lowercase();
        if stderr_lower.contains("no such subcommand")
            || state.entities.contains_key("suggested_subcommand")
        {
            vec![FailureReason::SubcommandNotFound, FailureReason::Unknown]
        } else if stderr_lower.contains("could not find")
            || stderr_lower.contains("unresolved import")
            || state.entities.contains_key("package")
        {
            vec![
                FailureReason::MissingPackageOrBinary,
                FailureReason::Unknown,
            ]
        } else {
            vec![
                FailureReason::SubcommandNotFound,
                FailureReason::MissingPackageOrBinary,
                FailureReason::Unknown,
            ]
        }
    }

    fn available_actions(&self, state: &ExecutionState) -> Vec<ActionStrategy> {
        let mut actions = Vec::new();
        let stderr_lower = state.sanitized_lines.join("\n").to_lowercase();

        if state.entities.contains_key("suggested_subcommand") {
            actions.push(ActionStrategy::SubcommandCorrection);
        }

        if state.entities.contains_key("package") {
            actions.push(ActionStrategy::CargoAddDependency);
            actions.push(ActionStrategy::CargoInstallPackage);
        }

        if actions.is_empty() && !stderr_lower.contains("no such subcommand") {
            actions.push(ActionStrategy::CargoInstallPackage);
        }

        if !actions.contains(&ActionStrategy::Abort) {
            actions.push(ActionStrategy::Abort);
        }

        actions
    }
}

/// Provider for Python commands
pub struct PythonCriteriaProvider;

impl DomainCriteriaProvider for PythonCriteriaProvider {
    fn matches(&self, state: &ExecutionState) -> bool {
        let first_cmd = state.argv.first().map(|s| s.as_str()).unwrap_or("");
        matches!(first_cmd, "python" | "python3" | "pip" | "pip3")
    }

    fn domain_name(&self) -> &'static str {
        "python_ecosystem"
    }

    fn available_reasons(&self, state: &ExecutionState) -> Vec<FailureReason> {
        let stderr_lower = state.sanitized_lines.join("\n").to_lowercase();
        if state.exit_code == 126 || stderr_lower.contains("permission denied") {
            vec![FailureReason::PermissionDenied, FailureReason::Unknown]
        } else if stderr_lower.contains("no module named")
            || state.entities.contains_key("python_module")
        {
            vec![
                FailureReason::MissingPackageOrBinary,
                FailureReason::Unknown,
            ]
        } else {
            vec![
                FailureReason::MissingPackageOrBinary,
                FailureReason::PermissionDenied,
                FailureReason::Unknown,
            ]
        }
    }

    fn available_actions(&self, state: &ExecutionState) -> Vec<ActionStrategy> {
        let mut actions = Vec::new();
        let stderr_lower = state.sanitized_lines.join("\n").to_lowercase();

        if state.exit_code == 126 || stderr_lower.contains("permission denied") {
            actions.push(ActionStrategy::PrependSudo);
        } else {
            actions.push(ActionStrategy::PipInstallPackage);
            actions.push(ActionStrategy::PrependSudo);
        }

        if !actions.contains(&ActionStrategy::Abort) {
            actions.push(ActionStrategy::Abort);
        }

        actions
    }
}

/// Fallback provider for System CLI commands with OS & Entity aware pruning
pub struct SystemCliCriteriaProvider;

impl DomainCriteriaProvider for SystemCliCriteriaProvider {
    fn matches(&self, _state: &ExecutionState) -> bool {
        true
    }

    fn domain_name(&self) -> &'static str {
        "system_cli"
    }

    fn available_reasons(&self, state: &ExecutionState) -> Vec<FailureReason> {
        let stderr_lower = state.sanitized_lines.join("\n").to_lowercase();
        let cmd_lower = state.command.to_lowercase();

        let is_docker_daemon_error = (cmd_lower.contains("docker")
            || stderr_lower.contains("docker daemon")
            || stderr_lower.contains("cannot connect to the docker")
            || stderr_lower.contains("failed to connect to the docker")
            || stderr_lower.contains("docker.sock"))
            && state.exit_code != 127
            && !stderr_lower.contains("command not found");

        if state.exit_code == 126 || stderr_lower.contains("permission denied") {
            vec![
                FailureReason::PermissionDenied,
                FailureReason::PermissionNotExecutable,
                FailureReason::Unknown,
            ]
        } else if is_docker_daemon_error {
            vec![FailureReason::DaemonNotRunning, FailureReason::Unknown]
        } else if state.exit_code == 127 || stderr_lower.contains("command not found") {
            vec![
                FailureReason::CommandNotFound,
                FailureReason::MissingPackageOrBinary,
                FailureReason::Unknown,
            ]
        } else if stderr_lower.contains("no such file or directory") {
            vec![FailureReason::NoSuchFileOrDirectory, FailureReason::Unknown]
        } else {
            vec![
                FailureReason::PermissionDenied,
                FailureReason::CommandNotFound,
                FailureReason::SubcommandNotFound,
                FailureReason::MissingPackageOrBinary,
                FailureReason::NoSuchFileOrDirectory,
                FailureReason::PermissionNotExecutable,
                FailureReason::DaemonNotRunning,
                FailureReason::Unknown,
            ]
        }
    }

    fn available_actions(&self, state: &ExecutionState) -> Vec<ActionStrategy> {
        let stderr_lower = state.sanitized_lines.join("\n").to_lowercase();
        let cmd_lower = state.command.to_lowercase();
        let is_macos = cfg!(target_os = "macos");

        let is_docker_daemon_error = (cmd_lower.contains("docker")
            || stderr_lower.contains("docker daemon")
            || stderr_lower.contains("cannot connect to the docker")
            || stderr_lower.contains("failed to connect to the docker")
            || stderr_lower.contains("docker.sock"))
            && state.exit_code != 127
            && !stderr_lower.contains("command not found");

        let mut actions = Vec::new();

        if state.exit_code == 126 || stderr_lower.contains("permission denied") {
            actions.push(ActionStrategy::PrependSudo);
            actions.push(ActionStrategy::ChmodExecutable);
        } else if is_docker_daemon_error {
            actions.push(ActionStrategy::DockerStartDaemon);
            actions.push(ActionStrategy::PrependSudo);
        } else if state.exit_code == 127 || stderr_lower.contains("command not found") {
            if let Some(suggested) = state.entities.get("suggested_binary") {
                if state.argv.first().map(|s| s.as_str()) != Some(suggested.as_str()) {
                    actions.push(ActionStrategy::PathCorrection);
                }
            }
            if is_macos {
                actions.push(ActionStrategy::BrewInstallPackage);
            } else {
                actions.push(ActionStrategy::AptInstallPackage);
            }
            actions.push(ActionStrategy::CargoInstallPackage);
        } else if stderr_lower.contains("no such file or directory") {
            actions.push(ActionStrategy::MakeDirectory);
            actions.push(ActionStrategy::PrependSudo);
        } else {
            actions.push(ActionStrategy::PrependSudo);
            if let Some(suggested) = state.entities.get("suggested_binary") {
                if state.argv.first().map(|s| s.as_str()) != Some(suggested.as_str()) {
                    actions.push(ActionStrategy::PathCorrection);
                }
            }
            if state.entities.contains_key("suggested_subcommand") {
                actions.push(ActionStrategy::SubcommandCorrection);
            }
            if is_macos {
                actions.push(ActionStrategy::BrewInstallPackage);
            } else {
                actions.push(ActionStrategy::AptInstallPackage);
            }
            actions.push(ActionStrategy::CargoInstallPackage);
            actions.push(ActionStrategy::MakeDirectory);
            actions.push(ActionStrategy::ChmodExecutable);
        }

        if !actions.contains(&ActionStrategy::Abort) {
            actions.push(ActionStrategy::Abort);
        }

        actions
    }
}

fn has_git_branch_hint(state: &ExecutionState) -> bool {
    let text = format!("{}\n{}", state.command, state.sanitized_lines.join("\n"));
    for word in text.split_whitespace() {
        if word.starts_with("feat") || word.starts_with("fix") || word == "main" || word == "master"
        {
            return true;
        }
    }
    false
}
