# Product Requirements Document (PRD): mend

**Document Name:** PRD-mend  
**Product Name:** mend (Terminal Error Healing Engine)  
**Status:** Draft  
**Version:** 1.0.0  

---

## 1. Summary

mend is a fast command-line helper that detects terminal errors and fixes them safely. When a command fails for a human or an automated AI agent, mend finds the right fix without secretly re-running commands or breaking systems. It provides single-key terminal fixes for developers and automatic, safe retries for AI agents.

---

## 2. Contacts

| Name | Role | Responsibilities & Comments |
| --- | --- | --- |
| Product Lead | Product Management | Oversees product scope, UX specifications, and rollout priorities |
| Systems Engineer | Lead Rust Architect | Owns PTY capture, OSC 133 integration, binary size, and local fast-path engine |
| AI Integration Engineer | TypeSafe Jev Specialist | Manages Jev schema design, prompt criteria routing, and confidence scoring |
| Developer Experience Lead | CLI / Shell Contributor | Designs ratatui inline interaction, shell integration scripts (Zsh, Bash, Fish) |

---

## 3. Background

### 3.1 Context

Developers spend substantial time dealing with trivial terminal errors, such as typos, forgotten `sudo` permissions, and missing git upstream branches. Tools like `thefuck` attempted to solve this years ago. However, older tools had a critical design flaw: they captured command output by silently re-running the failed command in the background. If the failed command had side effects (such as deleting files or triggering partial database changes), re-running it caused data loss or unexpected side effects.

### 3.2 Why Now?

Two major shifts make this project essential right now:

1. **The Rise of Autonomous AI Coding Agents**: Coding agents (such as Claude Code, OpenCode, and Codex) execute hundreds of bash commands unattended. When a command fails due to a minor syntax or environment issue, agents waste context tokens and round-trips guessing fixes. They need a zero-friction, deterministic error recovery mechanism.
2. **Modern Terminal Standards**: Modern terminal emulators (iTerm2, WezTerm, Ghostty, VS Code) now widely support OSC 133 Shell Integration. This allows programs to capture exact output and status codes cleanly in real time without re-running anything.

### 3.3 What Just Became Possible?

With the TypeSafe Jev Engine, we no longer need to ask general large language models to write unpredictable bash scripts. Instead, the engine classifies the root cause and selects a pre-approved, safe action template. Coupled with Rust's single-binary performance, error diagnosis and remediation take under 100 milliseconds.

---

## 4. Objective

### 4.1 What is the Objective?

Build a single binary application (`mend`) that diagnoses and fixes failed shell commands in real time with zero dangerous re-runs, serving both interactive human developers and headless AI agents.

### 4.2 Why It Matters

- **Saves Developer Focus**: Engineers stay in their flow state instead of manually fixing repetitive typos and configuration mismatches.
- **Enables Autonomous Execution**: AI agents can mend from common execution errors in a single step without hallucinating dangerous shell commands.
- **Guarantees System Safety**: By verifying risk scores and never generating open-ended shell code via LLMs, systems remain protected against destructive actions.

### 4.3 Strategic Alignment

mend supports our core mission to provide fast, rock-solid developer tools and reliable autonomous agent infrastructure.

### 4.4 Key Results (SMART OKRs)

- **KR 1 (Latency)**: Achieve p95 remediation response latency under 120 milliseconds for local fast-path fixes and under 250 milliseconds for cloud Jev evaluations.
- **KR 2 (Safety)**: 0 incidents of unconfirmed destructive command executions across all runs.
- **KR 3 (Remediation Accuracy)**: Reach an acceptance rate higher than 85% for human interactive suggestions and a first-time auto-mend pass rate higher than 90% for AI agent runs.
- **KR 4 (Binary Footprint)**: Keep the compiled binary footprint under 8 MB with zero external runtime dependencies.

---

## 5. Market Segment(s)

### 5.1 Target Segments

