//! Files the caller attached, copied into the sandbox workspace before the first model call.

use base64::Engine as _;

use super::Agent;
use super::execute::workspace_session;
use super::read;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::file::FileAttachment;
use crate::core::types::tools::sandbox::SandboxRequest;
use crate::core::types::trace::TraceEvent;

/// Words too common to find a passage by.
const COMMON: [&str; 40] = [
    "the", "and", "for", "are", "was", "were", "what", "when", "where", "which", "who", "why",
    "how", "does", "did", "this", "that", "these", "those", "with", "from", "into", "about",
    "have", "has", "had", "you", "your", "can", "could", "would", "should", "tell", "please",
    "file", "form", "document", "attached", "there", "they",
];

/// The words of question a passage is ranked by: lowercase, three letters or more, not common.
pub fn terms(question: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in question
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
    {
        if word.len() >= 3 && !COMMON.contains(&word) && !out.iter().any(|w| w == word) {
            out.push(word.to_owned());
        }
        if out.len() == 12 {
            break;
        }
    }
    out
}

/// The sandbox command that turns an uploaded file at path into text beside it at path.txt.
pub fn to_text(
    path: &str,
    terms: &[String],
    preview: usize,
    matches: usize,
    ocr_pages: u32,
) -> String {
    let words = terms.join(" ");
    format!(
        r#"f={path}; t={path}.txt; : > "$t"
case "$(printf %s "$f" | tr A-Z a-z)" in
*.pdf) pdftotext -layout "$f" "$t" 2>/dev/null
  if [ "$(tr -d '[:space:]' < "$t" | wc -c)" -lt 10 ]; then
    pdftoppm -r 150 -png -l {ocr_pages} "$f" /tmp/ocr-page 2>/dev/null
    for p in /tmp/ocr-page*.png; do [ -e "$p" ] && tesseract "$p" - 2>/dev/null; done > "$t"
    rm -f /tmp/ocr-page*.png; echo OCR
  fi ;;
*.docx) python3 -c 'import docx,sys
d=docx.Document(sys.argv[1])
print("\n".join(p.text for p in d.paragraphs))
for tb in d.tables:
    for r in tb.rows: print("\t".join(c.text for c in r.cells))' "$f" > "$t" 2>/dev/null ;;
*.xlsx|*.xls) python3 -c 'import pandas,sys
for n,df in pandas.read_excel(sys.argv[1],sheet_name=None).items(): print("sheet: "+str(n)); print(df.to_csv(index=False))' "$f" > "$t" 2>/dev/null ;;
*) file -b --mime-type "$f" | grep -q -e '^text/' -e json -e xml && cp "$f" "$t" ;;
esac
echo "CHARS $(wc -c < "$t")"
echo "=== PREVIEW"
head -c {preview} "$t"
echo
echo "=== MATCHES"
python3 -c 'import sys
lines=open(sys.argv[1],errors="replace").read().splitlines()
terms=sys.argv[3:]; limit=int(sys.argv[2])
ranked=[]
for i in range(len(lines)):
    window=" ".join(lines[max(0,i-1):i+3]).lower()
    score=sum(1 for w in terms if w in window)
    if score: ranked.append((-score,i))
ranked.sort()
kept=[]; used=0
for _,i in ranked:
    if any(abs(i-j)<5 for j in kept): continue
    block="\n".join(l.rstrip() for l in lines[max(0,i-1):i+5]).strip()
    if used+len(block)>limit: continue
    kept.append(i); used+=len(block)
    print("[line "+str(i+1)+"]\n"+block+"\n")
    if len(kept)==8: break' "$t" {matches} {words}"#
    )
}

/// What the conversion printed: OCR use, character count, preview, and matching passages.
pub fn converted(stdout: &str) -> Converted {
    let ocr = stdout.lines().next().is_some_and(|l| l.trim() == "OCR");
    let chars = stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix("CHARS "))
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or(0);
    let after = |marker: &str| stdout.split_once(marker).map_or("", |(_, rest)| rest);
    let preview = after("=== PREVIEW\n")
        .split_once("\n=== MATCHES")
        .map_or("", |(head, _)| head);
    let matches = after("=== MATCHES\n");
    (
        ocr,
        chars,
        preview.trim().to_owned(),
        matches.trim().to_owned(),
    )
}

