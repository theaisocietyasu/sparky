//! calculate: arithmetic and date math, worked out exactly instead of by the model.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Datelike, Duration, FixedOffset, NaiveDate, Utc};
use serde_json::{Value, json};

use crate::core::traits::tools::Tool;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};

/// Name of the calculator tool.
pub const CALCULATE: &str = "calculate";

/// Longest expression accepted.
const MAX_EXPRESSION: usize = 500;

/// Deepest nesting of parentheses and unary signs accepted.
const MAX_DEPTH: usize = 64;

/// Largest day count a date may move by.
const MAX_DAYS: i64 = 36_600;

/// Works out arithmetic and date math.
pub struct CalculateTool {
    offset: FixedOffset,
}

impl CalculateTool {
    /// A calculator whose today is the date at offset.
    pub fn new(offset: FixedOffset) -> Self {
        Self { offset }
    }

    /// Today at the configured offset.
    fn today(&self) -> NaiveDate {
        Utc::now().with_timezone(&self.offset).date_naive()
    }

    /// A date argument: YYYY-MM-DD or today.
    fn date(&self, args: &Value, key: &str) -> Result<Option<NaiveDate>, ToolError> {
        let Some(raw) = args.get(key).and_then(Value::as_str).map(str::trim) else {
            return Ok(None);
        };
        if raw.is_empty() {
            return Ok(None);
        }
        if raw.eq_ignore_ascii_case("today") {
            return Ok(Some(self.today()));
        }
        NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .map(Some)
            .map_err(|_| ToolError::InvalidArguments(format!("{key} must be YYYY-MM-DD or today")))
    }

    /// The answer to a date question: the gap between two dates, or a date moved by days.
    fn dates(&self, args: &Value) -> Result<String, ToolError> {
        let from = self
            .date(args, "from")?
            .ok_or_else(|| ToolError::InvalidArguments("from is required for date math".into()))?;
        if let Some(to) = self.date(args, "to")? {
            let days = (to - from).num_days();
            return Ok(format!(
                "From {} to {} is {days} days ({}).",
                describe(from),
                describe(to),
                weeks(days)
            ));
        }
        if let Some(n) = args.get("add_days").and_then(Value::as_i64) {
            if n.abs() > MAX_DAYS {
                return Err(ToolError::InvalidArguments(format!(
                    "add_days must be within {MAX_DAYS}"
                )));
            }
            let moved = from
                .checked_add_signed(Duration::days(n))
                .ok_or_else(|| ToolError::InvalidArguments("that date is out of range".into()))?;
            return Ok(format!(
                "{n} days from {} is {}.",
                describe(from),
                describe(moved)
            ));
        }
        Ok(format!("{} is a {}.", from, from.weekday_name()))
    }
}

/// A date with its weekday.
fn describe(date: NaiveDate) -> String {
    format!("{date} ({})", date.weekday_name())
}

/// A day count as weeks and days.
fn weeks(days: i64) -> String {
    let (w, d) = (days.abs() / 7, days.abs() % 7);
    format!("{w} weeks and {d} days")
}

/// The full English name of a weekday.
trait WeekdayName {
    /// Monday, Tuesday and so on.
    fn weekday_name(&self) -> &'static str;
}

impl WeekdayName for NaiveDate {
    fn weekday_name(&self) -> &'static str {
        match self.weekday() {
            chrono::Weekday::Mon => "Monday",
            chrono::Weekday::Tue => "Tuesday",
            chrono::Weekday::Wed => "Wednesday",
            chrono::Weekday::Thu => "Thursday",
            chrono::Weekday::Fri => "Friday",
            chrono::Weekday::Sat => "Saturday",
            chrono::Weekday::Sun => "Sunday",
        }
    }
}

