use mend_core::{ActionStrategy, ExecutionState, FailureReason, QuestionSpec};
use mend_jev::CriteriaRouter;

#[test]
fn test_router_solution_space_all_options_valid() {
    let test_commands = vec![
        "git push",
        "npm test",
        "pnpm build",
        "yarn start",
        "bun run dev",
        "cargo build",
        "python main.py",
        "unknown-tool -v",
    ];

    for cmd in test_commands {
        let state = ExecutionState::new(cmd, 1);
        let plan = CriteriaRouter::build_decision_plan(&state);

        for question in plan.questions {
            match question {
                QuestionSpec::Choice { id, options, .. } => {
                    assert!(
                        !options.is_empty(),
                        "Options should not be empty for question {}",
                        id
                    );
                    if id == "failure_reason" {
                        for opt in &options {
                            let parsed: FailureReason = opt.parse().expect("Valid parse");
                            if opt != "UNKNOWN" {
                                assert_ne!(
                                    parsed,
                                    FailureReason::Unknown,
                                    "Option '{}' parsed as Unknown",
                                    opt
                                );
                            }
                        }
                    } else if id == "remediation_action" {
                        for opt in &options {
                            let parsed: ActionStrategy = opt.parse().expect("Valid parse");
                            if opt != "ABORT" {
                                assert_ne!(
                                    parsed,
                                    ActionStrategy::Abort,
                                    "Option '{}' parsed as Abort",
                                    opt
                                );
                            }
                        }
                    }
                }
                QuestionSpec::Noul { min, max, .. } => {
                    assert!(min <= max, "Noul min should be <= max");
                }
            }
        }
    }
}
