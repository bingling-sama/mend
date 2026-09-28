use crate::provider::{
    CargoCriteriaProvider, DomainCriteriaProvider, GitCriteriaProvider,
    PackageManagerCriteriaProvider, PythonCriteriaProvider, SystemCliCriteriaProvider,
};
use mend_core::{ActionStrategy, DecisionPlan, ExecutionState, QuestionSpec};

pub struct CriteriaRouter {
    providers: Vec<Box<dyn DomainCriteriaProvider>>,
    denylist: Vec<ActionStrategy>,
    allowlist: Option<Vec<ActionStrategy>>,
}

impl Default for CriteriaRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl CriteriaRouter {
    pub fn new() -> Self {
        Self {
            providers: vec![
                Box::new(GitCriteriaProvider),
                Box::new(PackageManagerCriteriaProvider),
                Box::new(CargoCriteriaProvider),
                Box::new(PythonCriteriaProvider),
                Box::new(SystemCliCriteriaProvider),
            ],
            denylist: Vec::new(),
            allowlist: None,
        }
    }

    pub fn with_policies(
        denylist: Vec<ActionStrategy>,
        allowlist: Option<Vec<ActionStrategy>>,
    ) -> Self {
        let mut router = Self::new();
        router.denylist = denylist;
        router.allowlist = allowlist;
        router
    }

    pub fn register_provider(&mut self, provider: Box<dyn DomainCriteriaProvider>) {
        let insert_idx = self.providers.len().saturating_sub(1);
        self.providers.insert(insert_idx, provider);
    }

    pub fn build_decision_plan(state: &ExecutionState) -> DecisionPlan {
        Self::default().plan(state)
    }