#[async_trait]
impl Tool for CalculateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: CALCULATE.to_owned(),
            description: "Work out math or dates exactly. Give expression for arithmetic, such as \
                          (3*4 + 2*3) / 7 or round(780 * 1.08, 2); it knows + - * / % ^, \
                          parentheses, sqrt, abs, round, floor, ceil, min, max, ln, log, pi and e. \
                          For dates give from, and to for the days between them, or add_days to \
                          move a date; from alone gives its weekday. Dates are YYYY-MM-DD or today."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "An arithmetic expression."},
                    "from": {"type": "string", "description": "A date, YYYY-MM-DD or today."},
                    "to": {"type": "string", "description": "A second date, YYYY-MM-DD or today."},
                    "add_days": {"type": "integer", "description": "Days to move from by, negative for earlier."}
                }
            }),
            risk: RiskClass::ReadPublic,
            private: false,
            sequential: false,
            timeout_secs: None,
        }
    }

    async fn call(&self, _ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let expression = args
            .get("expression")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|e| !e.is_empty());
        let content = match expression {
            Some(expr) => {
                let value = evaluate(expr).map_err(ToolError::InvalidArguments)?;
                format!("{expr} = {}", format_number(value))
            }
            None => self.dates(&args)?,
        };
        Ok(ToolOutput {
            content,
            data: None,
            sources: Vec::new(),
        })
    }
}

/// The value of an arithmetic expression, or why it has none.
pub fn evaluate(expression: &str) -> Result<f64, String> {
    if expression.len() > MAX_EXPRESSION {
        return Err(format!(
            "the expression is longer than {MAX_EXPRESSION} characters"
        ));
    }
    let tokens = tokenize(expression)?;
    let mut parser = Parser {
        tokens,
        at: 0,
        depth: 0,
    };
    let value = parser.sum()?;
    if parser.at != parser.tokens.len() {
        return Err("the expression has something left over after it ends".into());
    }
    if value.is_finite() {
        Ok(value)
    } else {
        Err("the result is not a finite number".into())
    }
}

/// A value as a person writes it: whole numbers without a point, others to ten places.
pub fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{value:.0}");
    }
    let text = format!("{value:.10}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// One piece of an expression.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Op(char),
    Open,
    Close,
    Comma,
}

/// The tokens of an expression.
fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_')
            {
                i += 1;
            }
            let raw: String = chars[start..i].iter().filter(|c| **c != '_').collect();
            let n = raw
                .parse::<f64>()
                .map_err(|_| format!("{raw} is not a number"))?;
            out.push(Token::Number(n));
        } else if c.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphanumeric() {
                i += 1;
            }
            out.push(Token::Name(
                chars[start..i]
                    .iter()
                    .collect::<String>()
                    .to_ascii_lowercase(),
            ));
        } else {
            out.push(match c {
                '+' | '-' | '*' | '/' | '%' | '^' => Token::Op(c),
                '\u{d7}' => Token::Op('*'),
                '\u{f7}' => Token::Op('/'),
                '(' => Token::Open,
                ')' => Token::Close,
                ',' => Token::Comma,
                other => return Err(format!("{other} is not part of arithmetic")),
            });
            i += 1;
        }
    }
    Ok(out)
}

/// A recursive descent parser over the tokens of one expression.
struct Parser {
    tokens: Vec<Token>,
    at: usize,
    depth: usize,
}

impl Parser {
    /// The next token, without taking it.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    /// Takes the next token when it is the operator op.
    fn take_op(&mut self, ops: &[char]) -> Option<char> {
        match self.peek() {
            Some(Token::Op(c)) if ops.contains(c) => {
                let c = *c;
                self.at += 1;
                Some(c)
            }
            _ => None,
        }
    }