/// What converting a file found: OCR used, characters, preview, and matching passages.
pub type Converted = (bool, usize, String, String);

/// One line naming a file: its name, type and size.
fn described(template: &str, file: &FileAttachment, bytes: usize) -> String {
    template
        .replace("{name}", &file.name)
        .replace("{kind}", kind(file))
        .replace("{size}", &size(bytes))
}

fn kind(file: &FileAttachment) -> &str {
    if file.media_type.is_empty() {
        "unknown type"
    } else {
        &file.media_type
    }
}

/// A byte count as a person reads it.
fn size(bytes: usize) -> String {
    #[allow(clippy::cast_precision_loss)]
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        format!("{kb:.0} KB")
    } else {
        format!("{:.1} MB", kb / 1024.0)
    }
}

/// What became of a file once its bytes were in hand.
enum Opened {
    /// Written to the sandbox workspace at path and converted there, when that worked.
    Sandbox(String, Option<Converted>),
    /// Read in the engine.
    Inline(Result<String, read::Unread>),
}

impl Agent {
    /// Copies every attached file into the workspace and returns the prompt lines naming them.
    pub(super) async fn uploads(&self, ctx: &RequestContext, input: &str) -> Vec<String> {
        let mut lines = Vec::new();
        let session = workspace_session(ctx);
        for file in &ctx.files {
            let opened = self.upload(ctx, &session, file, input).await;
            let path = match &opened {
                Ok((_, Opened::Sandbox(path, _))) => Some(path.clone()),
                _ => None,
            };
            let error = match &opened {
                Err(reason) => Some(reason.clone()),
                Ok((_, Opened::Inline(Err(unread)))) => Some(unread_reason(unread).to_owned()),
                Ok(_) => None,
            };
            self.deps.trace.emit(
                ctx,
                TraceEvent::FileAttached {
                    name: file.name.clone(),
                    bytes: opened.as_ref().map_or(0, |(bytes, _)| *bytes),
                    path,
                    error: error.clone(),
                },
            );
            if let Some(reason) = &error {
                tracing::warn!(file = %file.name, %reason, "attached file not opened");
            }
            let line = match opened {
                Ok((bytes, Opened::Sandbox(path, text))) => {
                    self.sandbox_line(file, bytes, &path, &session, text)
                }
                Ok((bytes, Opened::Inline(Ok(text)))) => {
                    let matches = read::passages(&text, &terms(input), self.cfg.upload_match_chars);
                    described(&self.prompt.upload_inline_line, file, bytes)
                        .replace("{chars}", &text.chars().count().to_string())
                        .replace(
                            "{preview}",
                            &read::preview(&text, self.cfg.upload_preview_chars),
                        )
                        .replace("{matches}", &or_none(matches))
                }
                Ok((_, Opened::Inline(Err(unread)))) => {
                    described(&self.prompt.upload_failed_line, file, 0)
                        .replace("{reason}", unread_reason(&unread))
                }
                Err(reason) => {
                    described(&self.prompt.upload_failed_line, file, 0).replace("{reason}", &reason)
                }
            };
            lines.push(line);
        }
        lines
    }

    /// The prompt line for a file opened in the sandbox.
    fn sandbox_line(
        &self,
        file: &FileAttachment,
        bytes: usize,
        path: &str,
        session: &str,
        text: Option<Converted>,
    ) -> String {
        let template = match &text {
            Some((_, chars, _, _)) if *chars > 0 => &self.prompt.upload_line,
            _ => &self.prompt.upload_raw_line,
        };
        let (ocr, chars, preview, matches) = text.unwrap_or_default();
        let read = if ocr { " (read by OCR)" } else { "" };
        described(template, file, bytes)
            .replace("{path}", path)
            .replace("{text_path}", &format!("{path}.txt"))
            .replace("{chars}", &format!("{chars}{read}"))
            .replace("{session}", session)
            .replace("{preview}", &preview)
            .replace("{matches}", &or_none(matches))
    }

