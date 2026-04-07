use std::time::Duration;
use serde::Deserialize;

pub struct SemanticResult {
    pub label: String,  // "leak" or "clean"
    pub skipped: bool,  // true if server unavailable or timed out
}

const SYSTEM: &str = "You are a code security classifier. \
Respond with exactly one word: leak or clean. No explanation.";

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

/// Call the local llama-server `/v1/chat/completions` endpoint.
/// Returns `skipped=true` when the server is unreachable, returns an
/// unexpected response, or exceeds `timeout_ms`.
pub fn classify(code: &str, endpoint: &str, timeout_ms: u64) -> SemanticResult {
    // Derive the chat completions URL from whatever endpoint is configured.
    // Support both bare base URLs (http://host:port) and explicit paths.
    let url = if endpoint.ends_with("/v1/chat/completions") {
        endpoint.to_string()
    } else {
        // Strip any trailing path and append the standard route.
        let base = endpoint
            .trim_end_matches('/')
            .trim_end_matches("/completion");
        format!("{}/v1/chat/completions", base)
    };

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
    {
        Ok(c) => c,
        Err(_) => return skipped(),
    };

    let body = serde_json::json!({
        "messages": [
            {"role": "system", "content": SYSTEM},
            {"role": "user",   "content": format!("Classify:\n```\n{code}\n```")},
        ],
        "temperature": 0.0,
        "max_tokens": 500,
    });

    match client.post(&url).json(&body).send() {
        Ok(resp) => match resp.json::<ChatResponse>() {
            Ok(cr) => match cr.choices.first() {
                Some(choice) => match parse_label(&choice.message.content) {
                    Some(label) => SemanticResult { label: label.into(), skipped: false },
                    None => skipped(),
                },
                None => skipped(),
            },
            Err(_) => skipped(),
        },
        Err(_) => skipped(),
    }
}

fn skipped() -> SemanticResult {
    SemanticResult { label: "clean".into(), skipped: true }
}

fn parse_label(content: &str) -> Option<&'static str> {
    let text = content.trim().to_lowercase();
    if text.starts_with("leak") {
        Some("leak")
    } else if text.starts_with("clean") {
        Some("clean")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::parse_label;

    #[test]
    fn accepts_leak_prefix() {
        assert_eq!(parse_label("leak"), Some("leak"));
        assert_eq!(parse_label("Leak detected"), Some("leak"));
    }

    #[test]
    fn accepts_clean_prefix() {
        assert_eq!(parse_label("clean"), Some("clean"));
        assert_eq!(parse_label("clean\n"), Some("clean"));
    }

    #[test]
    fn rejects_unexpected_tokens() {
        assert_eq!(parse_label("<unused25><unused25>"), None);
        assert_eq!(parse_label(""), None);
    }
}