1. **Human Software Developers and DevOps Engineers**
   - *Job to be Done*: Run commands quickly in daily development, git workflows, and server administration without getting slowed down by simple typos or missing flags.
   - *Constraints*: High sensitivity to terminal input lag; will immediately uninstall if a tool introduces noticeable latency or causes unintended side effects.

2. **Autonomous AI Coding Agents and CI/CD Runners**
   - *Job to be Done*: Execute builds, tests, and environment setups headlessly, recovering from common operational hiccups without human intervention.
   - *Constraints*: No interactive terminal interface; requires strict exit-code signaling, structured stdout/stderr handling, and strict safety guardrails.

### 5.2 Market Boundary

mend does not attempt to replace complex debugging tools, run static code analysis, or fix multi-file application source code. Its scope is strictly bounded to shell command execution errors and operational environment fixes.

---

## 6. Value Proposition(s)

### 6.1 Customer Needs and Gains

- **Instant Fixes**: Get the corrected command immediately with a single keystroke.
- **Complete Safety**: Sleep peacefully knowing commands will never be re-run behind your back to capture error output.
- **Deterministic Quality**: Fixes are generated from vetted Rust templates with extracted entity parameters, not wild LLM hallucinations.

### 6.2 Pains Avoided

- No accidental deletions or duplicate state changes caused by background command replays.
- No waiting 3 to 5 seconds for heavyweight cloud language models to answer simple command typos.
- No terminal freezing or shell slowdowns.

### 6.3 Value Curve Comparison

| Factor | Legacy Tools (thefuck) | Raw Cloud LLM CLI | mend |
| --- | --- | --- | --- |
| Zero Side-Effect Capture | Poor (Re-runs commands) | Medium (Captures pipes) | Excellent (OSC 133 / PTY) |
| Latency | Medium (300-800ms) | Slow (2000-5000ms) | Instant (<100ms Fast-Path) |
| Safety & Blast-Radius Gate | None | Poor (Hallucination risk) | Strict (Hardcoded risk gate) |
| Agent Headless Mode | None | Medium | Native (Exit 0 / Auto-retry) |
| Resource Footprint | Heavy (Python runtime) | Variable | Minimal (<8MB single binary) |

---

## 7. Solution

### 7.1 User Experience and Flows

#### Flow A: Human Developer Flow

```plaintext
[User runs mistyped command in shell]
                 |
                 v
         Command Fails (Exit != 0)
                 |
                 v
   OSC 133 hook saves last output
                 |
                 v
       User types "fix" or "fuck"
                 |
                 v
   +-------------------------------+
   | mend evaluates output         |
   | - Fast-Path / Jev Classifier  |
   +-------------------------------+
                 |
                 +--> Low Risk: Compact ratatui inline picker shown (Enter to run)
                 |
                 +--> High Risk (>0.30): Bold warning prompt; requires explicit "yes"
```

#### Flow B: Agent Hook Flow

```plaintext
Agent executes: mend exec -- "<target-command>"
                 |
                 v
     Target command runs inside PTY
                 |
        +--------+--------+
        |                 |
     Exit 0            Exit != 0
        |                 |
  Transparent exit    mend intercepts stderr buffer
                          |
                          v
                 Is Confidence >= 0.92 AND Destructive Risk <= 0.10?
                          |
               +----------+----------+
               |                     |
              YES                    NO
               |                     |
     Auto-executes fix once      Break circuit immediately
               |                     |
        +------+------+          Return original error code
        |             |
     Success        Failed
        |             |
   Print [auto-mended] Return failure exit code
   Exit 0
```

### 7.2 Key Features

1. **Dual Execution Entry Points**:
   - `eval $(mend --hook)`: Injects shell integration for Zsh, Bash, and Fish for human usage.
   - `mend exec -- <CMD>`: Wraps sub-processes inside a clean pseudo-terminal (PTY) for automated agents.

2. **Zero-Side-Effect Context Ingestion**:
   - In human mode, reads output boundaries via OSC 133 sequences (`\x1b]133;C` and `\x1b]133;D;<exit-code>`) from shell history or temp FIFO buffers.
   - In agent mode, uses `portable-pty` with a 64 KB in-memory ring buffer.

