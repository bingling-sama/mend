use regex::Regex;
use std::collections::HashMap;

/// Entity Extractor identifies dynamic CLI arguments and errors
/// like git branches, remotes, package names, files, subcommands, and scripts.
#[derive(Debug, Clone)]
pub struct EntityExtractor {
    git_upstream_re: Regex,
    git_branch_re: Regex,
    git_unknown_subcmd_re: Regex,
    apt_package_re: Regex,
    brew_package_re: Regex,
    cargo_package_re: Regex,
    cargo_did_you_mean_re: Regex,
    pnpm_command_not_found_re: Regex,
    npm_missing_script_re: Regex,
    docker_unknown_cmd_re: Regex,
    kubectl_unknown_cmd_re: Regex,
    python_mod_not_found_re: Regex,
    no_such_file_re: Regex,
    command_not_found_re: Regex,
}

impl Default for EntityExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityExtractor {
    pub fn new() -> Self {
        Self {
            // E.g.: "git push --set-upstream origin my-feature"
            git_upstream_re: Regex::new(
                r"git push --set-upstream\s+(?P<remote>[^\s]+)\s+(?P<branch>[^\s]+)",
            )
            .unwrap(),
            // E.g.: "The current branch feature/xyz has no upstream branch."
            git_branch_re: Regex::new(
                r"branch '?(?P<branch>[a-zA-Z0-9_\-\./]+)'? has no upstream branch",
            )
            .unwrap(),
            // E.g.: "git: 'brnach' is not a git command. See 'git --help'."
            // or "The most similar command is: branch"
            git_unknown_subcmd_re: Regex::new(
                r"(?:is not a git command|The most similar command is)\s*:?\s*'?(?P<subcmd>[a-zA-Z0-9_\-]+)'?",
            )
            .unwrap(),
            // E.g.: "Package 'nginx' has no installation candidate" or "apt-get install <pkg>"
            apt_package_re: Regex::new(
                r"(?:Package\s+'(?P<pkg>[a-zA-Z0-9_\-]+)'|sudo apt install\s+(?P<pkg2>[a-zA-Z0-9_\-]+))",
            )
            .unwrap(),
            // E.g.: "brew install <pkg>"
            brew_package_re: Regex::new(r"brew install\s+(?P<pkg>[a-zA-Z0-9_\-]+)").unwrap(),
            // E.g.: "cargo install <pkg>"
            cargo_package_re: Regex::new(r"cargo install\s+(?P<pkg>[a-zA-Z0-9_\-]+)").unwrap(),
            // E.g.: "error: no such subcommand: `bulid`" ... "did you mean `build`?"
            cargo_did_you_mean_re: Regex::new(
                r"did you mean\s*`(?P<subcmd>[a-zA-Z0-9_\-]+)`\?",
            )
            .unwrap(),
            // E.g.: "[ERR_PNPM_RECURSIVE_EXEC_FIRST_FAIL] Command "dev" not found" or "Command "build" not found"
            pnpm_command_not_found_re: Regex::new(
                r#"(?:\[ERR_PNPM_[^\]]+\]\s*)?Command\s*"(?P<script>[^"]+)"\s*not found"#,
            )
            .unwrap(),
            // E.g.: "npm error Missing script: "dev"" or "npm ERR! Missing script: dev"
            npm_missing_script_re: Regex::new(
                r#"Missing script:\s*"?`?(?P<script>[a-zA-Z0-9_\-:]+)"?`?"#,
            )
            .unwrap(),
            // E.g.: "docker: 'psa' is not a docker command."
            docker_unknown_cmd_re: Regex::new(
                r"docker:\s*'?(?P<subcmd>[a-zA-Z0-9_\-]+)'?\s*is not a docker command",
            )
            .unwrap(),
            // E.g.: "error: unknown command "get pods""
            kubectl_unknown_cmd_re: Regex::new(
                r#"error: unknown command\s*"(?P<subcmd>[^"]+)""#,
            )
            .unwrap(),
            // E.g.: "ModuleNotFoundError: No module named 'requests'"
            python_mod_not_found_re: Regex::new(
                r#"ModuleNotFoundError:\s*No module named\s*['"](?P<module>[a-zA-Z0-9_\-]+)['"]"#,
            )
            .unwrap(),
            // E.g.: "No such file or directory: /path/to/something" or "cat: foo.txt: No such file or directory"
            no_such_file_re: Regex::new(
                r"(?:No such file or directory:?\s*|:\s*No such file or directory\s*)(?P<path>[^\s:]+)?",
            )
            .unwrap(),
            // E.g.: "zsh: command not found: sl" or "bash: gti: command not found"
            command_not_found_re: Regex::new(
                r"(?:command not found:\s*|:\s*command not found:\s*|:\s*command not found\b)(?P<cmd>[^\s:]+)?",
            )
            .unwrap(),
        }
    }

