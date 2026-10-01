use clap::{Parser, Subcommand};
use mend_capture::{sanitize_stderr, EntityExtractor, PtyRunner};
use mend_core::{
    AgentHookInput, AgentHookOutput, ExecutionState, HookDecision, RemediationCandidate,
    RemediationSummary,
};
use mend_jev::{CriteriaRouter, JevClient};
use mend_reify::{SafetyGate, TemplateRenderer};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

const MEND_IN_FLIGHT_ENV: &str = "_MEND_IN_FLIGHT";

pub mod config;
pub mod daemon;
pub mod fast_path;
pub mod hook;
pub mod telemetry;
pub mod tui;

use config::{get_config_path, load_or_init_config, AppConfig};
use fast_path::FastPathEngine;
use hook::generate_shell_hook;
use telemetry::TelemetryCache;
use tui::{run_interactive_picker, TuiSelection};

#[derive(Parser, Debug)]
#[command(name = "mend")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(long)]
    hook: bool,

    #[arg(long, default_value = "zsh")]
    shell: String,

    #[arg(long, env = "JEV_ENDPOINT")]
    jev_endpoint: Option<String>,

    #[arg(long, env = "JEV_API_KEY")]
    jev_api_key: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    AgentHook {
        #[arg(long, default_value = "auto")]
        mode: String,

        #[arg(long)]
        command: Option<String>,

        #[arg(long)]
        exit_code: Option<i32>,

        #[arg(long)]
        stderr: Option<String>,

        #[arg(long)]
        json: bool,
    },
    Exec {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        cmd: Vec<String>,
    },
    Fix {
        #[arg(long, env = "_MEND_LAST_CMD")]
        command: Option<String>,

        #[arg(long, env = "_MEND_LAST_EXIT", default_value = "1")]
        exit_code: i32,

        #[arg(long)]
        stderr: Option<String>,
    },
    Init {
        shell: Option<String>,

        #[arg(long)]
        claude_code: bool,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let (cfg, newly_created) = load_or_init_config();
    if newly_created {
        eprintln!(
            "\x1b[1;36m[mend]\x1b[0m 首次运行检测：已自动生成配置文件至 \x1b[1;33m{}\x1b[0m",
            get_config_path().display()
        );
        if cfg.jev_api_key.trim().is_empty() {
            eprintln!(
                "\x1b[1;36m[mend]\x1b[0m 如需启用云端复杂自愈模型，请编辑该文件填写 \x1b[1;32m\"jev_api_key\"\x1b[0m。"
            );
        }
    }

    let final_endpoint = cli.jev_endpoint.unwrap_or_else(|| cfg.jev_endpoint.clone());
    let final_api_key = cli.jev_api_key.or_else(|| {
        if cfg.jev_api_key.trim().is_empty() {
            None
        } else {
            Some(cfg.jev_api_key.clone())
        }
    });

    if cli.hook {
        println!("{}", generate_shell_hook(&cli.shell));
        return ExitCode::SUCCESS;
    }

    match cli.command {
        Some(Commands::AgentHook {
            mode,
            command,
            exit_code,
            stderr,
            json,
        }) => {
            handle_agent_hook(
                mode,
                command,
                exit_code,
                stderr,
                json,
                &final_endpoint,
                final_api_key.as_deref(),
                &cfg,
            )
            .await
        }
        Some(Commands::Init {
            shell: None,
            claude_code: true,
        }) => handle_init_claude_code(),
        Some(Commands::Init { shell: Some(s), .. }) => {
            println!("{}", generate_shell_hook(&s));
            ExitCode::SUCCESS
        }
        Some(Commands::Init {
            shell: None,
            claude_code: false,
        }) => handle_init(),
        Some(Commands::Exec { cmd }) => {
            if cmd.is_empty() {
                eprintln!("Usage: mend exec -- <COMMAND> [ARGS...]");
                return ExitCode::from(1);
            }
            handle_exec(cmd, &final_endpoint, final_api_key.as_deref(), &cfg).await
        }
        Some(Commands::Fix {
            command,
            exit_code,
            stderr,
        }) => {
            handle_fix(
                command,
                exit_code,
                stderr,
                &final_endpoint,
                final_api_key.as_deref(),
                &cfg,
            )
            .await
        }
        None => {
            println!("{}", generate_shell_hook(&cli.shell));
            ExitCode::SUCCESS
        }
    }
}