    /// Enters one level of nesting, refused past the limit.
    fn enter(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err("the expression nests too deeply".into());
        }
        Ok(())
    }

    /// Terms joined by + and -.
    fn sum(&mut self) -> Result<f64, String> {
        let mut value = self.product()?;
        while let Some(op) = self.take_op(&['+', '-']) {
            let rhs = self.product()?;
            value = if op == '+' { value + rhs } else { value - rhs };
        }
        Ok(value)
    }

    /// Factors joined by *, / and %.
    fn product(&mut self) -> Result<f64, String> {
        let mut value = self.unary()?;
        while let Some(op) = self.take_op(&['*', '/', '%']) {
            let rhs = self.unary()?;
            if rhs == 0.0 && op != '*' {
                return Err("division by zero".into());
            }
            value = match op {
                '*' => value * rhs,
                '/' => value / rhs,
                _ => value % rhs,
            };
        }
        Ok(value)
    }

    /// A signed power.
    fn unary(&mut self) -> Result<f64, String> {
        if let Some(op) = self.take_op(&['+', '-']) {
            self.enter()?;
            let value = self.unary()?;
            self.depth -= 1;
            return Ok(if op == '-' { -value } else { value });
        }
        self.power()
    }

    /// An atom raised to a power, right associative.
    fn power(&mut self) -> Result<f64, String> {
        let base = self.atom()?;
        if self.take_op(&['^']).is_some() {
            self.enter()?;
            let exponent = self.unary()?;
            self.depth -= 1;
            return Ok(base.powf(exponent));
        }
        Ok(base)
    }

    /// A number, a constant, a call, or a parenthesised expression.
    fn atom(&mut self) -> Result<f64, String> {
        let token = self
            .peek()
            .cloned()
            .ok_or("the expression ends too early")?;
        self.at += 1;
        match token {
            Token::Number(n) => Ok(n),
            Token::Open => {
                self.enter()?;
                let value = self.sum()?;
                self.depth -= 1;
                self.expect_close()?;
                Ok(value)
            }
            Token::Name(name) => self.named(&name),
            other => Err(format!("unexpected {}", describe_token(&other))),
        }
    }

    /// Requires a closing parenthesis.
    fn expect_close(&mut self) -> Result<(), String> {
        if self.peek() == Some(&Token::Close) {
            self.at += 1;
            Ok(())
        } else {
            Err("a parenthesis is not closed".into())
        }
    }

    /// A constant, or a function applied to its arguments.
    fn named(&mut self, name: &str) -> Result<f64, String> {
        match name {
            "pi" => return Ok(std::f64::consts::PI),
            "e" => return Ok(std::f64::consts::E),
            _ => {}
        }
        if self.peek() != Some(&Token::Open) {
            return Err(format!("{name} is not a known constant"));
        }
        self.at += 1;
        self.enter()?;
        let mut args = vec![self.sum()?];
        while self.peek() == Some(&Token::Comma) {
            self.at += 1;
            args.push(self.sum()?);
        }
        self.depth -= 1;
        self.expect_close()?;
        apply(name, &args)
    }
}

/// A function applied to its arguments.
fn apply(name: &str, args: &[f64]) -> Result<f64, String> {
    let one = |f: fn(f64) -> f64| match args {
        [x] => Ok(f(*x)),
        _ => Err(format!("{name} takes one number")),
    };
    match name {
        "sqrt" => one(f64::sqrt),
        "abs" => one(f64::abs),
        "floor" => one(f64::floor),
        "ceil" => one(f64::ceil),
        "ln" => one(f64::ln),
        "log" => one(f64::log10),
        "round" => match args {
            [x] => Ok(x.round()),
            [x, places] => {
                let scale = 10f64.powf(places.clamp(0.0, 12.0).round());
                Ok((x * scale).round() / scale)
            }
            _ => Err("round takes a number and, optionally, decimal places".into()),
        },
        "min" => args
            .iter()
            .copied()
            .reduce(f64::min)
            .ok_or_else(|| "min needs a number".into()),
        "max" => args
            .iter()
            .copied()
            .reduce(f64::max)
            .ok_or_else(|| "max needs a number".into()),
        other => Err(format!("{other} is not a known function")),
    }
}

/// A token as words, for an error.
fn describe_token(token: &Token) -> String {
    match token {
        Token::Number(n) => format_number(*n),
        Token::Name(n) => n.clone(),
        Token::Op(c) => c.to_string(),
        Token::Open => "(".into(),
        Token::Close => ")".into(),
        Token::Comma => ",".into(),
    }
}

/// The calculator tool, its today at utc_offset_hours.
pub fn tools(utc_offset_hours: i32) -> Result<Vec<Arc<dyn Tool>>, String> {
    let offset = FixedOffset::east_opt(utc_offset_hours * 3600)
        .ok_or("prompt.utc_offset_hours is out of range")?;
    let tool: Arc<dyn Tool> = Arc::new(CalculateTool::new(offset));
    Ok(vec![tool])
}
