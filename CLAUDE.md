# CLAUDE.md — Provn Project Instructions

## Project Overview
Provn is a CLI-first, local pre-commit security scanner.

Current scope:
- Rust CLI for staged diff scanning
- HMAC-chained audit logging
- Optional local Gemma 4 semantic layer

Postponed for now:
- bridge services
- 3D office / Claw3D work
- any monitoring UI that is not required for the CLI

Read [PROJECT.md](/Users/kshitiz./Programming/aegis/PROJECT.md) for the current CLI-only plan.

## Repository Structure
- `provn-cli/` — Rust CLI, pre-commit hook, audit chain, local model integration
- `aegis-model/` — Gemma 4 E2B fine-tuning pipeline (Modal + Unsloth)
- `scripts/` — install, integration, and benchmark helpers
- `homebrew-tap/` — packaging metadata

## Build Commands
```bash
# Rust CLI
cd provn-cli && cargo build --release
cd provn-cli && cargo test
cd provn-cli && cargo clippy -- -D warnings

# Fine-tuning (requires Modal account)
cd aegis-model && modal run modal_finetune.py

# CLI integration smoke test
./scripts/integration-test.sh
```

## Development Workflow

### When working on the Rust CLI
- Always run `cargo clippy -- -D warnings` before finishing
- Every scanner function should have unit coverage when practical
- Target latencies: Layer 1 <30ms, Layer 2 <50ms, Layer 3 fallback-safe
- The CLI must work standalone with no extra daemon required
- Do not add bridge, office, or websocket dependencies unless the user explicitly asks

### When working on fine-tuning
- Model: Gemma 4 E2B
- Method: LoRA via Unsloth on Modal
- Task: binary classification for leak vs clean
- Export: local-friendly artifact for on-device inference
- If inference is too slow or flaky, Layer 3 must remain optional

## Code Style
- Rust: standard Rust conventions, `thiserror` for errors when needed
- Keep comments focused on the why, not the obvious what
- No fake numbers in docs, demos, or benchmark claims

## Testing Strategy
- Rust: unit tests per module plus CLI smoke coverage
- LeakBench: keep the benchmark corpus representative and honest
- Integration testing should be CLI-only for now

## Critical Constraints
1. Local-first: Provn must not send source code to cloud APIs by default
2. Fast: clean commits should stay under the latency target
3. Reliable: if a layer fails, the CLI should degrade gracefully
4. Auditable: decisions must remain reviewable through the audit chain
5. CLI-first: bridge and office work are out of scope unless explicitly requested

## Fallback Paths
- If Gemma inference hangs, skip Layer 3 and use Layer 1/2
- If audit verification fails, surface it clearly without crashing the commit flow
- If benchmark quality is weak, tune thresholds instead of overstating results

## Dependencies
### Rust
- clap = "4"
- regex = "1"
- serde = { version = "1", features = ["derive"] }
- serde_json = "1"
- sha2 = "0.10"
- ed25519-dalek = "2"
- tree-sitter = "0.24"
- tree-sitter-python = "0.23"
- tree-sitter-javascript = "0.23"
- tokio = { version = "1", features = ["full"] }
- chrono = "0.4"
- hex = "0.4"

## Reference Projects
- Gitleaks: https://github.com/gitleaks/gitleaks
- tree-sitter: https://tree-sitter.github.io/tree-sitter/
