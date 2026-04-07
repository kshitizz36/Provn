use crate::scanner::ScanResult;

#[derive(Debug, Clone)]
pub enum Verdict {
    Allow,
    Warn(String),
    Block(String),
}

pub fn determine_verdict(result: &ScanResult, _cfg: &crate::config::Config) -> Verdict {
    if result.confidence == 0.0 || result.tier.is_none() {
        return Verdict::Allow;
    }

    let tier = result.tier.as_deref().unwrap_or("T3");

    match tier {
        "T0" => {
            // Hard block, no bypass
            Verdict::Block("T0".to_string())
        }
        "T1" => {
            // Block with bypass allowed
            if result.confidence >= 0.7 {
                Verdict::Block("T1".to_string())
            } else {
                Verdict::Warn("T1".to_string())
            }
        }
        "T2" => {
            // Warn only
            Verdict::Warn("T2".to_string())
        }
        "T3" => {
            // Log only — treat as allow
            Verdict::Allow
        }
        _ => Verdict::Allow,
    }
}

