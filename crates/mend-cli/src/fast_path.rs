use mend_core::{ActionStrategy, ExecutionState, RemediationCandidate, RuleRegistry};
use mend_reify::{build_default_rule_registry, TemplateRenderer};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::sync::OnceLock;

static GLOBAL_RULES: OnceLock<RuleRegistry> = OnceLock::new();

pub const WELL_KNOWN_COMMANDS: &[&str] = &[
    "docker", "docker-compose", "podman", "kubectl", "helm", "minikube", "kind", "k9s", "vagrant", "container",
    "git", "gh", "glab", "svn", "hg",
    "claude", "opencode", "codex", "gemini",
    "cargo", "rustc", "rustup", "rustfmt", "clippy",
    "node", "nodejs", "npm", "npx", "pnpm", "pnpx", "yarn", "bun", "bunx", "deno",
    "vite", "next", "turbo", "webpack", "tsc", "eslint", "prettier",
    "python", "python3", "pip", "pip3", "poetry", "uv", "pytest", "conda", "pdm", "pipenv", "tox",
    "go", "gofmt",
    "make", "cmake", "ninja", "gcc", "g++", "clang", "clang++", "gdb", "lldb",
    "java", "javac", "mvn", "gradle", "kotlinc", "scala",
    "terraform", "tofu", "ansible", "aws", "gcloud", "az", "pulumi",
    "curl", "wget", "ssh", "scp", "rsync", "tar", "unzip", "gzip", "zip",
    "grep", "sed", "awk", "find", "cat", "chmod", "chown", "sudo", "su",
    "tmux", "screen", "htop", "btop", "top", "vim", "nvim", "nano", "code",
    "zsh", "bash", "fish", "sh",
    "brew", "apt", "apt-get", "yum", "dnf", "pacman", "apk", "zypper",
];

pub struct FastPathEngine;

impl FastPathEngine {
    pub fn try_mend(state: &mut ExecutionState) -> Option<RemediationCandidate> {
        let registry = GLOBAL_RULES.get_or_init(build_default_rule_registry);

        if let Some(cand) = registry.evaluate(state) {
            return Some(cand);
        }

        let first_cmd_exists = state
            .argv
            .first()
            .map(|cmd| Self::command_exists_in_path(cmd))
            .unwrap_or(false);

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
        let effective_max = if target.len() <= 2 {
            1.min(max_distance)
        } else {
            max_distance
        };

        let path_var = env::var("PATH").unwrap_or_default();
        let mut candidates: Vec<(String, usize, bool, usize, usize)> = Vec::new();
        let mut seen = HashSet::new();

        let target_chars: HashSet<char> = target.chars().collect();

        let mut search_dirs: Vec<std::path::PathBuf> = env::split_paths(&path_var).collect();
        let common_dirs = [
            "/usr/local/bin",
            "/opt/homebrew/bin",
            "/usr/bin",
            "/bin",
            "/usr/sbin",
            "/sbin",
            "/opt/homebrew/sbin",
        ];
        for d in common_dirs {
            let p = std::path::PathBuf::from(d);
            if !search_dirs.contains(&p) && p.is_dir() {
                search_dirs.push(p);
            }
        }
        if let Ok(home) = env::var("HOME") {
            let user_dirs = [
                format!("{}/.cargo/bin", home),
                format!("{}/.local/bin", home),
                format!("{}/.docker/bin", home),
                "/Applications/Docker.app/Contents/Resources/bin".to_string(),
            ];
            for ud in &user_dirs {
                let p = std::path::PathBuf::from(ud);
                if !search_dirs.contains(&p) && p.is_dir() {
                    search_dirs.push(p);
                }
            }
        }

        for dir in search_dirs {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let file_name = entry.file_name();
                    let name = file_name.to_string_lossy();
                    if name.starts_with('.') || !seen.insert(name.to_string()) {
                        continue;
                    }

                    let dist = strsim::levenshtein(target, &name);
                    if dist <= effective_max {
                        let len_diff = (name.len() as isize - target.len() as isize).unsigned_abs();
                        let char_overlap =
                            name.chars().filter(|c| target_chars.contains(c)).count();
                        let prefix_match = if name.starts_with(&target[..1.min(target.len())]) {
                            1
                        } else {
                            0
                        };

                        candidates.push((
                            name.to_string(),
                            dist,
                            true,
                            prefix_match * 10 + char_overlap,
                            len_diff,
                        ));
                    }
                }
            }
        }

        for &well_known in WELL_KNOWN_COMMANDS {
            if !seen.insert(well_known.to_string()) {
                continue;
            }

            let dist = strsim::levenshtein(target, well_known);
            if dist <= effective_max {
                let len_diff = (well_known.len() as isize - target.len() as isize).unsigned_abs();
                let char_overlap =
                    well_known.chars().filter(|c| target_chars.contains(c)).count();
                let prefix_match = if well_known.starts_with(&target[..1.min(target.len())]) {
                    1
                } else {
                    0
                };

                candidates.push((
                    well_known.to_string(),
                    dist,
                    false,
                    prefix_match * 10 + char_overlap,
                    len_diff,
                ));
            }
        }

        candidates.sort_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| b.2.cmp(&a.2))
                .then_with(|| b.3.cmp(&a.3))
                .then_with(|| a.4.cmp(&b.4))
                .then_with(|| a.0.cmp(&b.0))
        });

        candidates.first().map(|(name, _, _, _, _)| name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_path_permission_denied() {
        let mut state = ExecutionState::new("touch /root/foo", 126);
        let cand = FastPathEngine::try_mend(&mut state);
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

        let cand = FastPathEngine::try_mend(&mut state);
        assert!(cand.is_some());
        assert_eq!(cand.unwrap().rendered_command, "pn run dev");
    }

    #[test]
    fn test_fast_path_command_not_found() {
        let mut state = ExecutionState::new("gti status", 127);
        let cand = FastPathEngine::try_mend(&mut state);
        if let Some(c) = cand {
            assert!(c.rendered_command.contains("status"));
        }
    }

    #[test]
    fn test_fast_path_dcoker_typo() {
        let mut state = ExecutionState::new("dcoker ps", 127);
        let cand = FastPathEngine::try_mend(&mut state);
        assert!(cand.is_some());
        assert_eq!(cand.unwrap().rendered_command, "docker ps");
    }
}
