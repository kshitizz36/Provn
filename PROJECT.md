# Provn: CLI-First AI Security Scanner

## One-liner
Provn is a local pre-commit security scanner that blocks secrets, system prompts, and proprietary IP before they leave your machine.

## Current Scope
Provn is CLI-only for now.

Included now:
- Rust CLI for staged diff scanning
- Layer 1 regex and entropy detection
- Layer 2 AST-based checks
- optional local Layer 3 semantic classification
- HMAC-chained audit log
- packaging, install flow, and CI support

Postponed:
- bridge services
- 3D office / Claw3D
- monitoring dashboards

## Why This Direction
- It keeps the product focused on the real wedge: blocking unsafe commits
- It reduces demo risk and maintenance overhead
- It lets us lock speed, accuracy, and auditability before adding visualization

## Architecture

```text
git commit
   |
   v
Provn CLI (Rust)
   |
   +-- Layer 1: regex + entropy
   +-- Layer 2: AST / taint-style checks
   +-- Layer 3: optional local Gemma review
   |
   +-- verdict: allow | warn | block
   +-- auto-redact preview
   +-- HMAC audit chain
```

## Tech Stack
- CLI Core: Rust
- Parsing: tree-sitter
- Audit: HMAC-SHA256 + Ed25519
- Semantic Layer: local Gemma 4 E2B
- Fine-tuning: Modal + Unsloth + LoRA
- Packaging: npm wrapper + Homebrew tap

## What We Are Not Building Right Now
- 3D office
- websocket bridge
- fleet management UI
- cloud scanning by default
- compliance dashboards

## Target Metrics
- Clean commit latency: p50 < 120ms, p95 < 200ms
- Recall on seeded leaks: >95%
- False positive rate: <2%
- Bypass and audit events: logged and verifiable
- Layer 3 must remain optional and fail-safe

## Execution Plan

### Phase 1: Core CLI
- keep staged diff scanning fast and deterministic
- improve regex coverage and entropy heuristics
- harden AST checks for Python / TypeScript / JavaScript
- keep the hook standalone and reliable

### Phase 2: Audit + Semantic Layer
- maintain tamper-evident audit records
- keep local model serving optional
- only escalate ambiguous findings into Layer 3
- skip Layer 3 cleanly if it is slow or unavailable

### Phase 3: Packaging + Proof
- tighten LeakBench coverage and reporting
- keep CI focused on the CLI
- improve install and upgrade paths
- prepare a terminal-first demo and README

## Key Files

```text
Provn/
├── provn-cli/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── audit.rs
│   │   ├── config.rs
│   │   ├── diff.rs
│   │   ├── policy.rs
│   │   ├── redact.rs
│   │   └── scanner/
│   └── tests/
├── aegis-model/
│   ├── modal_finetune.py
│   ├── leakbench_train.jsonl
│   └── leakbench_eval.jsonl
├── scripts/
└── homebrew-tap/
```

## Current Rule
If work does not directly improve the CLI, audit chain, local semantic layer, packaging, or CLI demo reliability, it is out of scope for now.
