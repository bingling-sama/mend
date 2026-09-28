use clap::{Parser, Subcommand};
use mend_capture::{sanitize_stderr, EntityExtractor, PtyRunner};
use mend_core::{ExecutionState, RemediationCandidate};
use mend_jev::{CriteriaRouter, JevClient};
use mend_reify::{SafetyGate, TemplateRenderer};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

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
        eprintln!(
            "\x1b[1;36m[mend]\x1b[0m 如需启用云端复杂自愈模型，请编辑该文件填写 \x1b[1;32m\"jev_api_key\"\x1b[0m。"
        );
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
        Some(Commands::Init { shell: Some(s) }) => {
            println!("{}", generate_shell_hook(&s));
            ExitCode::SUCCESS
        }
        Some(Commands::Init { shell: None }) => handle_init(),
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

            match pty_runner.run(retry_prog, retry_args, &[], None, true) {
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

    let raw_stderr = stderr_opt.unwrap_or_default();
    let sanitized = sanitize_stderr(raw_stderr.as_bytes(), 12);

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
