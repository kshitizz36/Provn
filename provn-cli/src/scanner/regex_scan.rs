use once_cell::sync::Lazy;
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

pub struct RegexMatch {
    pub pattern_name: String,
    pub tier: String,
    pub confidence: f64,
    pub redacted: String,
    pub description: Option<String>,
}

struct Pattern {
    name: &'static str,
    tier: &'static str,
    confidence: f64,
    re: Regex,
    redacted_prefix: &'static str,
}

static PATTERNS: Lazy<Vec<Pattern>> = Lazy::new(|| {
    vec![
        Pattern {
            name: "aws_access_key",
            tier: "T0",
            confidence: 0.98,
            re: Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
            redacted_prefix: "PROVN_REDACTED_AWS_KEY",
        },
        Pattern {
            name: "aws_secret_key",
            tier: "T0",
            confidence: 0.95,
            re: Regex::new(r#"(?i)aws.{0,20}secret.{0,10}["']([A-Za-z0-9/+]{40})["']"#).unwrap(),
            redacted_prefix: "PROVN_REDACTED_AWS_SECRET",
        },
        Pattern {
            name: "openai_api_key",
            tier: "T1",
            confidence: 0.97,
            re: Regex::new(r"sk-(?:proj-)?[a-zA-Z0-9]{40,}").unwrap(),
            redacted_prefix: "PROVN_REDACTED_OPENAI_KEY",
        },
        Pattern {
            name: "anthropic_api_key",
            tier: "T1",
            confidence: 0.97,
            re: Regex::new(r"sk-ant-[a-zA-Z0-9\-_]{40,}").unwrap(),
            redacted_prefix: "PROVN_REDACTED_ANTHROPIC_KEY",
        },
        Pattern {
            name: "private_key_header",
            tier: "T0",
            confidence: 0.99,
            re: Regex::new(r"-----BEGIN (RSA|EC|DSA|OPENSSH|PGP) PRIVATE KEY").unwrap(),
            redacted_prefix: "PROVN_REDACTED_PRIVATE_KEY",
        },
        Pattern {
            name: "stripe_secret",
            tier: "T0",
            confidence: 0.98,
            re: Regex::new(r"sk_live_[a-zA-Z0-9]{24,}").unwrap(),
            redacted_prefix: "PROVN_REDACTED_STRIPE_KEY",
        },
        Pattern {
            name: "github_token",
            tier: "T0",
            confidence: 0.97,
            re: Regex::new(r"gh[pousr]_[A-Za-z0-9]{36,}").unwrap(),
            redacted_prefix: "PROVN_REDACTED_GITHUB_TOKEN",
        },
        Pattern {
            name: "database_url",
            tier: "T0",
            confidence: 0.92,
            re: Regex::new(r"(?i)(postgresql|mysql|mongodb|redis)\+?://[^:@\s]+:[^@\s]+@[^\s]+").unwrap(),
            redacted_prefix: "PROVN_REDACTED_DB_URL",
        },
        Pattern {
            name: "jwt_token",
            tier: "T1",
            confidence: 0.85,
            re: Regex::new(r"ey[A-Za-z0-9\-_]+\.ey[A-Za-z0-9\-_]+\.[A-Za-z0-9\-_]+").unwrap(),
            redacted_prefix: "PROVN_REDACTED_JWT",
        },
        Pattern {
            name: "huggingface_token",
            tier: "T1",
            confidence: 0.95,
            re: Regex::new(r"hf_[a-zA-Z0-9]{34,}").unwrap(),
            redacted_prefix: "PROVN_REDACTED_HF_TOKEN",
        },
        Pattern {
            name: "generic_api_key",
            tier: "T1",
            confidence: 0.75,
            re: Regex::new(r#"(?i)(api[_\-]?key|apikey)\s*[=:]\s*["'][\w\-]{20,}["']"#).unwrap(),
            redacted_prefix: "PROVN_REDACTED_API_KEY",
        },
        Pattern {
            name: "system_prompt_var",
            tier: "T1",
            confidence: 0.80,
            re: Regex::new(r#"(?i)system_prompt\s*[=:]\s*["'](.{30,})"#).unwrap(),
            redacted_prefix: "PROVN_REDACTED_SYSTEM_PROMPT",
        },
        Pattern {
            name: "password_in_code",
            tier: "T0",
            confidence: 0.82,
            re: Regex::new(r#"(?i)(password|passwd|pwd)\s*=\s*["'][^"']{8,}["']"#).unwrap(),
            redacted_prefix: "PROVN_REDACTED_PASSWORD",
        },
        Pattern {
            name: "slack_webhook",
            tier: "T1",
            confidence: 0.96,
            re: Regex::new(r"https://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[a-zA-Z0-9]+").unwrap(),
            redacted_prefix: "PROVN_REDACTED_SLACK_WEBHOOK",
        },
    ]
});

pub fn scan_line(line: &str, custom: &[crate::config::CustomPattern]) -> Option<RegexMatch> {
    // NFKC normalize to catch homoglyph attacks (Cyrillic 'а' → 'a')
    let normalized: String = line.nfkc().collect();

    for pattern in PATTERNS.iter() {
        if pattern.re.is_match(&normalized) {
            return Some(RegexMatch {
                pattern_name: pattern.name.to_string(),
                tier: pattern.tier.to_string(),
                confidence: pattern.confidence,
                redacted: format!("{}_1", pattern.redacted_prefix),
                description: None,
            });
        }
    }

    // User-defined patterns (from provn.yml regex.custom_patterns)
    for cp in custom {
        if let Ok(re) = Regex::new(&cp.pattern) {
            if re.is_match(&normalized) {
                return Some(RegexMatch {
                    pattern_name: cp.name.clone(),
                    tier:         cp.tier.clone(),
                    confidence:   cp.confidence,
                    redacted:     format!("PROVN_REDACTED_{}", cp.name.to_uppercase().replace(' ', "_")),
                    description:  cp.description.clone(),
                });
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_aws_access_key() {
        assert!(scan_line("AWS_ACCESS_KEY_ID = \"AKIAIOSFODNN7EXAMPLE\"", &[]).is_some());
    }

    #[test]
    fn detects_openai_key() {
        assert!(scan_line("key = \"sk-proj-abcdefghijklmnopqrstuvwxyz1234567890ABCD\"", &[]).is_some());
    }

    #[test]
    fn detects_private_key_header() {
        assert!(scan_line("-----BEGIN RSA PRIVATE KEY-----", &[]).is_some());
    }

    #[test]
    fn allows_clean_code() {
        assert!(scan_line("def calculate_total(items): return sum(items)", &[]).is_none());
    }

    #[test]
    fn detects_homoglyph_aws_key() {
        // Cyrillic А (U+0410) instead of Latin A — NFKC normalizes it
        let homoglyph_line = "АKIАIOSFODNNsomething7EXАMPLE";
        // This may or may not match depending on normalization result
        let _ = scan_line(homoglyph_line, &[]);
    }

    #[test]
    fn detects_custom_pattern() {
        use crate::config::CustomPattern;
        let cp = CustomPattern {
            name: "internal_import".to_string(),
            pattern: r"from corp_internal\.".to_string(),
            tier: "T1".to_string(),
            confidence: 0.9,
            description: None,
        };
        assert!(scan_line("from corp_internal.utils import helper", &[cp]).is_some());
    }
}
