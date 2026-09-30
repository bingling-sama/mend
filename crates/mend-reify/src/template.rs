use mend_core::{ActionStrategy, ExecutionState, RemediationCandidate, SafetyError};

pub struct TemplateRenderer;

impl TemplateRenderer {
    pub fn render(
        strategy: &ActionStrategy,
        state: &ExecutionState,
        confidence: f64,
        destructive_risk: f64,
        is_fast_path: bool,
    ) -> Result<RemediationCandidate, SafetyError> {
        let (rendered_command, explanation) = match strategy {
            ActionStrategy::PrependSudo => {
                let cmd = if state.command.starts_with("sudo ") {
                    state.command.clone()
                } else {
                    format!("sudo {}", state.command)
                };
                (
                    cmd,
                    "Retry command with elevated sudo privileges".to_string(),
                )
            }
            ActionStrategy::GitSetUpstream => {
                let remote = state
                    .entities
                    .get("remote")
                    .cloned()
                    .unwrap_or_else(|| "origin".to_string());
                let branch = state
                    .entities
                    .get("branch")
                    .cloned()
                    .or_else(|| Self::guess_git_branch(state))
                    .ok_or_else(|| SafetyError::MissingEntity("branch".to_string()))?;

                (
                    format!("git push --set-upstream {} {}", remote, branch),
                    format!("Set upstream tracking to {}/{}", remote, branch),
                )
            }
            ActionStrategy::GitPullRebase => (
                "git pull --rebase".to_string(),
                "Rebase local commits onto remote changes before pushing".to_string(),
            ),
            ActionStrategy::GitStashPop => (
                "git stash && git pull --rebase && git stash pop".to_string(),
                "Stash dirty changes, rebase from upstream, and pop stash".to_string(),
            ),
            ActionStrategy::GitCheckoutBranch => {
                let branch = state
                    .entities
                    .get("branch")
                    .cloned()
                    .ok_or_else(|| SafetyError::MissingEntity("branch".to_string()))?;
                (
                    format!("git checkout {}", branch),
                    format!("Switch to branch {}", branch),
                )
            }
            ActionStrategy::GitBranchCreate => {
                let branch = state
                    .entities
                    .get("branch")
                    .cloned()
                    .unwrap_or_else(|| "main".to_string());
                (
                    format!("git checkout -b {}", branch),
                    format!("Create and switch to new branch {}", branch),
                )
            }
            ActionStrategy::PnpmRunScript => {
                let prog = state.argv.first().map(|s| s.as_str()).unwrap_or("pnpm");
                let script = state
                    .entities
                    .get("package_script")
                    .cloned()
                    .or_else(|| state.argv.get(1).cloned())
                    .unwrap_or_else(|| "dev".to_string());

                let base_cmd = if prog == "pn" { "pn" } else { "pnpm" };
                (
                    format!("{} run {}", base_cmd, script),
                    format!("Run script '{}' using {} run", script, base_cmd),
                )
            }
            ActionStrategy::NpmRunScript => {
                let script = state
                    .entities
                    .get("package_script")
                    .cloned()
                    .or_else(|| state.argv.get(1).cloned())
                    .unwrap_or_else(|| "dev".to_string());
                (
                    format!("npm run {}", script),
                    format!("Run script '{}' using npm run", script),
                )
            }
            ActionStrategy::YarnRunScript => {
                let script = state
                    .entities
                    .get("package_script")
                    .cloned()
                    .or_else(|| state.argv.get(1).cloned())
                    .unwrap_or_else(|| "dev".to_string());
                (
                    format!("yarn run {}", script),
                    format!("Run script '{}' using yarn run", script),
                )
            }
            ActionStrategy::BunRunScript => {
                let script = state
                    .entities
                    .get("package_script")
                    .cloned()
                    .or_else(|| state.argv.get(1).cloned())
                    .unwrap_or_else(|| "dev".to_string());
                (
                    format!("bun run {}", script),
                    format!("Run script '{}' using bun run", script),
                )
            }
            ActionStrategy::PipInstallPackage => {
                let module = state
                    .entities
                    .get("python_module")
                    .cloned()
                    .ok_or_else(|| SafetyError::MissingEntity("python_module".to_string()))?;
                (
                    format!("pip install {}", module),
                    format!("Install missing Python module '{}' via pip", module),
                )
            }
            ActionStrategy::CargoAddDependency => {
                let pkg = state
                    .entities
                    .get("package")
                    .cloned()
                    .ok_or_else(|| SafetyError::MissingEntity("package".to_string()))?;
                (
                    format!("cargo add {}", pkg),
                    format!("Add dependency '{}' to Cargo.toml", pkg),
                )
            }
            ActionStrategy::SubcommandCorrection => {
                let suggested = state
                    .entities
                    .get("suggested_subcommand")
                    .cloned()
                    .ok_or_else(|| {
                        SafetyError::MissingEntity("suggested_subcommand".to_string())
                    })?;

                let mut tokens = state.argv.clone();
                if tokens.len() > 1 {
                    tokens[1] = suggested;
                }
                (tokens.join(" "), "Correct mistyped subcommand".to_string())
            }
            ActionStrategy::PathCorrection => {
                let corrected = state
                    .entities
                    .get("suggested_binary")
                    .cloned()
                    .ok_or_else(|| SafetyError::MissingEntity("suggested_binary".to_string()))?;

                if state.argv.first().map(|s| s.as_str()) == Some(&corrected) {
                    return Err(SafetyError::Aborted(ActionStrategy::PathCorrection));
                }

                let mut tokens = state.argv.clone();
                if !tokens.is_empty() {
                    tokens[0] = corrected;
                }
                (
                    tokens.join(" "),
                    "Correct command spelling typo from PATH lookup".to_string(),
                )
            }
            ActionStrategy::AptInstallPackage => {
                let pkg = state
                    .entities
                    .get("package")
                    .cloned()
                    .or_else(|| state.entities.get("missing_command").cloned())
                    .ok_or_else(|| SafetyError::MissingEntity("package".to_string()))?;
                (
                    format!("sudo apt update && sudo apt install -y {}", pkg),
                    format!("Install missing package {}", pkg),
                )
            }
            ActionStrategy::BrewInstallPackage => {
                let pkg = state
                    .entities
                    .get("package")
                    .cloned()
                    .or_else(|| state.entities.get("missing_command").cloned())
                    .ok_or_else(|| SafetyError::MissingEntity("package".to_string()))?;
                (
                    format!("brew install {}", pkg),
                    format!("Install package {} via Homebrew", pkg),
                )
            }
            ActionStrategy::CargoInstallPackage => {
                let pkg = state
                    .entities
                    .get("package")
                    .cloned()
                    .or_else(|| state.entities.get("missing_command").cloned())
                    .ok_or_else(|| SafetyError::MissingEntity("package".to_string()))?;
                (
                    format!("cargo install {}", pkg),
                    format!("Install package {} via Cargo", pkg),
                )
            }
            ActionStrategy::MakeDirectory => {
                let path = state
                    .entities
                    .get("path")
                    .cloned()
                    .unwrap_or_else(|| "dir".to_string());
                (
                    format!("mkdir -p {}", path),
                    format!("Create missing directory {}", path),
                )
            }
            ActionStrategy::ChmodExecutable => {
                let path = state
                    .entities
                    .get("path")
                    .cloned()
                    .or_else(|| state.argv.first().cloned())
                    .unwrap_or_else(|| "file".to_string());
                (
                    format!("chmod +x {}", path),
                    format!("Grant executable permission on {}", path),
                )
            }
            ActionStrategy::DockerStartDaemon => {
                let cmd = if cfg!(target_os = "macos") {
                    "open -a Docker".to_string()
                } else {
                    "sudo systemctl start docker".to_string()
                };
                (cmd, "Start the Docker daemon".to_string())
            }
            ActionStrategy::Abort => {
                return Err(SafetyError::Aborted(ActionStrategy::Abort));
            }
        };

        let explanation = if !is_fast_path {
            format!("Jev recommends you: {}", explanation)
        } else {
            explanation
        };

        Ok(RemediationCandidate {
            strategy: strategy.clone(),
            rendered_command,
            explanation,
            confidence,
            destructive_risk,
            is_fast_path,
        })
    }

