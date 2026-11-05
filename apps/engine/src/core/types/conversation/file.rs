//! FileAttachment: a document the caller attached, opened in the sandbox rather than sent to the model.

use serde::{Deserialize, Serialize};

/// One non-image file attached to a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileAttachment {
    /// Direct link to the file.
    pub url: String,
    /// File name as the caller uploaded it.
    pub name: String,
    /// Media type the edge reported.
    #[serde(default)]
    pub media_type: String,
    /// Size in bytes the edge reported.
    #[serde(default)]
    pub size: u64,
    /// The file itself, base64, when the caller sends it inline instead of a link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

impl FileAttachment {
    /// The files of a request no larger than max_bytes, at most most of them.
    pub fn accepted(files: Vec<Self>, most: usize, max_bytes: u64) -> Vec<Self> {
        files
            .into_iter()
            .filter(|f| match &f.data {
                Some(data) => (data.len() as u64 / 4).saturating_mul(3) <= max_bytes,
                None => !f.url.is_empty() && f.size <= max_bytes,
            })
            .take(most)
            .collect()
    }

    /// The name as a workspace file name, sanitized and length-capped.
    pub fn safe_name(&self) -> String {
        let mut cleaned = String::new();
        for c in self.name.chars() {
            let c = if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            };
            if !(c == '.' && cleaned.ends_with('.')) {
                cleaned.push(c);
            }
        }
        let trimmed = cleaned.trim_start_matches('.');
        if trimmed.is_empty() {
            return "file".to_owned();
        }
        let (stem, ext) = match trimmed.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() && ext.len() <= 8 => (stem, ext),
            _ => (trimmed, ""),
        };
        let stem: String = stem.chars().take(38).collect();
        if ext.is_empty() {
            stem
        } else {
            format!("{stem}.{ext}")
        }
    }
}

/// Why an attached file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    /// The link is not on a host files are fetched from.
    #[error("{0} is not a host attachments are read from")]
    Host(String),
    /// The file is larger than allowed.
    #[error("the file is larger than {0} bytes")]
    TooLarge(u64),
    /// The download failed.
    #[error("download failed: {0}")]
    Fetch(String),
}