fn handle_init() -> ExitCode {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let shell_path = env::var("SHELL").unwrap_or_default();

    let (shell_name, rc_file) = if shell_path.ends_with("/zsh") || shell_path == "zsh" {
        ("zsh", PathBuf::from(&home).join(".zshrc"))
    } else if shell_path.ends_with("/bash") || shell_path == "bash" {
        ("bash", PathBuf::from(&home).join(".bashrc"))
    } else if shell_path.ends_with("/fish") || shell_path == "fish" {
        (
            "fish",
            PathBuf::from(&home).join(".config/fish/config.fish"),
        )
    } else {
        ("zsh", PathBuf::from(&home).join(".zshrc"))
    };

    println!("\x1b[1;32m=== mend 终端环境初始化 (Init) ===\x1b[0m");
    println!("检测到当前系统 Shell: \x1b[1;34m{}\x1b[0m", shell_name);
    println!(
        "配置文件路径: \x1b[1;34m{}\x1b[0m",
        get_config_path().display()
    );
    println!("Shell 注入目标文件: \x1b[1;34m{}\x1b[0m", rc_file.display());

    let hook_line = match shell_name {
        "fish" => "mend init fish | source".to_string(),
        _ => format!("eval \"$(mend --hook --shell {})\"", shell_name),
    };

    let already_configured = if rc_file.exists() {
        fs::read_to_string(&rc_file)
            .map(|s| s.contains("mend") || s.contains("jev-heal"))
            .unwrap_or(false)
    } else {
        false
    };

    if already_configured {
        println!(
            "\x1b[1;33m[已存在]\x1b[0m {} 中已包含 mend 相关钩子配置，无需重复写入。",
            rc_file.display()
        );
    } else {
        let mut file = match OpenOptions::new().create(true).append(true).open(&rc_file) {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "\x1b[1;31m[错误]\x1b[0m 无法写入配置文件 {}: {}",
                    rc_file.display(),
                    e
                );
                return ExitCode::from(1);
            }
        };

        let block = format!("\n# mend auto-generated hook\n{}\n", hook_line);
        if let Err(e) = file.write_all(block.as_bytes()) {
            eprintln!("\x1b[1;31m[错误]\x1b[0m 写入失败: {}", e);
            return ExitCode::from(1);
        }
        println!(
            "\x1b[1;32m[成功]\x1b[0m 已成功向 {} 写入挂载脚本：",
            rc_file.display()
        );
        println!("  \x1b[1;36m{}\x1b[0m", hook_line);
    }

    println!("\n\x1b[1;32m[完成]\x1b[0m 请在终端执行以下命令生效：");
    println!("  source {}", rc_file.display());

    ExitCode::SUCCESS
}

