use mend_capture::{sanitize_stderr, EntityExtractor};
use mend_core::{ActionStrategy, ExecutionState};
use mend_jev::CriteriaRouter;
use mend_reify::{build_default_rule_registry, SafetyGate, TemplateRenderer};

#[test]
fn test_end_to_end_git_upstream_pipeline() {
    let raw_stderr = b"\x1b[31mfatal: The current branch feat-awesome has no upstream branch.\nTo push the current branch and set the remote as upstream, use\n    git push --set-upstream origin feat-awesome\x1b[0m\n";

    let sanitized = sanitize_stderr(raw_stderr, 10);
    assert_eq!(sanitized.len(), 3);

    let extractor = EntityExtractor::new();
    let entities = extractor.extract("git push", &sanitized);
    assert_eq!(entities.get("remote").map(|s| s.as_str()), Some("origin"));
    assert_eq!(
        entities.get("branch").map(|s| s.as_str()),
        Some("feat-awesome")
    );

    let state = ExecutionState::new("git push", 128)
        .with_sanitized_lines(sanitized)
        .with_entities(entities);

    let plan = CriteriaRouter::build_decision_plan(&state);
    assert_eq!(plan.domain, "git_operations");

    let confidence = 0.96;
    let destructive_risk = 0.05;

    assert!(SafetyGate::verify(
        &ActionStrategy::GitSetUpstream,
        destructive_risk,
        confidence,
        true
    )
    .is_ok());

    let candidate = TemplateRenderer::render(
        &ActionStrategy::GitSetUpstream,
        &state,
        confidence,
        destructive_risk,
        false,
    )
    .expect("Reification succeeds");

    assert_eq!(
        candidate.rendered_command,
        "git push --set-upstream origin feat-awesome"
    );
}

#[test]
fn test_end_to_end_thefuck_rules_catalog() {
    let registry = build_default_rule_registry();
    let extractor = EntityExtractor::new();

    // 1. PNPM missing run rule
    let pnpm_err =
        vec!["[ERR_PNPM_RECURSIVE_EXEC_FIRST_FAIL] Command \"dev\" not found".to_string()];
    let entities = extractor.extract("pn dev", &pnpm_err);
    let state = ExecutionState::new("pn dev", 1)
        .with_sanitized_lines(pnpm_err)
        .with_entities(entities);
    let cand = registry.evaluate(&state).expect("PNPM rule matches");
    assert_eq!(cand.rendered_command, "pn run dev");

    // 2. Git Subcommand typo
    let git_err =
        vec!["git: 'brnach' is not a git command. The most similar command is branch".to_string()];
    let entities = extractor.extract("git brnach", &git_err);
    let state = ExecutionState::new("git brnach", 1)
        .with_sanitized_lines(git_err)
        .with_entities(entities);
    let cand = registry
        .evaluate(&state)
        .expect("Git subcommand typo matches");
    assert_eq!(cand.rendered_command, "git branch");

    // 3. Cargo Subcommand typo
    let cargo_err = vec!["error: no such subcommand: `bulid`, did you mean `build`?".to_string()];
    let entities = extractor.extract("cargo bulid", &cargo_err);
    let state = ExecutionState::new("cargo bulid", 101)
        .with_sanitized_lines(cargo_err)
        .with_entities(entities);
    let cand = registry
        .evaluate(&state)
        .expect("Cargo subcommand typo matches");
    assert_eq!(cand.rendered_command, "cargo build");

    // 4. Python module missing
    let py_err = vec!["ModuleNotFoundError: No module named 'fastapi'".to_string()];
    let entities = extractor.extract("python main.py", &py_err);
    let state = ExecutionState::new("python main.py", 1)
        .with_sanitized_lines(py_err)
        .with_entities(entities);
    let cand = registry
        .evaluate(&state)
        .expect("Python module missing matches");
    assert_eq!(cand.rendered_command, "pip install fastapi");
}

#[test]
fn test_end_to_end_agent_safety_circuit_breaker() {
    let _state = ExecutionState::new("rm -rf /tmp/data", 1);
    let confidence = 0.95;
    let high_destructive_risk = 0.35;

    let res = SafetyGate::verify(
        &ActionStrategy::Abort,
        high_destructive_risk,
        confidence,
        true,
    );
    assert!(res.is_err());

    let res2 = SafetyGate::verify(
        &ActionStrategy::PrependSudo,
        high_destructive_risk,
        confidence,
        true,
    );
    assert!(res2.is_err());
}

#[test]
fn test_end_to_end_policy_and_telemetry_solution_space_pruning() {
    use mend_core::QuestionSpec;

    let state = ExecutionState::new("docker ps", 1)
        .with_sanitized_lines(vec!["Cannot connect to the Docker daemon".into()]);

    let base_plan = CriteriaRouter::build_decision_plan(&state);
    if let QuestionSpec::Choice { id, options, .. } = &base_plan.questions[1] {
        assert_eq!(id, "remediation_action");
        assert!(options.contains(&"DOCKER_START_DAEMON".to_string()));
        assert!(options.contains(&"PREPEND_SUDO".to_string()));
    } else {
        panic!("Expected Choice");
    }

    let router_denied = CriteriaRouter::with_policies(vec![ActionStrategy::PrependSudo], None);
    let plan_denied = router_denied.plan(&state);
    if let QuestionSpec::Choice { id, options, .. } = &plan_denied.questions[1] {
        assert_eq!(id, "remediation_action");
        assert!(options.contains(&"DOCKER_START_DAEMON".to_string()));
        assert!(!options.contains(&"PREPEND_SUDO".to_string()));
        assert!(options.contains(&"ABORT".to_string()));
    } else {
        panic!("Expected Choice");
    }

    let router_allowed =
        CriteriaRouter::with_policies(vec![], Some(vec![ActionStrategy::DockerStartDaemon]));
    let plan_allowed = router_allowed.plan(&state);
    if let QuestionSpec::Choice { id, options, .. } = &plan_allowed.questions[1] {
        assert_eq!(id, "remediation_action");
        assert_eq!(
            options,
            &vec!["DOCKER_START_DAEMON".to_string(), "ABORT".to_string()]
        );
    } else {
        panic!("Expected Choice");
    }
}