    /// The bytes of a file: decoded when sent inline, else downloaded.
    async fn bytes(&self, ctx: &RequestContext, file: &FileAttachment) -> Result<Vec<u8>, String> {
        if let Some(data) = &file.data {
            return base64::engine::general_purpose::STANDARD
                .decode(data.trim())
                .map_err(|_| "the file was not sent whole".to_owned());
        }
        let Some(files) = &self.deps.files else {
            return Err("files cannot be opened here".to_owned());
        };
        files.fetch(ctx, file).await.map_err(|e| e.to_string())
    }

    /// Gets one file and opens it in the sandbox when there is one, else reads it in the engine.
    async fn upload(
        &self,
        ctx: &RequestContext,
        session: &str,
        file: &FileAttachment,
        input: &str,
    ) -> Result<(usize, Opened), String> {
        let bytes = self.bytes(ctx, file).await?;
        let size = bytes.len();
        let sandbox = self.deps.sandbox.as_ref().filter(|s| s.enabled());
        let Some(sandbox) = sandbox else {
            let owned = file.clone();
            let text = tokio::task::spawn_blocking(move || read::text(&owned, &bytes))
                .await
                .unwrap_or(Err(read::Unread::NoText));
            return Ok((size, Opened::Inline(text)));
        };
        let name = format!("upload-{}", file.safe_name());
        if !sandbox.converts() {
            let path = sandbox
                .put(ctx, session, &name, &bytes)
                .await
                .map_err(|e| e.to_string())?;
            let owned = file.clone();
            let text = tokio::task::spawn_blocking(move || read::text(&owned, &bytes))
                .await
                .ok()
                .and_then(Result::ok);
            // The prompt points at a text copy beside the file, as the converters leave one.
            if let Some(text) = &text
                && let Err(error) = sandbox
                    .put(ctx, session, &format!("{name}.txt"), text.as_bytes())
                    .await
            {
                tracing::warn!(file = %file.name, %error, "text copy not written");
            }
            let text = text.map(|text| {
                (
                    false,
                    text.chars().count(),
                    read::preview(&text, self.cfg.upload_preview_chars),
                    read::passages(&text, &terms(input), self.cfg.upload_match_chars),
                )
            });
            return Ok((size, Opened::Sandbox(path, text)));
        }
        let path = sandbox
            .put(ctx, session, &name, &bytes)
            .await
            .map_err(|e| e.to_string())?;
        let request = SandboxRequest {
            command: to_text(
                &path,
                &terms(input),
                self.cfg.upload_preview_chars,
                self.cfg.upload_match_chars,
                self.cfg.upload_ocr_pages,
            ),
            session: Some(session.to_owned()),
        };
        let text = match sandbox.run(ctx, &request).await {
            Ok(out) if out.exit_code == 0 => Some(converted(&out.stdout)),
            Ok(out) => {
                tracing::warn!(file = %file.name, stderr = %out.stderr, "attached file not converted");
                None
            }
            Err(error) => {
                tracing::warn!(file = %file.name, %error, "attached file not converted");
                None
            }
        };
        Ok((size, Opened::Sandbox(path, text)))
    }
}

/// The passages, or a note that none held the question's words.
fn or_none(matches: String) -> String {
    if matches.is_empty() {
        "(no passage holds the words of the question)".to_owned()
    } else {
        matches
    }
}

/// Why a file read in the engine gave no text, as the prompt says it.
fn unread_reason(unread: &read::Unread) -> &'static str {
    match unread {
        read::Unread::Kind => "only PDF, text, CSV, JSON and code files can be read here",
        read::Unread::NoText => "no text could be pulled from it, it may be a scan",
    }
}
