use clap::{Parser, Subcommand};
use std::process;

mod audit;
mod config;
mod diff;
mod policy;
mod redact;
mod scanner;

// ── ANSI helpers ──────────────────────────────────────────────────────────────
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

#[allow(dead_code)]
const _BOLD_CHECK: &str = BOLD; // ensure constants are reachable

macro_rules! dim {
    ($s:expr) => {
        format!("{DIM}{}{RESET}", $s)
    };
}

// ── CLI definition ─────────────────────────────────────────────────────────────
#[derive(Parser)]
#[command(
    name = "provn",
    version,
    about = "AI-powered secret & IP leak detection",
    // Suppress default help so bare `provn` shows our dashboard instead
    disable_help_subcommand = true,
    disable_help_flag = true,
)]
struct Cli {
    #[arg(short, long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Scan staged git changes (pre-commit hook mode)
    Scan,
    /// Scan a specific file or directory for secrets and IP leaks
    Check {
        #[arg(value_name = "PATH")]
        file: String,
        /// Output results as JSON (useful for CI pipelines)
        #[arg(long, short = 'j')]
        json: bool,
    },
    /// Verify the integrity of the audit log HMAC chain
    VerifyAudit,
    /// Install the Provn pre-commit hook in the current git repo
    Install,
    /// Manage the local Layer 3 semantic inference server
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },
}

#[derive(Subcommand)]
enum ServerAction {
    /// Start the semantic server (auto-starts at login via launchd)
    Start,
    /// Stop the semantic server
    Stop,
    /// Show whether the semantic server is online
    Status,
}

// ── Entry point ────────────────────────────────────────────────────────────────
fn main() {
    let result = std::panic::catch_unwind(run);
    match result {
        Ok(code) => process::exit(code),
        Err(_) => {
            eprintln!("[provn] Unexpected panic — allowing commit");
            process::exit(0);
        }
    }
}

fn run() -> i32 {
    let cli = Cli::parse();
    match cli.command {
        None => cmd_dashboard(),
        Some(Command::Scan) => cmd_scan(),
        Some(Command::Check { file, json }) => cmd_check(&file, json),
        Some(Command::VerifyAudit) => cmd_verify_audit(),
        Some(Command::Install) => cmd_install(),
        Some(Command::Server { action }) => cmd_server(action),
    }
}

// ── Dashboard (bare `provn`) ───────────────────────────────────────────────────
fn cmd_dashboard() -> i32 {
    let healthy = server_healthy();
    let cfg = config::load().unwrap_or_default();

    let l3_link = hyperlink(
        "https://github.com/kshitizz36/Provn#layer-3-semantic-ai",
        "docs ↗",
    );
    let (l3_dot, l3_label) = if !cfg.layers.semantic.enabled {
        (
            format!("{}○{}", DIM, RESET),
            format!("{}Semantic AI  disabled{}", DIM, RESET),
        )
    } else if healthy {
        (
            format!("{}●{}", GREEN, RESET),
            format!("Semantic AI (Gemma 4 E2B)  {}online{}", GREEN, RESET),
        )
    } else {
        (
            format!("{}○{}", RED, RESET),
            format!(
                "Semantic AI (Gemma 4 E2B)  {}offline{}  ·  provn server start  {}{}{}",
                RED, RESET, DIM, l3_link, RESET,
            ),
        )
    };

    let hook_ok = std::path::Path::new(".git/hooks/pre-commit").exists();
    let hook_status = if hook_ok {
        format!("{}installed{}", GREEN, RESET)
    } else {
        format!("{}not installed{}  →  provn install", YELLOW, RESET)
    };

    eprintln!(
        "\n  {}Provn{}  ·  {}AI-powered secret & IP leak detection{}  ·  {}\n",
        BOLD,
        RESET,
        DIM,
        RESET,
        dim!(env!("CARGO_PKG_VERSION")),
    );
    eprintln!("  {}Layers{}", BOLD, RESET);
    eprintln!(
        "    Layer 1  {}●{}  Regex patterns          always active",
        GREEN, RESET
    );
    eprintln!(
        "    Layer 2  {}●{}  Entropy + AST analysis  always active",
        GREEN, RESET
    );
    eprintln!("    Layer 3  {}  {}", l3_dot, l3_label);
    eprintln!();
    eprintln!("  {}Pre-commit hook{}  {}", BOLD, RESET, hook_status);
    eprintln!();
    eprintln!("  {}Commands{}", BOLD, RESET);
    eprintln!(
        "    {}provn check <path>{}     scan a file for secrets or IP leaks",
        CYAN, RESET
    );
    eprintln!(
        "    {}provn scan{}             scan staged git changes",
        CYAN, RESET
    );
    eprintln!(
        "    {}provn server start{}     enable Layer 3 semantic AI",
        CYAN, RESET
    );
    eprintln!(
        "    {}provn server status{}    check if Layer 3 is online",
        CYAN, RESET
    );
    eprintln!(
        "    {}provn install{}          install git pre-commit hook",
        CYAN, RESET
    );
    eprintln!(
        "    {}provn verify-audit{}     verify audit log integrity",
        CYAN, RESET
    );
    eprintln!();
    eprintln!("  {}https://github.com/kshitizz36/Provn{}", DIM, RESET);
    eprintln!();

    0
}