    fn guess_git_branch(state: &ExecutionState) -> Option<String> {
        let text = format!("{}\n{}", state.command, state.sanitized_lines.join("\n"));
        for word in text.split_whitespace() {
            if word.starts_with("feat")
                || word.starts_with("fix")
                || word == "main"
                || word == "master"
            {
                return Some(word.trim_matches('\'').trim_matches('"').to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_render_git_set_upstream() {
        let mut entities = HashMap::new();
        entities.insert("remote".to_string(), "origin".to_string());
        entities.insert("branch".to_string(), "feature-x".to_string());

        let state = ExecutionState::new("git push", 1).with_entities(entities);
        let cand =
            TemplateRenderer::render(&ActionStrategy::GitSetUpstream, &state, 0.95, 0.05, false)
                .expect("Render should succeed");

        assert_eq!(
            cand.rendered_command,
            "git push --set-upstream origin feature-x"
        );
        assert_eq!(cand.confidence, 0.95);
        assert_eq!(
            cand.explanation,
            "Jev recommends you: Set upstream tracking to origin/feature-x"
        );
    }

    #[test]
    fn test_render_pnpm_run_script() {
        let mut entities = HashMap::new();
        entities.insert("package_script".to_string(), "dev".to_string());
        let state = ExecutionState::new("pn dev", 1).with_entities(entities);
        let cand =
            TemplateRenderer::render(&ActionStrategy::PnpmRunScript, &state, 0.98, 0.01, true)
                .expect("Render should succeed");
        assert_eq!(cand.rendered_command, "pn run dev");
    }

    #[test]
    fn test_render_prepend_sudo() {
        let state = ExecutionState::new("apt update", 126);
        let cand = TemplateRenderer::render(&ActionStrategy::PrependSudo, &state, 1.0, 0.05, true)
            .expect("Render should succeed");
        assert_eq!(cand.rendered_command, "sudo apt update");
    }

    #[test]
    fn test_render_path_correction() {
        let mut entities = HashMap::new();
        entities.insert("suggested_binary".to_string(), "git".to_string());
        let state = ExecutionState::new("gti status", 127).with_entities(entities);
        let cand =
            TemplateRenderer::render(&ActionStrategy::PathCorrection, &state, 0.99, 0.01, true)
                .expect("Render should succeed");
        assert_eq!(cand.rendered_command, "git status");
    }
}
