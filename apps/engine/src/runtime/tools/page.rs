//! read_page: the text of one public web page. Private and local addresses are refused.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::{Value, json};

use crate::core::config::ReadPage as ReadPageConfig;
use crate::core::traits::tools::Tool;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::knowledge::evidence::Citation;
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};
use crate::runtime::tools::http;

/// Name of the page reading tool.
pub const READ_PAGE: &str = "read_page";

/// Reads the text of a public web page.
pub struct ReadPageTool {
    cfg: Arc<Settings>,
}

/// What one read may fetch.
struct Settings {
    timeout: Duration,
    max_bytes: usize,
    max_chars: usize,
    max_redirects: usize,
    user_agent: String,
}

#[async_trait]
impl Tool for ReadPageTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: READ_PAGE.to_owned(),
            description: "Read the text of a public web page by its URL. Use it for any link the \
                          user gives you and for a page a search result points at."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "The full http or https URL of the page."}
                },
                "required": ["url"]
            }),
            risk: RiskClass::ReadPublic,
            private: false,
            sequential: false,
            timeout_secs: None,
        }
    }

    async fn call(&self, _ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let raw = args
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .ok_or_else(|| ToolError::InvalidArguments("url is required".into()))?;
        let url = parse(raw)?;
        let (url, body, kind) = self.fetch(url).await?;
        let (title, text) = if kind.contains("html") || looks_like_html(&body) {
            (html_title(&body), html_text(&body))
        } else if kind.starts_with("text/") || kind.contains("json") || kind.is_empty() {
            (None, collapse(&body))
        } else {
            return Err(ToolError::Failed(format!(
                "{url} is {kind}, not a page with text"
            )));
        };
        let text = cut(&text, self.cfg.max_chars);
        let title = title.unwrap_or_else(|| host_of(&url));
        Ok(ToolOutput {
            content: if text.is_empty() {
                format!("{title} ({url}) has no readable text. It may need a browser to load.")
            } else {
                format!("{title} ({url}):\n\n{text}")
            },
            data: None,
            sources: vec![Citation {
                key: READ_PAGE.to_owned(),
                title,
                url: Some(url.to_string()),
            }],
        })
    }
}

impl ReadPageTool {
    /// Fetches url, following redirects, with every hop checked to be public.
    async fn fetch(
        &self,
        mut url: reqwest::Url,
    ) -> Result<(reqwest::Url, String, String), ToolError> {
        for _ in 0..=self.cfg.max_redirects {
            let addr = public_addr(&url).await?;
            let host = url.host_str().unwrap_or_default().to_owned();
            let client = reqwest::Client::builder()
                .timeout(self.cfg.timeout)
                .redirect(reqwest::redirect::Policy::none())
                .resolve(&host, addr)
                .build()
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            let response = client
                .get(url.clone())
                .header(reqwest::header::USER_AGENT, &self.cfg.user_agent)
                .header(
                    reqwest::header::ACCEPT,
                    "text/html,text/plain;q=0.9,*/*;q=0.5",
                )
                .send()
                .await
                .map_err(|e| ToolError::Failed(format!("{url}: {}", http::failure(&e))))?;
            let status = response.status();
            if status.is_redirection() {
                let next = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|loc| url.join(loc).ok())
                    .ok_or_else(|| ToolError::Failed(format!("{url} redirects nowhere")))?;
                url = parse(next.as_str())?;
                continue;
            }
            if !status.is_success() {
                return Err(ToolError::Failed(format!(
                    "{url} returned {}",
                    status.as_u16()
                )));
            }
            let kind = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let body = self.read_body(response).await?;
            return Ok((url, body, kind));
        }
        Err(ToolError::Failed(format!("{url} redirects too many times")))
    }

    /// The body as text, cut at max_bytes.
    async fn read_body(&self, response: reqwest::Response) -> Result<String, ToolError> {
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ToolError::Failed(http::failure(&e).to_owned()))?;
            let room = self.cfg.max_bytes.saturating_sub(bytes.len());
            bytes.extend_from_slice(&chunk[..chunk.len().min(room)]);
            if bytes.len() >= self.cfg.max_bytes {
                break;
            }
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// An http or https URL on a default port, or why not.
fn parse(raw: &str) -> Result<reqwest::Url, ToolError> {
    let with_scheme = if raw.contains("://") {
        raw.to_owned()
    } else {
        format!("https://{raw}")
    };
    let url = reqwest::Url::parse(&with_scheme)
        .map_err(|_| ToolError::InvalidArguments(format!("{raw} is not a URL")))?;
    let scheme_ok = matches!(url.scheme(), "http" | "https");
    let port_ok = matches!(url.port_or_known_default(), Some(80 | 443));
    if !scheme_ok || !port_ok || url.host_str().is_none() || !url.username().is_empty() {
        return Err(ToolError::InvalidArguments(format!(
            "{raw} is not a public http or https page"
        )));
    }
    Ok(url)
}

