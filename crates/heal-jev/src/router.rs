use heal_core::{ActionStrategy, DecisionPlan, ExecutionState, QuestionSpec};

pub struct CriteriaRouter;

impl CriteriaRouter {
    pub fn build_decision_plan(state: &ExecutionState) -> DecisionPlan {
        let first_cmd = state.argv.first().map(|s| s.as_str()).unwrap_or("");
        let is_git = first_cmd == "git";
        let is_pkg_mgr = matches!(first_cmd, "npm" | "pnpm" | "pn" | "yarn" | "bun");
        let is_cargo = first_cmd == "cargo";
        let is_python = matches!(first_cmd, "python" | "python3" | "pip");

        let (domain, failure_reasons, remediation_actions) = if is_git {
            (
                "git_operations".to_string(),
                vec![
                    "GIT_NO_UPSTREAM".to_string(),
                    "GIT_NON_FAST_FORWARD".to_string(),
                    "GIT_UNCOMMITTED_CHANGES".to_string(),
                    "SUBCOMMAND_NOT_FOUND".to_string(),
                    "PERMISSION_DENIED".to_string(),
                    "UNKNOWN".to_string(),
                ],
                vec![
                    ActionStrategy::GitSetUpstream.as_str().to_string(),
                    ActionStrategy::GitPullRebase.as_str().to_string(),
                    ActionStrategy::GitStashPop.as_str().to_string(),
                    ActionStrategy::SubcommandCorrection.as_str().to_string(),
                    ActionStrategy::PrependSudo.as_str().to_string(),
                    ActionStrategy::Abort.as_str().to_string(),
                ],
            )
        } else if is_pkg_mgr {
            (
                "package_managers".to_string(),
                vec![
                    "PACKAGE_SCRIPT_NOT_FOUND".to_string(),
                    "MISSING_PACKAGE_OR_BINARY".to_string(),
                    "PERMISSION_DENIED".to_string(),
                    "UNKNOWN".to_string(),
                ],
                vec![
                    ActionStrategy::PnpmRunScript.as_str().to_string(),
                    ActionStrategy::NpmRunScript.as_str().to_string(),
                    ActionStrategy::YarnRunScript.as_str().to_string(),
                    ActionStrategy::BunRunScript.as_str().to_string(),
                    ActionStrategy::PrependSudo.as_str().to_string(),
                    ActionStrategy::Abort.as_str().to_string(),
                ],
            )
        } else if is_cargo {
            (
                "rust_cargo".to_string(),
                vec![
                    "SUBCOMMAND_NOT_FOUND".to_string(),
                    "MISSING_PACKAGE_OR_BINARY".to_string(),
                    "UNKNOWN".to_string(),
                ],
                vec![
                    ActionStrategy::SubcommandCorrection.as_str().to_string(),
                    ActionStrategy::CargoAddDependency.as_str().to_string(),
                    ActionStrategy::CargoInstallPackage.as_str().to_string(),
                    ActionStrategy::Abort.as_str().to_string(),
                ],
            )
        } else if is_python {
            (
                "python_ecosystem".to_string(),
                vec![
                    "MISSING_PACKAGE_OR_BINARY".to_string(),
                    "PERMISSION_DENIED".to_string(),
                    "UNKNOWN".to_string(),
                ],
                vec![
                    ActionStrategy::PipInstallPackage.as_str().to_string(),
                    ActionStrategy::PrependSudo.as_str().to_string(),
                    ActionStrategy::Abort.as_str().to_string(),
                ],
            )
        } else {
            (
                "system_cli".to_string(),
                vec![
                    "PERMISSION_DENIED".to_string(),
                    "COMMAND_NOT_FOUND".to_string(),
                    "SUBCOMMAND_NOT_FOUND".to_string(),
                    "MISSING_PACKAGE_OR_BINARY".to_string(),
                    "NO_SUCH_FILE_OR_DIRECTORY".to_string(),
                    "PERMISSION_NOT_EXECUTABLE".to_string(),
                    "DAEMON_NOT_RUNNING".to_string(),
                    "UNKNOWN".to_string(),
                ],
                vec![
                    ActionStrategy::PrependSudo.as_str().to_string(),
                    ActionStrategy::PathCorrection.as_str().to_string(),
                    ActionStrategy::SubcommandCorrection.as_str().to_string(),
                    ActionStrategy::AptInstallPackage.as_str().to_string(),
                    ActionStrategy::BrewInstallPackage.as_str().to_string(),
                    ActionStrategy::CargoInstallPackage.as_str().to_string(),
                    ActionStrategy::MakeDirectory.as_str().to_string(),
                    ActionStrategy::ChmodExecutable.as_str().to_string(),
                    ActionStrategy::DockerStartDaemon.as_str().to_string(),
                    ActionStrategy::Abort.as_str().to_string(),
                ],
            )
        };

        let context_text = format!(
            "COMMAND: {}\nEXIT_CODE: {}\nCWD: {}\nSTDERR_TAIL:\n{}",
            state.command,
            state.exit_code,
            state.cwd,
            state.sanitized_lines.join("\n")
        );

        let questions = vec![
            QuestionSpec::Choice {
                id: "failure_reason".to_string(),
                prompt: "Classify the root cause of this execution failure.".to_string(),
                options: failure_reasons,
            },
            QuestionSpec::Choice {
                id: "remediation_action".to_string(),
                prompt: "Select the discrete remediation action required to fix this command."
                    .to_string(),
                options: remediation_actions,
            },
            QuestionSpec::Noul {
                id: "destructive_risk".to_string(),
                prompt: "Rate destructive risk of executing the remediation from 0.0 (safe read/config) to 1.0 (dangerous state deletion/overwrite)."
                    .to_string(),
                min: 0.0,
                max: 1.0,
            },
        ];

        DecisionPlan {
            domain,
            context_text,
            questions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_routing() {
        let state = ExecutionState::new("git push origin master", 1)
            .with_sanitized_lines(vec!["fatal: current branch has no upstream".into()]);
        let plan = CriteriaRouter::build_decision_plan(&state);
        assert_eq!(plan.domain, "git_operations");
        assert_eq!(plan.questions.len(), 3);
    }

    #[test]
    fn test_package_manager_routing() {
        let state = ExecutionState::new("pnpm dev", 1)
            .with_sanitized_lines(vec!["Command \"dev\" not found".into()]);
        let plan = CriteriaRouter::build_decision_plan(&state);
        assert_eq!(plan.domain, "package_managers");
    }

    #[test]
    fn test_system_cli_routing() {
        let state = ExecutionState::new("docker ps", 1)
            .with_sanitized_lines(vec!["Cannot connect to Docker daemon".into()]);
        let plan = CriteriaRouter::build_decision_plan(&state);
        assert_eq!(plan.domain, "system_cli");
    }
}