// ── Remote allowlist ───────────────────────────────────────────────────────────
/// Returns true if the commit is allowed to proceed (remote is in the allowlist,
/// or the allowlist is empty, or no remote is configured).
fn check_remote_allowed(cfg: &config::Config) -> bool {
    if cfg.allowed_remotes.is_empty() {
        return true;
    }
    let remote = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string();
    if remote.is_empty() {
        return true; // local-only repo, no remote to check
    }
    cfg.allowed_remotes
        .iter()
        .any(|pattern| wildcard_match(pattern, &remote))
}

/// Minimal wildcard match: `*` matches any sequence of characters.
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !text.starts_with(part) {
                return false;
            }
            pos = part.len();
        } else if i == parts.len() - 1 {
            if !text[pos..].ends_with(part) {
                return false;
            }
        } else {
            match text[pos..].find(part) {
                Some(idx) => pos += idx + part.len(),
                None => return false,
            }
        }
    }
    true
}

// ── Scan (pre-commit) ──────────────────────────────────────────────────────────
fn cmd_scan() -> i32 {
    let cfg = config::load().unwrap_or_default();

    // Remote allowlist: block pushes to unapproved remotes
    if !check_remote_allowed(&cfg) {
        let remote = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_string();
        eprintln!();
        eprintln!("  {}✗  blocked  [remote not in allowlist]{}", RED, RESET);
        eprintln!("  {}Remote: {}{}", DIM, remote, RESET);
        eprintln!(
            "  {}Add to allowed_remotes in provn.yml to permit commits to this remote.{}",
            DIM, RESET
        );
        eprintln!();
        return 1;
    }

    if cfg.mode == "shadow" {
        eprintln!(
            "{}  shadow mode — logging only, commits always pass{}",
            DIM, RESET
        );
    }

    let chunks = match diff::parse_staged_diff(&cfg) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "{}provn  could not read diff: {e} — allowing commit{}",
                DIM, RESET
            );
            return 0;
        }
    };

    if chunks.is_empty() {
        return 0;
    }

    let findings = scanner::scan_chunks(&chunks, &cfg);
    let latency = findings.first().map(|r| r.latency_ms).unwrap_or(0);

    if findings.is_empty() {
        eprintln!(
            "  {}✓  clean{}  {}",
            GREEN,
            RESET,
            dim!(format!("{latency}ms"))
        );
        return 0;
    }

    // Process every finding; track worst exit code
    let mut exit_code = 0i32;
    let mut did_block = false;

    for result in &findings {
        let verdict = policy::determine_verdict(result, &cfg);
        audit::append(&verdict, result, &cfg).ok();

        match &verdict {
            policy::Verdict::Allow => {}
            policy::Verdict::Warn(tier) => {
                eprintln!(
                    "  {}⚠  [{}]{}  {}  {}",
                    YELLOW,
                    tier,
                    RESET,
                    result.file.as_deref().unwrap_or("?"),
                    dim!(result.description.as_deref().unwrap_or("")),
                );
            }
            policy::Verdict::Block(tier) => {
                if cfg.mode == "shadow" {
                    eprintln!(
                        "  {}[shadow]{}  would block [{}] — allowing",
                        DIM, RESET, tier
                    );
                    continue;
                }
                print_block(result, tier);
                did_block = true;
                exit_code = 1;
            }
        }
    }

    // Offer redaction only if exactly one T1 block and nothing worse
    if did_block && findings.len() == 1 {
        let result = &findings[0];
        if result.tier.as_deref() == Some("T1") {
            eprint!("\n  Accept redaction? [y/N]  ");
            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_ok()
                && input.trim().eq_ignore_ascii_case("y")
                && redact::apply_redaction(result).is_ok()
            {
                eprintln!("  redaction applied — re-stage and commit again");
                return 1;
            }
        }
    }

    if !did_block && exit_code == 0 {
        eprintln!(
            "  {}✓  clean{}  {}",
            GREEN,
            RESET,
            dim!(format!("{latency}ms"))
        );
    }

    exit_code
}