fn handle_init_claude_code() -> ExitCode {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let settings_path = PathBuf::from(&home).join(".claude/settings.json");

    println!("\x1b[1;32m=== mend Claude Code Hook 初始化 ===\x1b[0m");
    println!("目标配置文件: \x1b[1;34m{}\x1b[0m", settings_path.display());

    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path).unwrap_or_else(|_| "{}".to_string());
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    let map = settings.as_object_mut().unwrap();
    let hooks = map.entry("hooks").or_insert_with(|| serde_json::json!({}));
    if !hooks.is_object() {
        *hooks = serde_json::json!({});
    }
    let post_tool_use = hooks
        .as_object_mut()
        .unwrap()
        .entry("PostToolUse")
        .or_insert_with(|| serde_json::json!([]));

    let already_present = post_tool_use
        .as_array()
        .map(|arr| {
            arr.iter()
                .any(|entry| entry.to_string().contains("mend agent-hook"))
        })
        .unwrap_or(false);

    if already_present {
        println!(
            "\x1b[1;33m[已存在]\x1b[0m settings.json 中已包含 mend agent-hook 钩子，无需重复写入。"
        );
        return ExitCode::SUCCESS;
    }

    if let Some(arr) = post_tool_use.as_array_mut() {
        arr.push(serde_json::json!({
            "matcher": "Bash",
            "hooks": [
                {
                    "type": "command",
                    "command": "mend agent-hook"
                }
            ]
        }));
    }

    let formatted = match serde_json::to_string_pretty(&settings) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("\x1b[1;31m[错误]\x1b[0m 序列化 JSON 失败: {}", e);
            return ExitCode::from(1);
        }
    };

    if let Err(e) = fs::write(&settings_path, formatted) {
        eprintln!("\x1b[1;31m[错误]\x1b[0m 写入 settings.json 失败: {}", e);
        return ExitCode::from(1);
    }

    println!("\x1b[1;32m[成功]\x1b[0m 已将 mend agent-hook 注入到 Claude Code PostToolUse 钩子！");
    println!("现在当 Claude Code 运行 Bash 报错时，mend 将在后台就地自愈。");
    ExitCode::SUCCESS
}