    pub fn plan(&self, state: &ExecutionState) -> DecisionPlan {
        let provider = self
            .providers
            .iter()
            .find(|p| p.matches(state))
            .expect("SystemCliCriteriaProvider matches all states as fallback");

        let domain = provider.domain_name().to_string();
        let reasons = provider.available_reasons(state);
        let mut actions = provider.available_actions(state);

        // Apply policy filtering
        if let Some(allowlist) = &self.allowlist {
            actions.retain(|a| *a == ActionStrategy::Abort || allowlist.contains(a));
        }
        if !self.denylist.is_empty() {
            actions.retain(|a| *a == ActionStrategy::Abort || !self.denylist.contains(a));
        }

        // Always guarantee Abort is present
        if !actions.contains(&ActionStrategy::Abort) {
            actions.push(ActionStrategy::Abort);
        }

        let failure_reasons: Vec<String> = reasons
            .into_iter()
            .map(|r| r.as_str().to_string())
            .collect();
        let remediation_actions: Vec<String> = actions
            .into_iter()
            .map(|a| a.as_str().to_string())
            .collect();

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
    use std::collections::HashMap;

    #[test]
    fn test_git_routing_and_pruning() {
        let state = ExecutionState::new("git push origin master", 1)
            .with_sanitized_lines(vec!["fatal: current branch has no upstream".into()]);
        let plan = CriteriaRouter::build_decision_plan(&state);
        assert_eq!(plan.domain, "git_operations");
        assert_eq!(plan.questions.len(), 3);

        if let QuestionSpec::Choice { id, options, .. } = &plan.questions[1] {
            assert_eq!(id, "remediation_action");
            assert!(options.contains(&"GIT_SET_UPSTREAM".to_string()));
            assert!(options.contains(&"ABORT".to_string()));
            // Pruned non-relevant git actions
            assert!(!options.contains(&"GIT_PULL_REBASE".to_string()));
        } else {
            panic!("Expected Choice question");
        }
    }

    #[test]
    fn test_package_manager_routing_and_pruning() {
        let state = ExecutionState::new("pnpm dev", 1)
            .with_sanitized_lines(vec!["Command \"dev\" not found".into()]);
        let plan = CriteriaRouter::build_decision_plan(&state);
        assert_eq!(plan.domain, "package_managers");

        if let QuestionSpec::Choice { id, options, .. } = &plan.questions[1] {
            assert_eq!(id, "remediation_action");
            assert!(options.contains(&"PNPM_RUN_SCRIPT".to_string()));
            assert!(options.contains(&"ABORT".to_string()));
            // Specific tool pruning: npm/yarn/bun should not appear for pnpm
            assert!(!options.contains(&"NPM_RUN_SCRIPT".to_string()));
            assert!(!options.contains(&"YARN_RUN_SCRIPT".to_string()));
            assert!(!options.contains(&"BUN_RUN_SCRIPT".to_string()));
        } else {
            panic!("Expected Choice question");
        }
    }

    #[test]
    fn test_system_cli_exit_126_pruning() {
        let state = ExecutionState::new("/usr/local/bin/my-script", 126)
            .with_sanitized_lines(vec!["bash: Permission denied".into()]);
        let plan = CriteriaRouter::build_decision_plan(&state);
        assert_eq!(plan.domain, "system_cli");

        if let QuestionSpec::Choice { id, options, .. } = &plan.questions[1] {
            assert_eq!(id, "remediation_action");
            assert!(options.contains(&"PREPEND_SUDO".to_string()));
            assert!(options.contains(&"CHMOD_EXECUTABLE".to_string()));
            assert!(options.contains(&"ABORT".to_string()));
            // Irrelevant actions pruned
            assert!(!options.contains(&"DOCKER_START_DAEMON".to_string()));
            assert!(!options.contains(&"BREW_INSTALL_PACKAGE".to_string()));
        } else {
            panic!("Expected Choice question");
        }
    }

    #[test]
    fn test_system_cli_docker_pruning() {
        let state = ExecutionState::new("docker ps", 1)
            .with_sanitized_lines(vec!["Cannot connect to the Docker daemon".into()]);
        let plan = CriteriaRouter::build_decision_plan(&state);
        assert_eq!(plan.domain, "system_cli");

        if let QuestionSpec::Choice { id, options, .. } = &plan.questions[1] {
            assert_eq!(id, "remediation_action");
            assert!(options.contains(&"DOCKER_START_DAEMON".to_string()));
            assert!(options.contains(&"PREPEND_SUDO".to_string()));
            assert!(options.contains(&"ABORT".to_string()));
            assert!(!options.contains(&"MAKE_DIRECTORY".to_string()));
        } else {
            panic!("Expected Choice question");
        }
    }

    #[test]
    fn test_policy_filtering_denylist() {
        let state = ExecutionState::new("docker ps", 1)
            .with_sanitized_lines(vec!["Cannot connect to the Docker daemon".into()]);
        let router = CriteriaRouter::with_policies(vec![ActionStrategy::PrependSudo], None);
        let plan = router.plan(&state);

        if let QuestionSpec::Choice { id, options, .. } = &plan.questions[1] {
            assert_eq!(id, "remediation_action");
            assert!(!options.contains(&"PREPEND_SUDO".to_string()));
            assert!(options.contains(&"DOCKER_START_DAEMON".to_string()));
            assert!(options.contains(&"ABORT".to_string()));
        }
    }

    #[test]
    fn test_policy_filtering_allowlist() {
        let state = ExecutionState::new("docker ps", 1)
            .with_sanitized_lines(vec!["Cannot connect to the Docker daemon".into()]);
        let router =
            CriteriaRouter::with_policies(vec![], Some(vec![ActionStrategy::DockerStartDaemon]));
        let plan = router.plan(&state);

        if let QuestionSpec::Choice { id, options, .. } = &plan.questions[1] {
            assert_eq!(id, "remediation_action");
            assert_eq!(
                options,
                &vec!["DOCKER_START_DAEMON".to_string(), "ABORT".to_string()]
            );
        }
    }

    #[test]
    fn test_entity_gated_pruning_missing_subcommand() {
        let state = ExecutionState::new("cargo unknowntrick", 1)
            .with_sanitized_lines(vec!["error: no such subcommand `unknowntrick`".into()]);
        let plan = CriteriaRouter::build_decision_plan(&state);
        if let QuestionSpec::Choice { id, options, .. } = &plan.questions[1] {
            assert_eq!(id, "remediation_action");
            // Since no suggested_subcommand entity is present, SubcommandCorrection is pruned
            assert!(!options.contains(&"SUBCOMMAND_CORRECTION".to_string()));
        }

        let mut entities = HashMap::new();
        entities.insert("suggested_subcommand".to_string(), "check".to_string());
        let state_with_entity = state.with_entities(entities);
        let plan_with_entity = CriteriaRouter::build_decision_plan(&state_with_entity);
        if let QuestionSpec::Choice { id, options, .. } = &plan_with_entity.questions[1] {
            assert_eq!(id, "remediation_action");
            assert!(options.contains(&"SUBCOMMAND_CORRECTION".to_string()));
        }
    }
}