fn print_block(result: &scanner::ScanResult, tier: &str) {
    eprintln!();
    eprintln!("  {}✗  blocked  [{}]{}", RED, tier, RESET);
    if let Some(d) = &result.description {
        eprintln!("  {}", dim!(d));
    }
    if let Some(f) = &result.file {
        eprintln!(
            "  {}:{}{}",
            f,
            result.line.unwrap_or(0),
            if let Some(l) = &result.layer {
                format!("  {}", dim!(format!("via {l}")))
            } else {
                String::new()
            }
        );
    }
    if let Some(s) = &result.snippet {
        let short: String = s.chars().take(80).collect();
        eprintln!("\n  {}- {}{}", RED, short, RESET);
        eprintln!(
            "  {}+ {}{}",
            GREEN,
            result.redacted.as_deref().unwrap_or("PROVN_REDACTED"),
            RESET
        );
    }
}

// ── Check ──────────────────────────────────────────────────────────────────────
fn cmd_check(file: &str, json: bool) -> i32 {
    let cfg = config::load().unwrap_or_default();
    let chunks = match diff::parse_file(file, &cfg) {
        Ok(c) => c,
        Err(e) => {
            if json {
                println!("{}", serde_json::json!({ "error": e.to_string() }));
            } else {
                eprintln!("  {}error{}  {}: {e}", RED, RESET, file);
            }
            return 2;
        }
    };

    let findings = scanner::scan_chunks(&chunks, &cfg);
    let latency = findings.first().map(|r| r.latency_ms).unwrap_or(0);
    let clean = findings.is_empty();

    if json {
        let items: Vec<_> = findings
            .iter()
            .map(|r| {
                serde_json::json!({
                    "file":        r.file,
                    "line":        r.line,
                    "match_type":  r.match_type,
                    "tier":        r.tier,
                    "layer":       r.layer,
                    "confidence":  r.confidence,
                    "description": r.description,
                    "snippet":     r.snippet,
                    "redacted":    r.redacted,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "file":       file,
                "clean":      clean,
                "findings":   items,
                "latency_ms": latency,
            })
        );
        return if clean { 0 } else { 1 };
    }

    if clean {
        println!(
            "  {}✓  clean{}  {}",
            GREEN,
            RESET,
            dim!(format!("{latency}ms"))
        );
        return 0;
    }

    for result in &findings {
        let verdict = policy::determine_verdict(result, &cfg);
        let tier = match &verdict {
            policy::Verdict::Allow => continue,
            policy::Verdict::Warn(t) | policy::Verdict::Block(t) => t.clone(),
        };
        let desc = result.description.as_deref().unwrap_or("unknown");
        let layer = result
            .layer
            .as_deref()
            .map(|l| format!("  {}", dim!(format!("via {l}"))))
            .unwrap_or_default();
        let loc = match (result.file.as_deref(), result.line) {
            (Some(f), Some(l)) => format!("  {}", dim!(format!("{f}:{l}"))),
            (Some(f), None) => format!("  {}", dim!(f)),
            _ => String::new(),
        };
        println!("  {}✗  [{}]{}  {}{}{}", RED, tier, RESET, desc, layer, loc);
    }

    1
}

// ── Verify audit ───────────────────────────────────────────────────────────────
fn cmd_verify_audit() -> i32 {
    let cfg = config::load().unwrap_or_default();
    match audit::verify_chain(&cfg.audit.path, &cfg.audit.hmac_key_path) {
        Ok(count) => {
            if count == 0 {
                println!(
                    "  {}✓  no audit entries yet{}  {}",
                    GREEN,
                    RESET,
                    dim!("fresh repo or no findings logged")
                );
            } else {
                println!(
                    "  {}✓  audit chain intact{}  {} entries",
                    GREEN, RESET, count
                );
            }
            0
        }
        Err(e) => {
            eprintln!("  {}✗  audit chain invalid{}  {e}", RED, RESET);
            1
        }
    }
}

// ── Install ────────────────────────────────────────────────────────────────────
fn cmd_install() -> i32 {
    let hook_path = ".git/hooks/pre-commit";
    let provn_bin = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "provn".to_string());

    let content = format!("#!/bin/sh\n{provn_bin} scan\n");
    match std::fs::write(hook_path, &content) {
        Ok(_) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(hook_path) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(hook_path, perms).ok();
                }
            }
            println!("  {}✓  pre-commit hook installed{}", GREEN, RESET);
            println!("  {}provn scan will run on every git commit{}", DIM, RESET);
            0
        }
        Err(e) => {
            eprintln!("  {}✗  failed to install hook{}  {e}", RED, RESET);
            eprintln!("  {}make sure you are inside a git repo{}", DIM, RESET);
            1
        }
    }
}