async fn handle_agent_hook(
    mode: String,
    cli_command: Option<String>,
    cli_exit_code: Option<i32>,
    cli_stderr: Option<String>,
    json_flag: bool,
    endpoint: &str,
    api_key: Option<&str>,
    cfg: &AppConfig,
) -> ExitCode {
    let mut stdin_buf = String::new();
    if !io::stdin().is_terminal() {
        let _ = io::stdin().read_to_string(&mut stdin_buf);
    }

    let input: AgentHookInput = if !stdin_buf.trim().is_empty() {
        serde_json::from_str(&stdin_buf).unwrap_or_default()
    } else {
        AgentHookInput::default()
    };

    let command = cli_command
        .or_else(|| input.extract_command())
        .unwrap_or_default();
    let exit_code = cli_exit_code.unwrap_or_else(|| input.extract_exit_code());
    let raw_stderr = cli_stderr
        .or_else(|| {
            let s = input.extract_stderr();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
        .unwrap_or_default();

    let is_json = json_flag || input.hook_event_name.is_some();

    if exit_code == 0 {
        if is_json {
            let resp = AgentHookOutput {
                decision: HookDecision::Ignored,
                original_command: command,
                original_exit_code: 0,
                new_exit_code: None,
                suggested_command: None,
                system_message: None,
                updated_output: None,
                remediation: None,
            };
            println!("{}", serde_json::to_string(&resp).unwrap());
        }
        return ExitCode::SUCCESS;
    }

    if env::var(MEND_IN_FLIGHT_ENV).is_ok() {
        eprintln!("[mend:recursion-breaker] In-flight healing detected. Skipping.");
        if is_json {
            let resp = AgentHookOutput {
                decision: HookDecision::CircuitBroken,
                original_command: command,
                original_exit_code: exit_code,
                new_exit_code: None,
                suggested_command: None,
                system_message: Some("Anti-recursion lock triggered".to_string()),
                updated_output: None,
                remediation: None,
            };
            println!("{}", serde_json::to_string(&resp).unwrap());
        }
        return ExitCode::from(exit_code as u8);
    }

    env::set_var(MEND_IN_FLIGHT_ENV, "1");

    let sanitized = sanitize_stderr(raw_stderr.as_bytes(), 12);
    let mut state =
        ExecutionState::new(&command, exit_code).with_sanitized_lines(sanitized.clone());

    let extractor = EntityExtractor::new();
    let entities = extractor.extract(&command, &sanitized);
    state = state.with_entities(entities);

    let remediation_candidate = resolve_remediation(&mut state, endpoint, api_key, cfg).await;

    match remediation_candidate {
        Some(cand) => {
            if let Err(safety_err) =
                SafetyGate::verify(&cand.strategy, cand.destructive_risk, cand.confidence, true)
            {
                eprintln!("[mend:safety-gate] Circuit breaker triggered: {}", safety_err);
                if is_json {
                    let resp = AgentHookOutput {
                        decision: HookDecision::CircuitBroken,
                        original_command: command,
                        original_exit_code: exit_code,
                        new_exit_code: None,
                        suggested_command: Some(cand.rendered_command.clone()),
                        system_message: Some(format!("Safety gate blocked: {}", safety_err)),
                        updated_output: None,
                        remediation: Some(RemediationSummary {
                            strategy: cand.strategy.as_str().to_string(),
                            command: cand.rendered_command.clone(),
                            confidence: cand.confidence,
                            destructive_risk: cand.destructive_risk,
                        }),
                    };
                    println!("{}", serde_json::to_string(&resp).unwrap());
                }
                return ExitCode::from(exit_code as u8);
            }

            if mode == "suggest" {
                eprintln!("[mend:suggestion] Suggested fix: {}", cand.rendered_command);
                if is_json {
                    let resp = AgentHookOutput {
                        decision: HookDecision::Suggested,
                        original_command: command,
                        original_exit_code: exit_code,
                        new_exit_code: None,
                        suggested_command: Some(cand.rendered_command.clone()),
                        system_message: Some(format!("Suggested fix: {}", cand.rendered_command)),
                        updated_output: None,
                        remediation: Some(RemediationSummary {
                            strategy: cand.strategy.as_str().to_string(),
                            command: cand.rendered_command,
                            confidence: cand.confidence,
                            destructive_risk: cand.destructive_risk,
                        }),
                    };
                    println!("{}", serde_json::to_string(&resp).unwrap());
                }
                return ExitCode::from(exit_code as u8);
            }

            eprintln!("[auto-mended] Retrying with: {}", cand.rendered_command);

            let retry_tokens: Vec<String> = cand
                .rendered_command
                .split_whitespace()
                .map(String::from)
                .collect();
            if retry_tokens.is_empty() {
                return ExitCode::from(exit_code as u8);
            }

            let retry_prog = &retry_tokens[0];
            let retry_args = &retry_tokens[1..];
            let pty_runner = PtyRunner::default();
            let env_overrides = [(MEND_IN_FLIGHT_ENV.to_string(), "1".to_string())];

            match pty_runner.run(retry_prog, retry_args, &env_overrides, None, true) {
                Ok(retry_out) => {
                    if retry_out.exit_code == 0 {
                        eprintln!("[auto-mended] Remediation successful.");
                        let mut cache = TelemetryCache::load();
                        cache.record_success(&command, &cand.strategy);

                        let updated_output = format!(
                            "[mend:auto-mended] Original command failed (exit {}).\n[mend:auto-mended] Applied fix: {}\n{}",
                            exit_code,
                            cand.rendered_command,
                            String::from_utf8_lossy(&retry_out.raw_output)
                        );

                        if is_json {
                            let resp = AgentHookOutput {
                                decision: HookDecision::Remediated,
                                original_command: command,
                                original_exit_code: exit_code,
                                new_exit_code: Some(0),
                                suggested_command: Some(cand.rendered_command.clone()),
                                system_message: Some(format!(
                                    "Auto-mended via: {}",
                                    cand.rendered_command
                                )),
                                updated_output: Some(updated_output),
                                remediation: Some(RemediationSummary {
                                    strategy: cand.strategy.as_str().to_string(),
                                    command: cand.rendered_command,
                                    confidence: cand.confidence,
                                    destructive_risk: cand.destructive_risk,
                                }),
                            };
                            println!("{}", serde_json::to_string(&resp).unwrap());
                        }
                        ExitCode::SUCCESS
                    } else {
                        eprintln!(
                            "mend: Remediation retry failed with exit code {}",
                            retry_out.exit_code
                        );
                        let mut cache = TelemetryCache::load();
                        cache.record_failure(&command, &cand.strategy);
                        ExitCode::from(retry_out.exit_code as u8)
                    }
                }
                Err(e) => {
                    eprintln!("mend: Failed to execute remediation: {}", e);
                    ExitCode::from(exit_code as u8)
                }
            }
        }
        None => {
            if is_json {
                let resp = AgentHookOutput {
                    decision: HookDecision::Ignored,
                    original_command: command,
                    original_exit_code: exit_code,
                    new_exit_code: None,
                    suggested_command: None,
                    system_message: Some("No remediation strategy matched".to_string()),
                    updated_output: None,
                    remediation: None,
                };
                println!("{}", serde_json::to_string(&resp).unwrap());
            }
            ExitCode::from(exit_code as u8)
        }
    }
}

async fn handle_exec(
    cmd_tokens: Vec<String>,
    endpoint: &str,
    api_key: Option<&str>,
    cfg: &AppConfig,
) -> ExitCode {
    let full_command = cmd_tokens.join(" ");
    let prog = &cmd_tokens[0];
    let args = &cmd_tokens[1..];

    let pty_runner = PtyRunner::default();
    let initial_run = match pty_runner.run(prog, args, &[], None, true) {
        Ok(out) => out,
        Err(e) => {
            eprintln!("mend: PTY execution failed: {}", e);
            return ExitCode::from(1);
        }
    };

    if initial_run.exit_code == 0 {
        return ExitCode::SUCCESS;
    }

    if env::var(MEND_IN_FLIGHT_ENV).is_ok() {
        return ExitCode::from(initial_run.exit_code as u8);
    }

    let sanitized = sanitize_stderr(&initial_run.raw_output, 12);
    let mut state = ExecutionState::new(&full_command, initial_run.exit_code)
        .with_sanitized_lines(sanitized.clone());

    let extractor = EntityExtractor::new();
    let entities = extractor.extract(&full_command, &sanitized);
    state = state.with_entities(entities);

    let remediation_candidate = resolve_remediation(&mut state, endpoint, api_key, cfg).await;

    match remediation_candidate {
        Some(cand) => {
            if let Err(safety_err) =
                SafetyGate::verify(&cand.strategy, cand.destructive_risk, cand.confidence, true)
            {
                eprintln!(
                    "mend: circuit breaker triggered for agent safety: {}",
                    safety_err
                );
                return ExitCode::from(initial_run.exit_code as u8);
            }

            eprintln!("[auto-mended] Retrying with: {}", cand.rendered_command);

            let retry_tokens: Vec<String> = cand
                .rendered_command
                .split_whitespace()
                .map(String::from)
                .collect();
            if retry_tokens.is_empty() {
                return ExitCode::from(initial_run.exit_code as u8);
            }

            let retry_prog = &retry_tokens[0];
            let retry_args = &retry_tokens[1..];
            let env_overrides = [(MEND_IN_FLIGHT_ENV.to_string(), "1".to_string())];

            match pty_runner.run(retry_prog, retry_args, &env_overrides, None, true) {
                Ok(retry_out) => {
                    if retry_out.exit_code == 0 {
                        eprintln!("[auto-mended] Remediation successful.");
                        let mut cache = TelemetryCache::load();
                        cache.record_success(&full_command, &cand.strategy);
                        ExitCode::SUCCESS
                    } else {
                        eprintln!(
                            "mend: Remediation retry failed with exit code {}",
                            retry_out.exit_code
                        );
                        let mut cache = TelemetryCache::load();
                        cache.record_failure(&full_command, &cand.strategy);
                        ExitCode::from(retry_out.exit_code as u8)
                    }
                }
                Err(e) => {
                    eprintln!("mend: Failed to execute remediation: {}", e);
                    let mut cache = TelemetryCache::load();
                    cache.record_failure(&full_command, &cand.strategy);
                    ExitCode::from(initial_run.exit_code as u8)
                }
            }
        }
        None => ExitCode::from(initial_run.exit_code as u8),
    }
}

async fn handle_fix(
    command_opt: Option<String>,
    exit_code: i32,
    stderr_opt: Option<String>,
    endpoint: &str,
    api_key: Option<&str>,
    cfg: &AppConfig,
) -> ExitCode {
    let command = command_opt.unwrap_or_else(|| {
        env::var("_MEND_LAST_CMD")
            .or_else(|_| env::var("_JEV_HEAL_LAST_CMD"))
            .unwrap_or_else(|_| "unknown_command".to_string())
    });

    if command.trim().is_empty() || command == "unknown_command" {
        eprintln!("mend: No previous command executed in this session.");
        return ExitCode::from(1);
    }

    let raw_stderr = stderr_opt.unwrap_or_default();
    let sanitized = sanitize_stderr(raw_stderr.as_bytes(), 12);

    if exit_code == 0 && sanitized.is_empty() {
        eprintln!("mend: Previous command succeeded (exit code 0). Nothing to mend.");
        return ExitCode::SUCCESS;
    }

    let mut state =
        ExecutionState::new(&command, exit_code).with_sanitized_lines(sanitized.clone());
    let extractor = EntityExtractor::new();
    let entities = extractor.extract(&command, &sanitized);
    state = state.with_entities(entities);

    let candidate_opt = resolve_remediation(&mut state, endpoint, api_key, cfg).await;

    if let Some(cand) = candidate_opt {
        if let Err(e) = SafetyGate::verify(
            &cand.strategy,
            cand.destructive_risk,
            cand.confidence,
            false,
        ) {
            eprintln!(
                "\x1b[31mmend: Cannot suggest action due to safety threshold: {}\x1b[0m",
                e
            );
            return ExitCode::from(1);
        }

        let requires_yes = SafetyGate::requires_explicit_yes(cand.destructive_risk);
        match run_interactive_picker(&[cand], requires_yes) {
            Ok(TuiSelection::Execute(cmd_to_run)) => {
                eprintln!("Executing: {}", cmd_to_run);
                let status = Command::new("sh").arg("-c").arg(&cmd_to_run).status();
                match status {
                    Ok(s) => {
                        if s.success() {
                            ExitCode::SUCCESS
                        } else {
                            ExitCode::from(s.code().unwrap_or(1) as u8)
                        }
                    }
                    Err(e) => {
                        eprintln!("Execution error: {}", e);
                        ExitCode::from(1)
                    }
                }
            }
            Ok(TuiSelection::Abort) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("Interactive UI error: {}", e);
                ExitCode::from(1)
            }
        }
    } else {
        eprintln!("mend: No reliable remediation found for: {}", command);
        ExitCode::from(1)
    }
}

async fn resolve_remediation(
    state: &mut ExecutionState,
    endpoint: &str,
    api_key: Option<&str>,
    cfg: &AppConfig,
) -> Option<RemediationCandidate> {
    if let Some(cand) = FastPathEngine::try_mend(state) {
        return Some(cand);
    }

    if api_key.is_none() || api_key.map(|k| k.trim().is_empty()).unwrap_or(true) {
        eprintln!(
            "\x1b[1;33m[mend 警告]\x1b[0m 本地快速路径未命中，且未在 \x1b[1;36m{}\x1b[0m 中检测到有效的 \x1b[1;36m\"jev_api_key\"\x1b[0m，跳过云端 Jev 深度诊断。",
            get_config_path().display()
        );
        return None;
    }

    let mut denylist = cfg.parsed_denylist();
    let telemetry = TelemetryCache::load();
    for penalized in telemetry.get_penalized_actions(&state.command) {
        if !denylist.contains(&penalized) {
            denylist.push(penalized);
        }
    }

    let router = CriteriaRouter::with_policies(denylist, cfg.parsed_allowlist());
    let plan = router.plan(state);
    let client = JevClient::new(endpoint, api_key.map(String::from));

    match client.evaluate(&plan).await {
        Ok(jev_resp) => TemplateRenderer::render(
            &jev_resp.remediation_action,
            state,
            jev_resp.action_confidence,
            jev_resp.destructive_risk,
            false,
        )
        .ok(),
        Err(e) => {
            tracing::warn!("Jev evaluation skipped or failed: {}", e);
            None
        }
    }
}
