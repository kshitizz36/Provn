use std::time::Instant;
use crate::config::Config;
use crate::diff::DiffChunk;

pub mod ast;
pub mod entropy;
pub mod regex_scan;
pub mod semantic;

#[derive(Debug, Clone, Default)]
pub struct ScanResult {
    pub file: Option<String>,
    pub line: Option<usize>,
    pub match_type: Option<String>,
    pub description: Option<String>,
    pub snippet: Option<String>,
    pub redacted: Option<String>,
    pub confidence: f64,
    pub layer: Option<String>,
    pub tier: Option<String>,
    pub latency_ms: u64,
}

/// Scan all chunks and return every finding, sorted by confidence descending.
/// Layer 3 (semantic) is invoked at most once — on the best ambiguous candidate —
/// to keep pre-commit latency low.
pub fn scan_chunks(chunks: &[DiffChunk], cfg: &Config) -> Vec<ScanResult> {
    let start = Instant::now();

    let mut confirmed: Vec<ScanResult> = Vec::new(); // high-confidence, no L3 needed
    let mut ambiguous: Option<ScanResult> = None;    // best L1/L2 in the grey zone

    for chunk in chunks {
        for (line_num, line_content) in &chunk.added_lines {
            // ── Layer 1a: regex ─────────────────────────────────────────────
            if cfg.layers.regex.enabled {
                if let Some(m) = regex_scan::scan_line(line_content, &cfg.layers.regex.custom_patterns) {
                    let r = ScanResult {
                        file:        Some(chunk.file.to_string_lossy().into_owned()),
                        line:        Some(*line_num),
                        match_type:  Some(m.pattern_name.clone()),
                        description: Some(
                            m.description.unwrap_or_else(|| {
                                format!("Matched pattern: {}", m.pattern_name)
                            }),
                        ),
                        snippet:     Some(line_content.chars().take(120).collect()),
                        redacted:    Some(m.redacted),
                        confidence:  m.confidence,
                        layer:       Some("regex".to_string()),
                        tier:        Some(m.tier.clone()),
                        latency_ms:  0,
                    };
                    if m.confidence >= cfg.layers.semantic.ambiguous_high {
                        confirmed.push(r);
                    } else {
                        keep_best(&mut ambiguous, r);
                    }
                }
            }

            // ── Layer 1b: entropy ────────────────────────────────────────────
            if cfg.layers.entropy.enabled {
                if let Some(m) = entropy::scan_line(line_content, &cfg.layers.entropy) {
                    let r = ScanResult {
                        file:        Some(chunk.file.to_string_lossy().into_owned()),
                        line:        Some(*line_num),
                        match_type:  Some("high_entropy".to_string()),
                        description: Some(format!("High entropy token (H={:.2})", m.entropy)),
                        snippet:     Some(line_content.chars().take(120).collect()),
                        redacted:    Some("PROVN_REDACTED_HIGH_ENTROPY".to_string()),
                        confidence:  m.confidence,
                        layer:       Some("entropy".to_string()),
                        tier:        Some("T2".to_string()),
                        latency_ms:  0,
                    };
                    if m.confidence >= cfg.layers.semantic.ambiguous_high {
                        confirmed.push(r);
                    } else {
                        keep_best(&mut ambiguous, r);
                    }
                }
            }
        }

        // ── Layer 2: AST (per file) ──────────────────────────────────────────
        if cfg.layers.ast.enabled {
            let src: String = chunk
                .added_lines
                .iter()
                .map(|(_, l)| l.as_str())
                .collect::<Vec<_>>()
                .join("\n");

            let lang = match chunk.extension.as_str() {
                "py"                                    => Some("python"),
                "ts" | "tsx" | "js" | "jsx" | "mjs"   => Some("javascript"),
                _                                       => None,
            };

            if let Some(lang) = lang {
                if let Some(m) = ast::scan_source(&src, lang, &cfg.layers.ast) {
                    let r = ScanResult {
                        file:        Some(chunk.file.to_string_lossy().into_owned()),
                        line:        Some(m.line),
                        match_type:  Some("ast_taint".to_string()),
                        description: Some(format!(
                            "Sensitive variable '{}' assigned string literal",
                            m.var_name
                        )),
                        snippet:     Some(m.snippet.chars().take(120).collect()),
                        redacted:    Some(format!("PROVN_REDACTED_{}", m.var_name.to_uppercase())),
                        confidence:  m.confidence,
                        layer:       Some("ast".to_string()),
                        tier:        Some("T1".to_string()),
                        latency_ms:  0,
                    };
                    if m.confidence >= cfg.layers.semantic.ambiguous_high {
                        confirmed.push(r);
                    } else {
                        keep_best(&mut ambiguous, r);
                    }
                }
            }
        }
    }

    // ── Layer 3: semantic — one call on the best ambiguous candidate ──────────
    if let Some(cand) = ambiguous {
        let conf    = cand.confidence;
        let sem_cfg = &cfg.layers.semantic;
        let ready   = sem_cfg.enabled && !sem_cfg.model.trim().is_empty();

        if ready && conf >= sem_cfg.ambiguous_low && conf < sem_cfg.ambiguous_high {
            let code = cand.snippet.as_deref().unwrap_or("");
            let sem  = semantic::classify(code, &sem_cfg.endpoint, sem_cfg.timeout_ms);

            if !sem.skipped {
                if sem.label == "leak" {
                    let mut c = cand;
                    c.confidence = 0.85;
                    c.layer      = Some("semantic".to_string());
                    confirmed.push(c);
                }
                // "clean" → L3 cleared it, discard
            } else {
                // L3 unavailable — apply fallback policy
                if sem_cfg.fallback != "clean" {
                    confirmed.push(cand);
                }
            }
        } else if conf >= sem_cfg.ambiguous_low {
            // Outside ambiguous band but above low threshold — include as-is
            confirmed.push(cand);
        }
    }

    // Sort by confidence descending and stamp shared latency
    confirmed.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    let elapsed = start.elapsed().as_millis() as u64;
    for r in &mut confirmed {
        r.latency_ms = elapsed;
    }
    confirmed
}

/// Keep the higher-confidence candidate in the ambiguous pool.
fn keep_best(slot: &mut Option<ScanResult>, new: ScanResult) {
    match slot {
        Some(existing) if existing.confidence >= new.confidence => {}
        _ => *slot = Some(new),
    }
}