// ── Server ─────────────────────────────────────────────────────────────────────
const PLIST_LABEL: &str = "com.provn.semantic-server";

fn print_server_status() -> i32 {
    if server_healthy() {
        eprintln!("  {}●  Layer 3 online{}  ·  127.0.0.1:8080", GREEN, RESET);
        eprintln!(
            "  {}Gemma 4 E2B · Q4_K_M · ambiguous-case classifier{}",
            DIM, RESET
        );
        0
    } else {
        eprintln!("  {}○  Layer 3 offline{}", RED, RESET);
        eprintln!(
            "  {}provn server start{}  to enable semantic AI  {}{}{}",
            CYAN,
            RESET,
            DIM,
            hyperlink(
                "https://github.com/kshitizz36/Provn#layer-3-semantic-ai",
                "docs ↗"
            ),
            RESET,
        );
        1
    }
}

#[cfg(target_os = "macos")]
fn cmd_server(action: ServerAction) -> i32 {
    let plist = format!(
        "{}/Library/LaunchAgents/{PLIST_LABEL}.plist",
        std::env::var("HOME").unwrap_or_default()
    );
    let uid = unsafe { libc::getuid() };
    let domain = format!("gui/{uid}");

    match action {
        ServerAction::Start => {
            eprintln!();
            eprintln!("  {}Layer 3  ·  Semantic AI{}", BOLD, RESET);
            eprintln!(
                "  {}model   {}Gemma 4 E2B · fine-tuned on LeakBench · Q4_K_M{}",
                DIM, RESET, DIM
            );
            eprintln!(
                "  {}scope   {}ambiguous detections only  (confidence 40 – 80 %%){}",
                DIM, RESET, DIM
            );
            eprintln!(
                "  {}logs    {}/tmp/provn-semantic-server.log{}",
                DIM, RESET, DIM
            );
            eprintln!();

            if server_healthy() {
                eprintln!("  {}●  already online{}  ·  127.0.0.1:8080", GREEN, RESET);
                eprintln!();
                return 0;
            }

            if !std::path::Path::new(&plist).exists() {
                eprintln!("  {}✗  launchd plist not found{}", RED, RESET);
                eprintln!("  expected: {}", dim!(&plist));
                eprintln!();
                return 1;
            }

            eprint!("  starting");
            let out = std::process::Command::new("launchctl")
                .args(["bootstrap", &domain, &plist])
                .output();

            match out {
                Ok(o) if o.status.success() => {
                    eprintln!("  {}●  online{}  ·  127.0.0.1:8080", GREEN, RESET);
                    eprintln!(
                        "  {}model loads in ~25 s  ·  provn server status to confirm{}",
                        DIM, RESET
                    );
                }
                Ok(o) => {
                    eprintln!();
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    eprintln!("  {}✗  failed to start{}  {}", RED, RESET, stderr.trim());
                    eprintln!("  {}tail -f /tmp/provn-semantic-server.log{}", DIM, RESET);
                    eprintln!();
                    return 1;
                }
                Err(e) => {
                    eprintln!();
                    eprintln!("  {}✗  launchctl error{}  {e}", RED, RESET);
                    eprintln!();
                    return 1;
                }
            }
            eprintln!();
            0
        }

        ServerAction::Stop => {
            let out = std::process::Command::new("launchctl")
                .args(["bootout", &domain, &plist])
                .output();
            match out {
                Ok(o) if o.status.success() => {
                    eprintln!("  {}○  semantic server stopped{}", DIM, RESET);
                    eprintln!(
                        "  {}Layer 3 will fall back to Layer 1 / 2 result{}",
                        DIM, RESET
                    );
                    0
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    eprintln!(
                        "  {}✗  {}{}  (may already be stopped)",
                        RED,
                        RESET,
                        stderr.trim()
                    );
                    1
                }
                Err(e) => {
                    eprintln!("  {}✗  launchctl error{}  {e}", RED, RESET);
                    1
                }
            }
        }

        ServerAction::Status => print_server_status(),
    }
}

