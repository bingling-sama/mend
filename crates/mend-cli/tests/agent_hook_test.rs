use mend_capture::{sanitize_stderr, EntityExtractor};
use mend_core::{
    ActionStrategy, AgentHookInput, AgentHookOutput, ExecutionState, HookDecision,
    RemediationSummary,
};
use mend_reify::{build_default_rule_registry, SafetyGate, TemplateRenderer};

#[test]
fn test_agent_hook_rtk_git_set_upstream() {
    let raw_stderr = b"fatal: The current branch feat-agent has no upstream branch.\nTo push the current branch and set the remote as upstream, use\n    git push --set-upstream origin feat-agent\n";
    let sanitized = sanitize_stderr(raw_stderr, 10);

    let extractor = EntityExtractor::new();
    let entities = extractor.extract("rtk git push", &sanitized);
    assert_eq!(entities.get("remote").map(|s| s.as_str()), Some("origin"));
    assert_eq!(
        entities.get("branch").map(|s| s.as_str()),
        Some("feat-agent")
    );

    let state = ExecutionState::new("rtk git push", 128)
        .with_sanitized_lines(sanitized)
        .with_entities(entities);

    assert_eq!(state.wrappers, vec!["rtk"]);
    assert_eq!(state.argv, vec!["git", "push"]);

    let registry = build_default_rule_registry();
    let cand = registry.evaluate(&state).expect("Rule should match");

    assert_eq!(
        cand.rendered_command,
        "rtk git push --set-upstream origin feat-agent"
    );
    assert!(cand.confidence >= 0.92);
    assert!(cand.destructive_risk <= 0.10);
    assert!(SafetyGate::verify(&cand.strategy, cand.destructive_risk, cand.confidence, true).is_ok());
}

#[test]
fn test_agent_hook_rtk_path_correction() {
    let state = ExecutionState::new("rtk gti status", 127);
    assert_eq!(state.wrappers, vec!["rtk"]);
    assert_eq!(state.argv, vec!["gti", "status"]);

    let mut entities = std::collections::HashMap::new();
    entities.insert("suggested_binary".to_string(), "git".to_string());
    let state_with_entity = state.with_entities(entities);

    let cand = TemplateRenderer::render(
        &ActionStrategy::PathCorrection,
        &state_with_entity,
        0.98,
        0.02,
        true,
    )
    .expect("Render should succeed");

    assert_eq!(cand.rendered_command, "rtk git status");
}

#[test]
fn test_agent_hook_multi_layer_wrappers() {
    let mut entities = std::collections::HashMap::new();
    entities.insert("remote".to_string(), "origin".to_string());
    entities.insert("branch".to_string(), "main".to_string());

    let state = ExecutionState::new("sudo rtk git push", 128).with_entities(entities);
    assert_eq!(state.wrappers, vec!["sudo", "rtk"]);
    assert_eq!(state.argv, vec!["git", "push"]);

    let cand = TemplateRenderer::render(
        &ActionStrategy::GitSetUpstream,
        &state,
        0.95,
        0.05,
        true,
    )
    .expect("Render should succeed");

    assert_eq!(
        cand.rendered_command,
        "sudo rtk git push --set-upstream origin main"
    );
}

#[test]
fn test_agent_hook_claude_code_json_protocol() {
    let claude_json = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {
            "command": "rtk cargo bulid"
        },
        "tool_result": {
            "exit_code": 101,
            "stderr": "error: no such subcommand: `bulid`, did you mean `build`?"
        }
    });

    let input: AgentHookInput = serde_json::from_value(claude_json).unwrap();
    assert_eq!(input.extract_command(), Some("rtk cargo bulid".to_string()));
    assert_eq!(input.extract_exit_code(), 101);

    let stderr = input.extract_stderr();
    let sanitized = sanitize_stderr(stderr.as_bytes(), 5);

    let extractor = EntityExtractor::new();
    let entities = extractor.extract(&input.extract_command().unwrap(), &sanitized);

    let state = ExecutionState::new(input.extract_command().unwrap(), input.extract_exit_code())
        .with_sanitized_lines(sanitized)
        .with_entities(entities);

    let registry = build_default_rule_registry();
    let cand = registry.evaluate(&state).expect("Cargo typo rule should match");

    assert_eq!(cand.rendered_command, "rtk cargo build");

    let output = AgentHookOutput {
        decision: HookDecision::Remediated,
        original_command: "rtk cargo bulid".to_string(),
        original_exit_code: 101,
        new_exit_code: Some(0),
        suggested_command: Some(cand.rendered_command.clone()),
        system_message: Some(format!("Auto-mended via: {}", cand.rendered_command)),
        updated_output: Some("[mend:auto-mended] Success".to_string()),
        remediation: Some(RemediationSummary {
            strategy: cand.strategy.as_str().to_string(),
            command: cand.rendered_command,
            confidence: cand.confidence,
            destructive_risk: cand.destructive_risk,
        }),
    };

    let serialized = serde_json::to_string(&output).unwrap();
    assert!(serialized.contains("\"decision\":\"remediated\""));
    assert!(serialized.contains("\"command\":\"rtk cargo build\""));
}