    pub fn extract(&self, command: &str, stderr_lines: &[String]) -> HashMap<String, String> {
        let mut entities = HashMap::new();
        let full_text = format!("{}\n{}", command, stderr_lines.join("\n"));

        // 1. Original command and first token
        if let Some(cmd0) = command.split_whitespace().next() {
            entities.insert("original_command".to_string(), cmd0.to_string());
        }

        // 2. Git Upstream / Branch
        if let Some(caps) = self.git_upstream_re.captures(&full_text) {
            if let Some(r) = caps.name("remote") {
                entities.insert("remote".to_string(), r.as_str().to_string());
            }
            if let Some(b) = caps.name("branch") {
                entities.insert("branch".to_string(), b.as_str().to_string());
            }
        }
        if !entities.contains_key("branch") {
            if let Some(caps) = self.git_branch_re.captures(&full_text) {
                if let Some(b) = caps.name("branch") {
                    entities.insert("branch".to_string(), b.as_str().to_string());
                    entities.insert("remote".to_string(), "origin".to_string());
                }
            }
        }
        if let Some(caps) = self.git_unknown_subcmd_re.captures(&full_text) {
            if let Some(s) = caps.name("subcmd") {
                entities.insert("suggested_subcommand".to_string(), s.as_str().to_string());
            }
        }

        // 3. Node/Frontend (pnpm / npm / yarn / bun) missing scripts
        if let Some(caps) = self.pnpm_command_not_found_re.captures(&full_text) {
            if let Some(s) = caps.name("script") {
                entities.insert("package_script".to_string(), s.as_str().to_string());
            }
        } else if let Some(caps) = self.npm_missing_script_re.captures(&full_text) {
            if let Some(s) = caps.name("script") {
                entities.insert("package_script".to_string(), s.as_str().to_string());
            }
        }

        // 4. Cargo subcommands
        if let Some(caps) = self.cargo_did_you_mean_re.captures(&full_text) {
            if let Some(s) = caps.name("subcmd") {
                entities.insert("suggested_subcommand".to_string(), s.as_str().to_string());
            }
        }

        // 5. Python module missing
        if let Some(caps) = self.python_mod_not_found_re.captures(&full_text) {
            if let Some(m) = caps.name("module") {
                entities.insert("python_module".to_string(), m.as_str().to_string());
            }
        }

        // 6. Docker / Kubectl unknown command
        if let Some(caps) = self.docker_unknown_cmd_re.captures(&full_text) {
            if let Some(s) = caps.name("subcmd") {
                entities.insert("docker_subcmd".to_string(), s.as_str().to_string());
            }
        } else if let Some(caps) = self.kubectl_unknown_cmd_re.captures(&full_text) {
            if let Some(s) = caps.name("subcmd") {
                entities.insert("kubectl_subcmd".to_string(), s.as_str().to_string());
            }
        }

        // 7. Command not found
        if let Some(caps) = self.command_not_found_re.captures(&full_text) {
            if let Some(c) = caps.name("cmd") {
                let val = c.as_str().trim_matches('\'').trim_matches('"');
                if !val.is_empty() {
                    entities.insert("missing_command".to_string(), val.to_string());
                }
            }
        }
        if !entities.contains_key("missing_command") {
            for line in stderr_lines {
                if line.contains("command not found") && !line.contains("Command \"") {
                    if let Some(cmd0) = command.split_whitespace().next() {
                        entities.insert("missing_command".to_string(), cmd0.to_string());
                        break;
                    }
                }
            }
        }

        // 8. Missing packages (apt / brew / cargo)
        if let Some(caps) = self.apt_package_re.captures(&full_text) {
            if let Some(p) = caps.name("pkg").or_else(|| caps.name("pkg2")) {
                entities.insert("package".to_string(), p.as_str().to_string());
            }
        } else if let Some(caps) = self.brew_package_re.captures(&full_text) {
            if let Some(p) = caps.name("pkg") {
                entities.insert("package".to_string(), p.as_str().to_string());
            }
        } else if let Some(caps) = self.cargo_package_re.captures(&full_text) {
            if let Some(p) = caps.name("pkg") {
                entities.insert("package".to_string(), p.as_str().to_string());
            }
        }

        // 9. Missing file/directory
        if let Some(caps) = self.no_such_file_re.captures(&full_text) {
            if let Some(p) = caps.name("path") {
                let path = p.as_str().trim_matches('\'').trim_matches('"');
                if !path.is_empty() {
                    entities.insert("path".to_string(), path.to_string());
                }
            }
        }

        entities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_git_upstream() {
        let extractor = EntityExtractor::new();
        let stderr = vec![
            "fatal: The current branch feat-login has no upstream branch.".into(),
            "To push the current branch and set the remote as upstream, use".into(),
            "    git push --set-upstream origin feat-login".into(),
        ];
        let entities = extractor.extract("git push", &stderr);
        assert_eq!(entities.get("remote").map(|s| s.as_str()), Some("origin"));
        assert_eq!(
            entities.get("branch").map(|s| s.as_str()),
            Some("feat-login")
        );
    }

    #[test]
    fn test_extract_pnpm_missing_script() {
        let extractor = EntityExtractor::new();
        let stderr = vec!["[ERR_PNPM_RECURSIVE_EXEC_FIRST_FAIL] Command \"dev\" not found".into()];
        let entities = extractor.extract("pn dev", &stderr);
        assert_eq!(
            entities.get("package_script").map(|s| s.as_str()),
            Some("dev")
        );
    }

    #[test]
    fn test_extract_python_missing_module() {
        let extractor = EntityExtractor::new();
        let stderr = vec!["ModuleNotFoundError: No module named 'requests'".into()];
        let entities = extractor.extract("python main.py", &stderr);
        assert_eq!(
            entities.get("python_module").map(|s| s.as_str()),
            Some("requests")
        );
    }
}