3. **Fast-Path Heuristics (Tier 1)**:
   - Exit code 126 or `EACCES`: Suggests `PrependSudo`.
   - Exit code 127 (`command not found`): Computes Levenshtein distance against known `$PATH` binaries. If edit distance is 2 or less, returns replacement locally within 2 milliseconds.

4. **Context Hygiene and Entity Extraction**:
   - Strips ANSI escape codes using SIMD-accelerated filtering.
   - Truncates logs to the trailing 8 to 12 meaningful lines, dropping empty lines and progress bars.
   - Extracts dynamic entities (git remote names, branch tags, package names, file paths) via compiled regex tables.

5. **TypeSafe Jev Decision Engine (Tier 2)**:
   - Uses persistent HTTP/2 keep-alive connections or local daemon over Unix Domain Sockets (UDS) to minimize handshake latency.
   - Evaluates three criteria simultaneously: failure reason (choice), remediation action (choice), and destructive risk (numeric score).

6. **Deterministic Template Reification**:
   - Maps chosen action enum directly to hardcoded safe templates (for example: `git push --set-upstream <remote> <branch>`).
   - Never allows raw generative AI code generation in the shell.

7. **Safety Gatekeeper**:
   - Human threshold: Requires typing "yes" if risk score exceeds 0.30.
   - Agent threshold: Automatically aborts remediation if confidence is below 0.92 or risk score exceeds 0.10.

### 7.3 Technology Architecture

- **Language**: 100% safe Rust.
- **Workspace Layout**:
  - `crates/mend-core`: Core data models (`ExecutionState`, `DecisionPlan`, `ActionStrategy`).
  - `crates/mend-capture`: PTY runner, OSC 133 parser, ring buffer, ANSI sanitizer.
  - `crates/mend-jev`: Jev client SDK, HTTP/2 connection pooling, criteria serialization.
  - `crates/mend-reify`: Parameterized template rendering, entity substitution, safety verification gate.
  - `crates/mend-cli`: Terminal user interface (`ratatui`, `crossterm`), command-line parser, shell hook generators.
- **Optimization**: Link-Time Optimization (`lto = "fat"`), symbol stripping (`strip = true`), minimal dependency graph.

### 7.4 Assumptions

1. Modern terminal emulators used by developers support OSC 133 or allow temporary FIFO shell wrappers.
2. The TypeSafe Jev API service maintains an uptime SLA of at least 99.9% and responds in under 100ms over warm connections.
3. Over 80% of daily shell failures belong to a finite set of known root causes (permissions, branch setup, rebase, typos, missing packages).

---

## 8. Release Plan

### 8.1 Phase 1: Core Engine & Fast-Path Prototype (Month 1)

- Scaffold Cargo workspace with all five core crates.
- Implement PTY runner and ANSI sanitization pipeline.
- Implement Tier 1 local fast-path engine (exit codes 126, 127, Levenshtein matching).
- Deliver baseline CLI `mend exec -- <CMD>` supporting local auto-fixes.

### 8.2 Phase 2: Jev Cloud Integration & Safety Gate (Month 2)

- Integrate `mend-jev` client with warm connection pooling.
- Wire criteria routing for Git and common Unix tools.
- Implement template rendering engine (`mend-reify`) and safety gate rules.
- Add headless agent circuit breaker and automated single-retry logic.

### 8.3 Phase 3: Human Terminal UX & Shell Integration (Month 3)

- Build `ratatui` inline terminal UI for interactive command selection.
- Implement OSC 133 capture and shell initialization scripts for Zsh, Bash, and Fish.
- Implement optional background Unix Domain Socket daemon for sub-100ms interactive response times.

### 8.4 Phase 4: Public Beta & Performance Hardening (Month 4)

- Run end-to-end performance benchmarking and latency optimization.
- Package single binaries for macOS (ARM64/x86_64) and Linux (glibc/musl).
- Integrate mend as the standard execution wrapper in major AI coding agent frameworks.
