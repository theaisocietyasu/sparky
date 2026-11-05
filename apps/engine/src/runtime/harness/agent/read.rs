//! Text from an attached file read in the engine, for hosts with no sandbox.

use std::fmt::Write as _;

use crate::core::types::conversation::file::FileAttachment;

/// Extensions read as plain text.
const TEXT: [&str; 24] = [
    "txt", "md", "csv", "tsv", "json", "xml", "yaml", "yml", "toml", "html", "htm", "css", "js",
    "ts", "tsx", "py", "rs", "java", "c", "cpp", "h", "go", "sql", "tex",
];

/// Why no text came out of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unread {
    /// The type is not one the engine reads.
    Kind,
    /// A PDF with no text layer, such as a scan.
    NoText,
}

fn extension(file: &FileAttachment) -> String {
    file.name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

fn is_pdf(file: &FileAttachment) -> bool {
    file.media_type.starts_with("application/pdf") || extension(file) == "pdf"
}

fn is_text(file: &FileAttachment) -> bool {
    let kind = file.media_type.to_ascii_lowercase();
    kind.starts_with("text/")
        || ["json", "xml", "yaml", "csv", "javascript"]
            .iter()
            .any(|k| kind.contains(k))
        || TEXT.contains(&extension(file).as_str())
}

/// The text of a PDF or text file. Blocking: a PDF is parsed here.
pub fn text(file: &FileAttachment, bytes: &[u8]) -> Result<String, Unread> {
    let text = if is_pdf(file) {
        pdf_extract::extract_text_from_mem(bytes).map_err(|_| Unread::NoText)?
    } else if is_text(file) && !bytes.contains(&0) {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        return Err(Unread::Kind);
    };
    if text.trim().is_empty() {
        return Err(Unread::NoText);
    }
    Ok(text)
}

/// The start of text, at most preview characters.
pub fn preview(text: &str, preview: usize) -> String {
    text.trim().chars().take(preview).collect()
}

/// Up to eight passages of text holding the most terms, within limit characters, best first.
pub fn passages(text: &str, terms: &[String], limit: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut ranked: Vec<(usize, usize)> = Vec::new();
    for i in 0..lines.len() {
        let window = lines[i.saturating_sub(1)..(i + 3).min(lines.len())]
            .join(" ")
            .to_lowercase();
        let score = terms.iter().filter(|t| window.contains(t.as_str())).count();
        if score > 0 {
            ranked.push((score, i));
        }
    }
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut kept: Vec<usize> = Vec::new();
    let mut used = 0;
    let mut out = String::new();
    for (_, i) in ranked {
        if kept.iter().any(|&j| i.abs_diff(j) < 5) {
            continue;
        }
        let block = lines[i.saturating_sub(1)..(i + 5).min(lines.len())]
            .iter()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n");
        let block = block.trim();
        if used + block.len() > limit {
            continue;
        }
        kept.push(i);
        used += block.len();
        let _ = write!(out, "[line {}]\n{block}\n\n", i + 1);
        if kept.len() == 8 {
            break;
        }
    }
    out.trim_end().to_owned()
}
