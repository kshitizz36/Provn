use crate::config::EntropyConfig;

pub struct EntropyMatch {
    pub entropy: f64,
    pub confidence: f64,
}

pub fn scan_line(line: &str, cfg: &EntropyConfig) -> Option<EntropyMatch> {
    // Only check lines that look like assignments
    let has_assignment = line.contains('=') || line.contains(':');
    if !has_assignment {
        return None;
    }

    // Split on common separators and check each token
    let tokens: Vec<&str> = line
        .split(['"', '\'', ' ', '\t', ',', ';'])
        .filter(|t| t.len() >= cfg.min_length)
        .collect();

    for token in tokens {
        let h = shannon_entropy(token);
        if h >= cfg.threshold {
            // Filter out obvious false positives
            if is_likely_false_positive(token) {
                continue;
            }
            let confidence = ((h - cfg.threshold) / 2.0).min(1.0) * 0.7 + 0.3;
            return Some(EntropyMatch {
                entropy: h,
                confidence,
            });
        }
    }
    None
}

fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let len = s.len() as f64;
    let mut counts = [0u32; 256];
    for b in s.bytes() {
        counts[b as usize] += 1;
    }
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / len;
            -p * p.log2()
        })
        .sum()
}

fn is_likely_false_positive(token: &str) -> bool {
    // Base64-encoded PNG/image headers are not secrets
    if token.starts_with("iVBORw0KGgo") || token.starts_with("/9j/") {
        return true;
    }
    // Long hex strings that are hashes (64 chars = SHA-256)
    if token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    // Looks like a URL path
    if token.starts_with("http") || token.starts_with('/') {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cfg() -> EntropyConfig {
        EntropyConfig {
            enabled: true,
            threshold: 4.5,
            min_length: 20,
        }
    }

    #[test]
    fn flags_high_entropy_secret() {
        let line = r#"secret = "x7Kp2mNqR9vT4wYjLhBcDfAeGiUoSzXn""#;
        assert!(scan_line(line, &default_cfg()).is_some());
    }

    #[test]
    fn skips_png_base64() {
        let line = r#"icon = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk""#;
        assert!(scan_line(line, &default_cfg()).is_none());
    }

    #[test]
    fn skips_non_assignment_lines() {
        let line = "x7Kp2mNqR9vT4wYjLhBcDfAeGiUoSzXn";
        assert!(scan_line(line, &default_cfg()).is_none());
    }
}
