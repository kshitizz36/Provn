use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;
use crate::config::Config;

#[derive(Debug, Error)]
pub enum DiffError {
    #[error("git command failed: {0}")]
    Git(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct DiffChunk {
    pub file: PathBuf,
    pub extension: String,
    pub added_lines: Vec<(usize, String)>,
}

pub fn parse_staged_diff(cfg: &Config) -> Result<Vec<DiffChunk>, DiffError> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--unified=0"])
        .output()?;

    if !output.status.success() {
        return Err(DiffError::Git(String::from_utf8_lossy(&output.stderr).into_owned()));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    parse_diff_text(&text, cfg)
}

pub fn parse_file(path: &str, cfg: &Config) -> Result<Vec<DiffChunk>, DiffError> {
    let content = std::fs::read_to_string(path)?;
    let ext = PathBuf::from(path)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();

    // Synthesize fake added lines (all lines are "added" when checking a file directly)
    let added_lines: Vec<(usize, String)> = content
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.to_string()))
        .collect();

    let chunk = DiffChunk {
        file: PathBuf::from(path),
        extension: ext,
        added_lines,
    };

    if should_skip(&chunk.file, cfg) {
        return Ok(vec![]);
    }

    Ok(vec![chunk])
}

fn parse_diff_text(text: &str, cfg: &Config) -> Result<Vec<DiffChunk>, DiffError> {
    let mut chunks: Vec<DiffChunk> = Vec::new();
    let mut current_file: Option<PathBuf> = None;
    let mut current_lines: Vec<(usize, String)> = Vec::new();
    let mut current_line_num: usize = 0;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            // Save previous chunk
            if let Some(file) = current_file.take() {
                if !current_lines.is_empty() && !should_skip(&file, cfg) {
                    let ext = file
                        .extension()
                        .map(|e| e.to_string_lossy().to_string())
                        .unwrap_or_default();
                    chunks.push(DiffChunk {
                        file,
                        extension: ext,
                        added_lines: std::mem::take(&mut current_lines),
                    });
                } else {
                    current_lines.clear();
                }
            }
            current_file = Some(PathBuf::from(rest));
            current_line_num = 0;
        } else if line.starts_with("@@ ") {
            // Parse hunk header: @@ -a,b +c,d @@
            if let Some(new_info) = line.split('+').nth(1) {
                let num_str = new_info.split(',').next().unwrap_or("0").split(' ').next().unwrap_or("0");
                current_line_num = num_str.parse().unwrap_or(0);
            }
        } else if let Some(added) = line.strip_prefix('+') {
            if current_file.is_some() {
                let content = added.to_string();
                // Respect provn:skip-file annotation, plus legacy aegis:skip-file.
                if content.contains("provn:skip-file") || content.contains("aegis:skip-file") {
                    current_lines.clear();
                    current_file = None;
                    continue;
                }
                // Skip provn:allow lines, plus legacy aegis:allow.
                if !content.contains("provn:allow") && !content.contains("aegis:allow") {
                    current_lines.push((current_line_num, content));
                }
                current_line_num += 1;
            }
        } else if (line.starts_with(' ') || line.starts_with('-')) && !line.starts_with('-') {
            current_line_num += 1;
        }
    }

    // Push last chunk
    if let Some(file) = current_file {
        if !current_lines.is_empty() && !should_skip(&file, cfg) {
            let ext = file
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();
            chunks.push(DiffChunk {
                file,
                extension: ext,
                added_lines: current_lines,
            });
        }
    }

    Ok(chunks)
}

fn should_skip(path: &std::path::Path, cfg: &Config) -> bool {
    let path_str = path.to_string_lossy();

    // Skip excluded dirs
    for dir in &cfg.exclude_dirs {
        if path_str.contains(dir.as_str()) {
            return true;
        }
    }

    // Skip excluded file patterns (simple glob: leading/trailing *)
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    for pattern in &cfg.exclude_files {
        if let Some(suffix) = pattern.strip_prefix('*') {
            if filename.ends_with(suffix) {
                return true;
            }
        } else if let Some(prefix) = pattern.strip_suffix('*') {
            if filename.starts_with(prefix) {
                return true;
            }
        } else if &filename == pattern {
            return true;
        }
    }

    false
}
