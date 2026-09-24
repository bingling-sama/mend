# PROJECT KNOWLEDGE BASE

**Generated:** 2026-09-23
**Commit:** pre-init
**Branch:** main

## OVERVIEW
jev-heal is a high-performance terminal error self-healing engine (Rust single binary) providing safe, zero-side-effect error correction for human developers and autonomous AI coding agents.

## STRUCTURE
```
fuck-jev/
├── PRD-jev-heal.md          # Product requirements, OKRs, and system boundaries
├── PLAN.md                  # 5-crate architecture, runtime protocol, and implementation roadmap
├── docs/agents/             # Agent skills contracts (issue tracker, triage labels, domain docs)
└── crates/ (planned)        # 5-crate Rust workspace (core, capture, jev, reify, cli)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Requirements & OKRs | `PRD-jev-heal.md` | Target latency, safety thresholds, user stories |
| Crate architecture & roadmap | `PLAN.md` | Workspace layout, PTY ring buffer, Jev client |
| Agent issue tracker rules | `docs/agents/issue-tracker.md` | GitHub CLI (`gh`) integration & wayfinding |
| Triage role mapping | `docs/agents/triage-labels.md` | Canonical triage label definitions |
| Domain doc conventions | `docs/agents/domain.md` | Single-context layout rules and glossary |

## CODE MAP
Project currently in architectural specification phase (<10 files; 0 source files). Code map will populate upon crate scaffolding (`crates/heal-core`, `crates/heal-capture`, `crates/heal-jev`, `crates/heal-reify`, `crates/heal-cli`).

## CONVENTIONS
- **100% Safe Rust**: No raw LLM shell generation; all remediations reify through deterministic, pre-vetted Rust templates.
- **Zero-Side-Effect Capture**: Real-time error interception via OSC 133 sequences (human mode) or `portable-pty` 64KB ring buffer (agent mode).
- **Single-Binary Footprint**: Release build with `lto = "fat"`, `strip = true`, target binary <8 MB.
- **Bimodal Safety Gates**:
  - Agent mode: Auto-heal only when `confidence >= 0.92` AND `destructive_risk <= 0.10`.
  - Human mode: Require typing `"yes"` when `destructive_risk > 0.30`.
- **Karpathy Principles**: Minimal surgical changes; solve the direct problem without speculative abstractions.

## ANTI-PATTERNS (THIS PROJECT)
- **NEVER re-run failed commands** in the background to capture stderr (the critical flaw of legacy tools like `thefuck`).
- **NEVER generate free-form shell code** with generative LLMs; return discrete action enum IDs only.
- **DO NOT invoke remote Jev API for trivial errors**: Handle Exit 126 (`sudo`) and Exit 127 (Levenshtein distance <= 2) via Tier 1 local fast-path in memory.
- **DO NOT perform cold TLS handshakes per invocation**: Use background daemon over UDS or HTTP/2 keep-alive connection pool.
- **DO NOT flag missing domain docs** (`CONTEXT.md` / `docs/adr/`) before terms/decisions are actually resolved.

## COMMANDS
```bash
# Planned developer workflow (once Cargo workspace is scaffolded)
cargo check --workspace
cargo test --workspace
cargo build --release
```

## NOTES
- Sub-100ms p95 latency is a hard constraint for local fast-path; sub-250ms for cloud Jev evaluations.
- When implementing, start with `crates/heal-core` contracts before building capture or reification crates.

## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default triage labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context repository layout (`CONTEXT.md` and `docs/adr/`). See `docs/agents/domain.md`.
