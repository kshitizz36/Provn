use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use crate::config::Config;
use crate::policy::Verdict;
use crate::scanner::ScanResult;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Chain invalid: {0}")]
    Chain(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    pub seq: u64,
    pub timestamp: String,
    pub event: String,
    pub file: Option<String>,
    pub tier: Option<String>,
    pub layer: Option<String>,
    pub verdict: String,
    pub prev_hash: String,
    pub hmac: String,
}

fn get_or_create_hmac_key(key_path: &str) -> Vec<u8> {
    if let Ok(key) = fs::read(key_path) {
        return key;
    }
    // Generate a random 32-byte key
    let key: Vec<u8> = (0..32).map(|_| rand::random::<u8>()).collect();
    if let Some(parent) = Path::new(key_path).parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(key_path, &key).ok();
    key
}

fn read_hmac_key(key_path: &str) -> Result<Vec<u8>, AuditError> {
    fs::read(key_path).map_err(AuditError::Io)
}

fn hmac_sign(key: &[u8], data: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key valid");
    mac.update(data.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn read_last_entry(audit_path: &str) -> Option<AuditEntry> {
    let file = fs::File::open(audit_path).ok()?;
    let reader = BufReader::new(file);
    let mut last = None;
    for line in reader.lines().map_while(Result::ok) {
        if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line) {
            last = Some(entry);
        }
    }
    last
}

fn hash_entry(entry: &AuditEntry) -> String {
    use sha2::{Digest, Sha256};
    let data = serde_json::to_string(entry).unwrap_or_default();
    let digest = Sha256::digest(data.as_bytes());
    hex::encode(digest)
}

pub fn append(verdict: &Verdict, result: &ScanResult, cfg: &Config) -> Result<(), AuditError> {
    if !cfg.audit.enabled {
        return Ok(());
    }

    let audit_path = &cfg.audit.path;
    if let Some(parent) = Path::new(audit_path.as_str()).parent() {
        fs::create_dir_all(parent)?;
    }

    let key = get_or_create_hmac_key(&cfg.audit.hmac_key_path);

    let last = read_last_entry(audit_path);
    let seq = last.as_ref().map(|e| e.seq + 1).unwrap_or(0);
    let prev_hash = last.as_ref().map(hash_entry).unwrap_or_else(|| "genesis".to_string());
    let timestamp = Utc::now().to_rfc3339();

    let verdict_str = match verdict {
        Verdict::Allow => "ALLOW",
        Verdict::Warn(t) | Verdict::Block(t) => t.as_str(),
    };

    // Build entry without HMAC field first for signing
    let entry_payload = serde_json::json!({
        "seq": seq,
        "timestamp": timestamp,
        "event": format!("{:?}", verdict).split('(').next().unwrap_or("Unknown").to_uppercase(),
        "file": result.file,
        "tier": result.tier,
        "layer": result.layer,
        "verdict": verdict_str,
        "prev_hash": prev_hash,
    });

    let hmac = hmac_sign(&key, &entry_payload.to_string());

    let entry = AuditEntry {
        seq,
        timestamp,
        event: entry_payload["event"].as_str().unwrap_or("").to_string(),
        file: result.file.clone(),
        tier: result.tier.clone(),
        layer: result.layer.clone(),
        verdict: verdict_str.to_string(),
        prev_hash,
        hmac,
    };

    let mut file = OpenOptions::new().create(true).append(true).open(audit_path)?;
    writeln!(file, "{}", serde_json::to_string(&entry)?)?;

    Ok(())
}

pub fn verify_chain(audit_path: &str, hmac_key_path: &str) -> Result<usize, AuditError> {
    if !Path::new(audit_path).exists() {
        return Ok(0);
    }

    let key = read_hmac_key(hmac_key_path)?;

    let file = fs::File::open(audit_path)?;
    let reader = BufReader::new(file);
    let mut count = 0;
    let mut prev_hash = "genesis".to_string();

    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let entry: AuditEntry = serde_json::from_str(&line)?;

        if entry.prev_hash != prev_hash {
            return Err(AuditError::Chain(format!(
                "Hash chain broken at seq {} — expected prev_hash {}",
                entry.seq, prev_hash
            )));
        }

        // Verify HMAC
        let entry_payload = serde_json::json!({
            "seq": entry.seq,
            "timestamp": entry.timestamp,
            "event": entry.event,
            "file": entry.file,
            "tier": entry.tier,
            "layer": entry.layer,
            "verdict": entry.verdict,
            "prev_hash": entry.prev_hash,
        });
        let expected_hmac = hmac_sign(&key, &entry_payload.to_string());
        if entry.hmac != expected_hmac {
            return Err(AuditError::Chain(format!(
                "HMAC invalid at seq {}",
                entry.seq
            )));
        }

        prev_hash = hash_entry(&entry);
        count += 1;
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::{append, verify_chain};
    use crate::config::Config;
    use crate::policy::Verdict;
    use crate::scanner::ScanResult;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn appended_entries_verify_successfully() {
        let temp_dir = std::env::temp_dir().join(format!("provn-audit-{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        let mut cfg = Config::default();
        cfg.audit.path = temp_dir.join("audit.jsonl").display().to_string();
        cfg.audit.hmac_key_path = temp_dir.join("hmac.key").display().to_string();

        let t1_result = ScanResult {
            file: Some("bot.py".to_string()),
            line: Some(1),
            match_type: Some("ast_taint".to_string()),
            description: Some("Sensitive variable 'system_prompt' assigned string literal".to_string()),
            snippet: Some(
                "system_prompt = \"Use our proprietary ranking rubric and do not disclose it.\""
                    .to_string(),
            ),
            redacted: Some("PROVN_REDACTED_SYSTEM_PROMPT".to_string()),
            confidence: 0.70,
            layer: Some("ast".to_string()),
            tier: Some("T1".to_string()),
            latency_ms: 0,
        };

        let t2_result = ScanResult {
            file: Some("config.py".to_string()),
            line: Some(1),
            match_type: Some("high_entropy".to_string()),
            description: Some("High entropy token (H=5.00)".to_string()),
            snippet: Some("secret_token = \"x7Kp2mNqR9vT4wYjLhBcDfAeGiUoSzXnPqRsT\"".to_string()),
            redacted: Some("PROVN_REDACTED_HIGH_ENTROPY".to_string()),
            confidence: 0.66,
            layer: Some("entropy".to_string()),
            tier: Some("T2".to_string()),
            latency_ms: 0,
        };

        append(&Verdict::Block("T1".to_string()), &t1_result, &cfg).unwrap();
        append(&Verdict::Warn("T2".to_string()), &t2_result, &cfg).unwrap();

        let count = verify_chain(&cfg.audit.path, &cfg.audit.hmac_key_path).unwrap();
        assert_eq!(count, 2);

        fs::remove_dir_all(&temp_dir).unwrap();
    }
}
