use heal_core::{ActionStrategy, ExecutionState, RemediationCandidate, RuleRegistry};
use heal_reify::{build_default_rule_registry, TemplateRenderer};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::sync::OnceLock;

static GLOBAL_RULES: OnceLock<RuleRegistry> = OnceLock::new();

pub struct FastPathEngine;

impl FastPathEngine {
    pub fn try_heal(state: &mut ExecutionState) -> Option<RemediationCandidate> {
        let registry = GLOBAL_RULES.get_or_init(build_default_rule_registry);

        // 1. Evaluate curated industrial rules (thefuck rule catalog)
        if let Some(cand) = registry.evaluate(state) {
            return Some(cand);
        }

        // 2. Binary Command typo correction (Levenshtein against $PATH binaries)
        let first_cmd_exists = state.argv.first().map(|cmd| {
            Self::command_exists_in_path(cmd)
        }).unwrap_or(false);

        let is_command_not_found = state.exit_code == 127
            || !first_cmd_exists
            || state
                .sanitized_lines
                .iter()
                .any(|l| l.contains("command not found") && !l.contains("Command \""));

        if is_command_not_found {
            if let Some(first_cmd) = state.argv.first().cloned() {
                if !first_cmd_exists {
                    if let Some(closest) = Self::find_closest_path_binary(&first_cmd, 2) {
                        state
                            .entities
                            .insert("suggested_binary".to_string(), closest);
                        if let Ok(cand) = TemplateRenderer::render(
                            &ActionStrategy::PathCorrection,
                            state,
                            0.98,
                            0.02,
                            true,
                        ) {
                            return Some(cand);
                        }
                    }
                }
            }
        }

        None
    }

    fn command_exists_in_path(cmd: &str) -> bool {
        let path_var = env::var("PATH").unwrap_or_default();
        for dir in env::split_paths(&path_var) {
            let p = dir.join(cmd);
            if p.is_file() {
                return true;
            }
        }
        false
    }

    pub fn find_closest_path_binary(target: &str, max_distance: usize) -> Option<String> {
        let path_var = env::var("PATH").unwrap_or_default();
        let mut candidates: Vec<(String, usize, usize, usize)> = Vec::new();
        let mut seen = HashSet::new();

        let target_chars: HashSet<char> = target.chars().collect();

        for dir in env::split_paths(&path_var) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let file_name = entry.file_name();
                    let name = file_name.to_string_lossy();
                    if name.starts_with('.') || !seen.insert(name.to_string()) {
                        continue;
                    }

                    let dist = strsim::levenshtein(target, &name);
                    if dist <= max_distance {
                        let len_diff = (name.len() as isize - target.len() as isize).unsigned_abs();
                        let char_overlap = name.chars().filter(|c| target_chars.contains(c)).count();
                        let prefix_match = if name.starts_with(&target[..1.min(target.len())]) {
                            1
                        } else {
                            0
                        };

                        candidates.push((name.to_string(), dist, len_diff, prefix_match * 10 + char_overlap));
                    }
                }
            }
        }

        candidates.sort_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| b.3.cmp(&a.3))
                .then_with(|| a.2.cmp(&b.2))
                .then_with(|| a.0.cmp(&b.0))
        });

        candidates.first().map(|(name, _, _, _)| name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_path_permission_denied() {
        let mut state = ExecutionState::new("touch /root/foo", 126);
        let cand = FastPathEngine::try_heal(&mut state);
        assert!(cand.is_some());
        assert_eq!(cand.unwrap().rendered_command, "sudo touch /root/foo");
    }

    #[test]
    fn test_fast_path_pnpm_missing_script() {
        let mut state = ExecutionState::new("pn dev", 1).with_sanitized_lines(vec![
            "[ERR_PNPM_RECURSIVE_EXEC_FIRST_FAIL] Command \"dev\" not found".into(),
        ]);
        let mut entities = std::collections::HashMap::new();
        entities.insert("package_script".to_string(), "dev".to_string());
        state = state.with_entities(entities);

        let cand = FastPathEngine::try_heal(&mut state);
        assert!(cand.is_some());
        assert_eq!(cand.unwrap().rendered_command, "pn run dev");
    }

    #[test]
    fn test_fast_path_command_not_found() {
        let mut state = ExecutionState::new("gti status", 127);
        let cand = FastPathEngine::try_heal(&mut state);
        if let Some(c) = cand {
            assert!(c.rendered_command.contains("status"));
        }
    }
}
