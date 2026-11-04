//! Masking and shortening of values on their way into a trace or a span attribute.

use serde_json::Value;

/// Cuts text to at most max bytes on a char boundary, marking the cut.
pub(crate) fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        text.to_owned()
    } else {
        let mut end = max;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &text[..end])
    }
}

/// Stands in for the result of an authenticated tool: only its length reaches a trace.
pub(crate) fn masked(text: &str) -> String {
    format!("[authenticated result, {} chars]", text.chars().count())
}

/// Masks values that follow a secret-looking key in free text.
pub(crate) fn redact_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        let secret = SECRET_KEYS
            .iter()
            .filter_map(|needle| lower.find(needle).map(|at| at + needle.len()))
            .min();
        match secret.and_then(|after| {
            line[after..]
                .find([':', '='])
                .map(|sep| after + sep + 1)
                .filter(|cut| !line[*cut..].trim().is_empty())
        }) {
            Some(cut) => {
                out.push_str(&line[..cut]);
                out.push_str(" [redacted]");
            }
            None => out.push_str(line),
        }
        out.push('\n');
    }
    if !text.ends_with('\n') {
        out.pop();
    }
    out
}

/// Key fragments that mark a value as a secret, in arguments and in text alike.
const SECRET_KEYS: [&str; 6] = [
    "password",
    "token",
    "secret",
    "cookie",
    "authorization",
    "api_key",
];

/// Drops argument values whose key looks like a secret before they reach the trace.
pub(crate) fn redact(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, inner)| {
                    let lower = key.to_ascii_lowercase();
                    let secret = SECRET_KEYS.iter().any(|needle| lower.contains(needle));
                    (
                        key.clone(),
                        if secret {
                            Value::String("[redacted]".into())
                        } else {
                            redact(inner)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redact).collect()),
        other => other.clone(),
    }
}

/// JSON for a span attribute. A value that will not serialize is recorded as unserializable.
pub(crate) fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|e| format!("{{\"unserializable\":{:?}}}", e.to_string()))
}