#[cfg(not(target_os = "macos"))]
fn cmd_server(action: ServerAction) -> i32 {
    match action {
        ServerAction::Status => print_server_status(),
        ServerAction::Start => {
            eprintln!();
            if server_healthy() {
                eprintln!("  {}●  already online{}  ·  127.0.0.1:8080", GREEN, RESET);
                eprintln!();
                return 0;
            }

            eprintln!("  {}Layer 3  ·  Semantic AI{}", BOLD, RESET);
            eprintln!(
                "  {}auto-start is currently supported on macOS only{}",
                YELLOW, RESET
            );
            eprintln!(
                "  {}Start your local semantic server manually, then run {}provn server status{}{}",
                DIM, CYAN, RESET, DIM
            );
            eprintln!(
                "  {}https://github.com/kshitizz36/Provn#layer-3-semantic-ai{}",
                DIM, RESET
            );
            eprintln!();
            1
        }
        ServerAction::Stop => {
            if server_healthy() {
                eprintln!(
                    "  {}✗  auto-stop is currently supported on macOS only{}",
                    YELLOW, RESET
                );
                eprintln!(
                    "  {}Stop your semantic server manually on this platform.{}",
                    DIM, RESET
                );
                1
            } else {
                eprintln!("  {}○  Layer 3 offline{}", DIM, RESET);
                0
            }
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────
/// OSC 8 terminal hyperlink — renders as clickable text in iTerm2, Warp, kitty, etc.
fn hyperlink(url: &str, label: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{label}\x1b]8;;\x1b\\")
}

fn server_healthy() -> bool {
    let cfg = config::load().unwrap_or_default();
    let base = cfg
        .layers
        .semantic
        .endpoint
        .trim_end_matches('/')
        .trim_end_matches("/completion")
        .to_string();
    let url = format!("{base}/health");

    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(500))
        .build()
        .ok()
        .and_then(|c| c.get(&url).send().ok())
        .and_then(|r| r.text().ok())
        .map(|t| t.contains("\"ok\""))
        .unwrap_or(false)
}
