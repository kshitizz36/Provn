use std::fs;
use thiserror::Error;
use crate::scanner::ScanResult;

#[derive(Debug, Error)]
pub enum RedactError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("No file to redact")]
    NoFile,
    #[error("No snippet to redact")]
    NoSnippet,
}

/// Apply redaction to the file on disk and re-stage it with `git add`.
pub fn apply_redaction(result: &ScanResult) -> Result<(), RedactError> {
    let file_path = result.file.as_deref().ok_or(RedactError::NoFile)?;
    let snippet = result.snippet.as_deref().ok_or(RedactError::NoSnippet)?;
    let replacement = result.redacted.as_deref().unwrap_or("PROVN_REDACTED");

    let content = fs::read_to_string(file_path)?;

    // Replace the first occurrence of the secret snippet
    let trimmed_snippet = snippet.trim_start_matches(['+', '-', ' ']);
    let new_content = content.replacen(trimmed_snippet, replacement, 1);

    if new_content == content {
        // Couldn't find exact match — skip
        return Ok(());
    }

    fs::write(file_path, &new_content)?;

    // Re-stage the file
    std::process::Command::new("git")
        .args(["add", file_path])
        .output()
        .ok();

    Ok(())
}
