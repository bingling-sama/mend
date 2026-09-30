use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
    ExecutableCommand,
};
use mend_core::RemediationCandidate;
use std::io::{stdin, stdout, IsTerminal, Write};

pub enum TuiSelection {
    Execute(String),
    Abort,
}

pub fn run_interactive_picker(
    candidates: &[RemediationCandidate],
    requires_explicit_yes: bool,
) -> Result<TuiSelection, Box<dyn std::error::Error>> {
    if candidates.is_empty() {
        return Ok(TuiSelection::Abort);
    }

    if requires_explicit_yes {
        return run_high_risk_prompt(&candidates[0]);
    }

    if !stdout().is_terminal() || !stdin().is_terminal() {
        let cand = &candidates[0];
        let prefix = if !cand.is_fast_path {
            "Jev recommends you: "
        } else {
            ""
        };
        eprintln!("{}\x1b[1;32m{}\x1b[0m", prefix, cand.rendered_command);
        return Ok(TuiSelection::Execute(cand.rendered_command.clone()));
    }

    let mut selected_idx = 0;
    let mut out = stdout();

    enable_raw_mode()?;

    let chosen;

    loop {
        let _ = out.execute(Clear(ClearType::CurrentLine));
        let _ = out.execute(cursor::MoveToColumn(0));

        let cand = &candidates[selected_idx];
        let prompt_nav = if candidates.len() > 1 {
            "[enter/↑/↓/ctrl+c]"
        } else {
            "[enter/ctrl+c]"
        };

        let prefix = if !cand.is_fast_path {
            "Jev recommends you: "
        } else {
            ""
        };

        print!(
            "{}\x1b[1;32m{}\x1b[0m \x1b[1;33m{}\x1b[0m",
            prefix, cand.rendered_command, prompt_nav
        );
        out.flush()?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    chosen = TuiSelection::Abort;
                    break;
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    chosen = TuiSelection::Abort;
                    break;
                }
                KeyCode::Enter => {
                    chosen =
                        TuiSelection::Execute(candidates[selected_idx].rendered_command.clone());
                    break;
                }
                KeyCode::Up => {
                    if selected_idx > 0 {
                        selected_idx -= 1;
                    } else {
                        selected_idx = candidates.len() - 1;
                    }
                }
                KeyCode::Down => {
                    if selected_idx + 1 < candidates.len() {
                        selected_idx += 1;
                    } else {
                        selected_idx = 0;
                    }
                }
                _ => {}
            }
        }
    }

    let _ = disable_raw_mode();
    let _ = out.execute(Clear(ClearType::CurrentLine));
    let _ = out.execute(cursor::MoveToColumn(0));
    out.flush()?;

    Ok(chosen)
}

fn run_high_risk_prompt(
    candidate: &RemediationCandidate,
) -> Result<TuiSelection, Box<dyn std::error::Error>> {
    let prefix = if !candidate.is_fast_path {
        "Jev recommends you: "
    } else {
        ""
    };
    eprintln!(
        "\x1b[1;31m==================== HIGH RISK REMEDIATION WARNING ====================\x1b[0m"
    );
    eprintln!("\x1b[1;31mAction: {}\x1b[0m", candidate.strategy.as_str());
    eprintln!(
        "\x1b[1;33mCommand: {}{}\x1b[0m",
        prefix, candidate.rendered_command
    );
    eprintln!(
        "\x1b[1;31mDestructive Risk: {:.2} (Threshold > 0.30 requires explicit confirmation)\x1b[0m",
        candidate.destructive_risk
    );
    eprintln!("\x1b[1;31mExplanation: {}\x1b[0m", candidate.explanation);
    eprintln!(
        "\x1b[1;31m========================================================================\x1b[0m"
    );
    eprint!("Type '\x1b[1;32myes\x1b[0m' to proceed and execute, or anything else to abort: ");
    stdout().flush()?;

    let mut input = String::new();
    stdin().read_line(&mut input)?;

    if input.trim().eq_ignore_ascii_case("yes") {
        Ok(TuiSelection::Execute(candidate.rendered_command.clone()))
    } else {
        eprintln!("\x1b[33mRemediation aborted by user.\x1b[0m");
        Ok(TuiSelection::Abort)
    }
}
