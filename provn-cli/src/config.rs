use serde::Deserialize;
use std::fs;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(#[from] serde_yaml::Error),
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_mode")]
    pub mode: String,

    #[serde(default)]
    pub exclude_dirs: Vec<String>,

    #[serde(default)]
    pub exclude_files: Vec<String>,

    #[serde(default)]
    pub layers: LayersConfig,

    #[serde(default)]
    pub audit: AuditConfig,

    /// Glob patterns for allowed git remotes.
    /// Empty = allow all remotes (no restriction).
    /// Example: ["git@github.com:mycompany/*", "https://github.com/mycompany/*"]
    #[serde(default)]
    pub allowed_remotes: Vec<String>,
}

/// A user-defined regex pattern in provn.yml
#[derive(Debug, Deserialize, Clone)]
pub struct CustomPattern {
    /// Unique name shown in findings output
    pub name: String,
    /// Regex pattern (Rust regex syntax)
    pub pattern: String,
    /// Risk tier: T0 | T1 | T2 | T3
    #[serde(default = "default_tier_t1")]
    pub tier: String,
    /// Confidence score 0.0–1.0
    #[serde(default = "default_custom_confidence")]
    pub confidence: f64,
    /// Human-readable description shown when triggered
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct LayersConfig {
    #[serde(default)]
    pub regex: RegexConfig,
    #[serde(default)]
    pub entropy: EntropyConfig,
    #[serde(default)]
    pub ast: AstConfig,
    #[serde(default)]
    pub semantic: SemanticConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RegexConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    /// User-defined patterns — checked after built-in patterns
    #[serde(default)]
    pub custom_patterns: Vec<CustomPattern>,
}

impl Default for RegexConfig {
    fn default() -> Self {
        Self { enabled: true, custom_patterns: Vec::new() }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SemanticConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    /// GGUF filename or absolute path loaded by the local llama-server helper.
    #[serde(default = "default_semantic_model")]
    pub model: String,
    /// llama-server HTTP endpoint (run: llama-server -m provn-gemma4-e2b.gguf)
    #[serde(default = "default_semantic_endpoint")]
    pub endpoint: String,
    /// Max ms to wait before falling back to Layer 1/2 result
    #[serde(default = "default_semantic_timeout")]
    pub timeout_ms: u64,
    /// Fallback behavior when semantic inference is unavailable.
    #[serde(default = "default_semantic_fallback")]
    pub fallback: String,
    /// Lower bound of ambiguous confidence band that triggers Layer 3
    #[serde(default = "default_ambiguous_low")]
    pub ambiguous_low: f64,
    /// Upper bound — confident detections above this skip Layer 3
    #[serde(default = "default_ambiguous_high")]
    pub ambiguous_high: f64,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model: default_semantic_model(),
            endpoint: default_semantic_endpoint(),
            timeout_ms: default_semantic_timeout(),
            fallback: default_semantic_fallback(),
            ambiguous_low: default_ambiguous_low(),
            ambiguous_high: default_ambiguous_high(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct EntropyConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    #[serde(default = "default_entropy_threshold")]
    pub threshold: f64,
    #[serde(default = "default_min_length")]
    pub min_length: usize,
}

impl Default for EntropyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 4.5,
            min_length: 20,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct AstConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    #[serde(default = "default_sensitive_vars")]
    pub sensitive_vars: Vec<String>,
}

impl Default for AstConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sensitive_vars: default_sensitive_vars(),
        }
    }
}


#[derive(Debug, Deserialize, Clone)]
pub struct AuditConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    #[serde(default = "default_audit_path")]
    pub path: String,
    #[serde(default = "default_hmac_key_path")]
    pub hmac_key_path: String,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: default_audit_path(),
            hmac_key_path: default_hmac_key_path(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: "enforce".to_string(),
            exclude_dirs: vec![
                "node_modules".into(),
                ".git".into(),
                "dist".into(),
                "build".into(),
                "__pycache__".into(),
                ".venv".into(),
                "target".into(),
            ],
            exclude_files: vec!["*.lock".into(), "*.min.js".into(), "*.map".into()],
            layers: LayersConfig::default(),
            audit: AuditConfig::default(),
            allowed_remotes: Vec::new(),
        }
    }
}

pub fn load() -> Result<Config, ConfigError> {
    let content = match fs::read_to_string("provn.yml") {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => fs::read_to_string("aegis.yml")?,
        Err(err) => return Err(err.into()),
    };
    let cfg: Config = serde_yaml::from_str(&content)?;
    Ok(cfg)
}

// Serde defaults
fn default_mode() -> String { "enforce".to_string() }
fn bool_true() -> bool { true }
fn default_entropy_threshold() -> f64 { 4.5 }
fn default_min_length() -> usize { 20 }
fn default_sensitive_vars() -> Vec<String> {
    vec![
        "system_prompt".into(), "api_key".into(), "secret".into(),
        "password".into(), "token".into(), "private_key".into(), "credentials".into(),
    ]
}
fn default_audit_path() -> String { ".provn/audit.jsonl".to_string() }
fn default_hmac_key_path() -> String { ".provn/hmac.key".to_string() }
fn default_semantic_model() -> String { "provn-gemma4-e2b.gguf".to_string() }
fn default_semantic_endpoint() -> String { "http://localhost:8080".to_string() }
fn default_semantic_timeout() -> u64 { 2000 }
fn default_semantic_fallback() -> String { "layer1".to_string() }
fn default_ambiguous_low() -> f64 { 0.4 }
fn default_ambiguous_high() -> f64 { 0.8 }
fn default_tier_t1() -> String { "T1".to_string() }
fn default_custom_confidence() -> f64 { 0.80 }