/// One public address the host of url resolves to. Refused when any address is not public.
async fn public_addr(url: &reqwest::Url) -> Result<SocketAddr, ToolError> {
    let host = url.host_str().unwrap_or_default();
    let port = url.port_or_known_default().unwrap_or(443);
    let found: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| ToolError::Failed(format!("{host} does not resolve")))?
        .collect();
    if found.is_empty() || found.iter().any(|a| !is_public(a.ip())) {
        return Err(ToolError::Failed(format!("{host} is not a public site")));
    }
    found
        .into_iter()
        .next()
        .ok_or_else(|| ToolError::Failed(format!("{host} does not resolve")))
}

/// Whether ip is a public internet address.
pub(crate) fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                || v4.is_multicast()
                || a == 0
                || a >= 240
                || (a == 100 && (64..128).contains(&b))
                || (a == 192 && b == 0 && c == 0)
                || (a == 198 && (b == 18 || b == 19)))
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || first == 0x2001 && v6.segments()[1] == 0x0db8
                || first == 0x0064
                || first == 0)
        }
    }
}

/// Whether a body without a content type is HTML.
fn looks_like_html(body: &str) -> bool {
    let start: String = body
        .chars()
        .take(512)
        .collect::<String>()
        .to_ascii_lowercase();
    start.contains("<html") || start.contains("<!doctype html")
}

/// The text of the title element, when the page has one.
pub(crate) fn html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = lower.find("<title")?;
    let start = open + lower[open..].find('>')? + 1;
    let end = start + lower[start..].find("</title")?;
    let title = collapse(&entities(html.get(start..end)?));
    (!title.is_empty()).then_some(title)
}

/// The visible text of an HTML page: no scripts, styles or tags, entities decoded, spaces collapsed.
pub(crate) fn html_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 4);
    let lower = html.to_ascii_lowercase();
    let mut i = 0;
    while i < html.len() {
        let Some(rel) = lower[i..].find('<') else {
            out.push_str(html.get(i..).unwrap_or_default());
            break;
        };
        out.push_str(html.get(i..i + rel).unwrap_or_default());
        let tag_start = i + rel;
        let skipped = ["script", "style", "noscript", "svg", "template", "head"]
            .iter()
            .find(|name| {
                lower[tag_start + 1..].starts_with(*name)
                    && lower[tag_start + 1 + name.len()..]
                        .starts_with(|c: char| c == '>' || c.is_whitespace())
            });
        if let Some(name) = skipped {
            let close = format!("</{name}");
            match lower[tag_start..].find(&close) {
                Some(end) => {
                    let after = tag_start + end;
                    i = lower[after..]
                        .find('>')
                        .map_or(html.len(), |gt| after + gt + 1);
                }
                None => break,
            }
            out.push(' ');
            continue;
        }
        match lower[tag_start..].find('>') {
            Some(gt) => {
                let tag = &lower[tag_start..tag_start + gt];
                let block = [
                    "<p", "</p", "<br", "<div", "</div", "<li", "<h", "</h", "<tr", "</tr",
                ]
                .iter()
                .any(|b| tag.starts_with(b));
                out.push(if block { '\n' } else { ' ' });
                i = tag_start + gt + 1;
            }
            None => break,
        }
    }
    let decoded = entities(&out);
    decoded
        .lines()
        .map(collapse)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Decodes the common HTML entities.
fn entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&mdash;", "-")
        .replace("&ndash;", "-")
        .replace("&amp;", "&")
}

/// Runs of whitespace as one space, trimmed.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// At most max characters of text, marked when cut.
fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    text.chars().take(max).collect::<String>() + " [cut]"
}

/// The host of url, for a page with no title.
fn host_of(url: &reqwest::Url) -> String {
    url.host_str()
        .unwrap_or_default()
        .trim_start_matches("www.")
        .to_owned()
}

/// The page reading tool, when it is enabled.
pub fn tools(cfg: &ReadPageConfig) -> Vec<Arc<dyn Tool>> {
    let tool: Arc<dyn Tool> = Arc::new(ReadPageTool {
        cfg: Arc::new(Settings {
            timeout: Duration::from_secs(cfg.timeout_secs),
            max_bytes: cfg.max_bytes,
            max_chars: cfg.max_chars,
            max_redirects: cfg.max_redirects,
            user_agent: cfg.user_agent.clone(),
        }),
    });
    vec![tool]
}
