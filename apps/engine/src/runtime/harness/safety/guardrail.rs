//! The default guardrail: denied phrases, a length ceiling, and an empty answer check.

use async_trait::async_trait;

use crate::core::traits::safety::guardrail::Guardrail;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::safety::guardrail::{Stage, Verdict};

/// What the default guardrail refuses.
#[derive(Debug, Clone)]
pub struct Rules {
    /// Lowercase phrases that block a response wherever they appear.
    pub denied_phrases: Vec<String>,
    /// Lowercase terms removed from an answer wherever they appear as a whole word.
    pub protected_terms: Vec<String>,
    /// Text shown in an answer in place of a protected term.
    pub redaction: String,
    /// Longest answer allowed. Zero removes the limit.
    pub max_answer_chars: usize,
    /// Text shown in place of a blocked response.
    pub replacement: String,
}

impl Rules {
    /// Protects the tool names that hold an underscore; a one-word name is an ordinary word.
    pub fn protect_tools(&mut self, names: &[String]) {
        self.protect(names.iter().filter(|n| n.contains('_')).cloned());
    }

    /// Adds terms to protect from answers, lowercased, keeping each once and ignoring empties.
    pub fn protect(&mut self, terms: impl IntoIterator<Item = String>) {
        for term in terms {
            let term = term.trim().to_lowercase();
            if !term.is_empty() && !self.protected_terms.contains(&term) {
                self.protected_terms.push(term);
            }
        }
    }
}

impl Default for Rules {
    fn default() -> Self {
        Self::from(&crate::core::config::Guardrail::default())
    }
}

impl From<&crate::core::config::Guardrail> for Rules {
    fn from(cfg: &crate::core::config::Guardrail) -> Self {
        Self {
            denied_phrases: cfg
                .denied_phrases
                .iter()
                .map(|p| p.to_lowercase())
                .collect(),
            protected_terms: cfg
                .protected_terms
                .iter()
                .map(|p| p.to_lowercase())
                .collect(),
            redaction: cfg.redaction.clone(),
            max_answer_chars: cfg.max_answer_chars,
            replacement: cfg.replacement.clone(),
        }
    }
}

/// Checks a response against a fixed set of rules.
#[derive(Debug, Clone, Default)]
pub struct RuleGuardrail {
    rules: Rules,
}

impl RuleGuardrail {
    /// Builds the guardrail over its rules.
    pub fn new(rules: Rules) -> Self {
        Self { rules }
    }

    /// A block verdict with the configured replacement.
    fn block(&self, reason: impl Into<String>) -> Verdict {
        Verdict::Block {
            replacement: self.rules.replacement.clone(),
            reason: reason.into(),
        }
    }
}

#[async_trait]
impl Guardrail for RuleGuardrail {
    async fn check(&self, _ctx: &RequestContext, stage: Stage, text: &str) -> Verdict {
        let lowered = text.to_lowercase();
        if let Some(phrase) = self
            .rules
            .denied_phrases
            .iter()
            .find(|p| !p.is_empty() && lowered.contains(p.as_str()))
        {
            return self.block(format!("denied phrase {phrase}"));
        }
        // Emptiness, length, and redaction apply only to an answer shown to the user.
        if stage == Stage::Answer {
            if text.trim().is_empty() {
                return self.block("the answer was empty");
            }
            let chars = text.chars().count();
            if self.rules.max_answer_chars > 0 && chars > self.rules.max_answer_chars {
                return self.block(format!("the answer ran to {chars} characters"));
            }
            if let Some((cleaned, count)) =
                redact_terms(text, &self.rules.protected_terms, &self.rules.redaction)
            {
                return Verdict::Redact {
                    text: cleaned,
                    reason: format!("{count} protected {}", plural(count, "term", "terms")),
                };
            }
        }
        Verdict::Pass
    }
}

/// Replaces every whole-word occurrence of a protected term. None when nothing matched.
fn redact_terms(text: &str, terms: &[String], redaction: &str) -> Option<(String, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let mut terms: Vec<Vec<char>> = terms
        .iter()
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().collect())
        .collect();
    // Longest first, so the longest term matching at a position wins over a shorter prefix.
    terms.sort_by_key(|t| std::cmp::Reverse(t.len()));
    let mut out = String::with_capacity(text.len());
    let mut count = 0;
    let mut i = 0;
    while let Some(&here) = chars.get(i) {
        if let Some(len) = terms
            .iter()
            .find_map(|term| matches_at(&chars, i, term).then_some(term.len()))
        {
            out.push_str(redaction);
            count += 1;
            i += len;
        } else {
            out.push(here);
            i += 1;
        }
    }
    (count > 0).then_some((out, count))
}

/// Whether a protected term sits at index i as a whole word, matched without ASCII case.
fn matches_at(chars: &[char], i: usize, term: &[char]) -> bool {
    let after = i + term.len();
    if after > chars.len() {
        return false;
    }
    if i.checked_sub(1)
        .and_then(|p| chars.get(p))
        .is_some_and(|c| is_word(*c))
    {
        return false;
    }
    if chars.get(after).is_some_and(|c| is_word(*c)) {
        return false;
    }
    chars[i..after]
        .iter()
        .zip(term)
        .all(|(c, t)| c.eq_ignore_ascii_case(t))
}

/// Whether a character joins a word, so a term touching it is part of a longer identifier.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The singular form for a count of one, the plural otherwise.
fn plural(count: usize, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}
