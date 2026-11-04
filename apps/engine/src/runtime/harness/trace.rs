//! JSONL trace sink: one file per request, one event per line.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use chrono::Utc;
use uuid::Uuid;

use crate::core::traits::trace::{TraceReader, TraceSink};
use crate::core::types::admin::TraceSummary;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::trace::progress::{Progress, ProgressStyle};
use crate::core::types::trace::{TraceEvent, TraceRecord};

/// The trace record of event under the ids of ctx.
fn record(ctx: &RequestContext, event: TraceEvent) -> TraceRecord {
    TraceRecord {
        request_id: ctx.request_id,
        conversation_id: ctx.conversation_id,
        at: Utc::now(),
        event,
    }
}

/// Appends one JSON line per event to a file named for the request id, under the trace dir.
#[derive(Debug)]
pub struct JsonlSink {
    dir: PathBuf,
    /// Stop writing the trace of a request past this many bytes. Zero removes the limit.
    max_file_bytes: u64,
}

impl JsonlSink {
    /// Writes under dir, creating it if missing.
    pub fn new(dir: impl Into<PathBuf>, max_file_bytes: u64) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            max_file_bytes,
        })
    }

    /// Path of the trace file for a request.
    pub fn path_for(&self, request_id: Uuid) -> PathBuf {
        self.dir.join(format!("{request_id}.jsonl"))
    }

    /// Deletes trace files last modified more than older_than ago. Called at boot and hourly.
    pub fn prune(&self, older_than: Duration) -> std::io::Result<usize> {
        let cutoff = SystemTime::now()
            .checked_sub(older_than)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let mut removed = 0;
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            if entry.path().extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            let stale = entry
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|modified| modified < cutoff);
            if !stale {
                continue;
            }
            match std::fs::remove_file(entry.path()) {
                Ok(()) => removed += 1,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        path = %entry.path().display(),
                        "stale trace not removed"
                    );
                }
            }
        }
        Ok(removed)
    }
}

impl TraceSink for JsonlSink {
    fn emit(&self, ctx: &RequestContext, event: TraceEvent) {
        use std::io::Write;
        if event.is_transient() {
            return;
        }
        let line = match serde_json::to_string(&record(ctx, event)) {
            Ok(line) => line,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    request_id = %ctx.request_id,
                    "trace event would not serialize"
                );
                return;
            }
        };
        let path = self.path_for(ctx.request_id);
        let result = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| {
                // Past the cap, later events are dropped.
                if self.max_file_bytes > 0 && f.metadata()?.len() >= self.max_file_bytes {
                    return Ok(());
                }
                writeln!(f, "{line}")
            });
        if let Err(e) = result {
            tracing::warn!(error = %e, path = %path.display(), "trace write failed");
        }
    }
}

/// Reads the JSONL traces under the trace dir.
#[derive(Debug, Clone)]
pub struct JsonlTraces {
    dir: PathBuf,
}

/// Characters of the user input a summary keeps.
const INPUT_CHARS: usize = 160;

impl JsonlTraces {
    /// Reads under dir.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The records of one file; a line that does not parse is skipped.
    fn records(path: &std::path::Path) -> std::io::Result<Vec<TraceRecord>> {
        let text = std::fs::read_to_string(path)?;
        Ok(text
            .lines()
            .filter_map(|line| serde_json::from_str::<TraceRecord>(line).ok())
            .collect())
    }

    /// The summary of one trace, or None when it is not of tenant or has no start.
    fn summary(records: &[TraceRecord], tenant: &str) -> Option<TraceSummary> {
        let first = records.first()?;
        let TraceEvent::RequestStarted {
            input,
            tenant_id,
            user_id,
        } = &first.event
        else {
            return None;
        };
        if tenant_id != tenant {
            return None;
        }
        let mut summary = TraceSummary {
            request_id: first.request_id.to_string(),
            conversation_id: first.conversation_id.to_string(),
            at: first.at.to_rfc3339(),
            tenant_id: tenant_id.clone(),
            user_id: user_id.clone(),
            input: input.chars().take(INPUT_CHARS).collect(),
            status: None,
            steps: 0,
            tokens: 0,
            duration_ms: 0,
        };
        if let Some(TraceEvent::Completed {
            status,
            steps,
            usage,
            duration_ms,
            ..
        }) = records.last().map(|r| &r.event)
        {
            summary.status = serde_json::to_value(status)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned));
            summary.steps = *steps;
            summary.tokens = usage.prompt_tokens.saturating_add(usage.completion_tokens);
            summary.duration_ms = *duration_ms;
        }
        Some(summary)
    }
}

impl TraceReader for JsonlTraces {
    fn list(&self, tenant: &str, limit: usize) -> std::io::Result<Vec<TraceSummary>> {
        let mut files: Vec<(SystemTime, PathBuf)> = Vec::new();
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            files.push((modified, path));
        }
        files.sort_by_key(|f| std::cmp::Reverse(f.0));
        let mut out = Vec::new();
        for (_, path) in files {
            if out.len() >= limit {
                break;
            }
            match Self::records(&path) {
                Ok(records) => out.extend(Self::summary(&records, tenant)),
                Err(e) => tracing::warn!(error = %e, path = %path.display(), "trace not read"),
            }
        }
        Ok(out)
    }

    fn read(&self, tenant: &str, request_id: Uuid) -> std::io::Result<Option<Vec<TraceRecord>>> {
        let path = self.dir.join(format!("{request_id}.jsonl"));
        let records = match Self::records(&path) {
            Ok(records) => records,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        if Self::summary(&records, tenant).is_none() {
            return Ok(None);
        }
        Ok(Some(records))
    }
}

/// Discards every event. Used when trace recording is switched off.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl TraceSink for NullSink {
    fn emit(&self, _ctx: &RequestContext, _event: TraceEvent) {}
}

/// Records every event through the sink beneath it and forwards progress to a watching caller.
pub struct Fanout {
    inner: Arc<dyn TraceSink>,
    style: ProgressStyle,
}

impl Fanout {
    /// Wraps the sink that records the full trace, with the default amount of detail.
    pub fn new(inner: Arc<dyn TraceSink>) -> Self {
        Self {
            inner,
            style: ProgressStyle::default(),
        }
    }

    /// Sets how much detail a forwarded progress line carries.
    #[must_use]
    pub fn with_style(mut self, style: ProgressStyle) -> Self {
        self.style = style;
        self
    }
}

impl TraceSink for Fanout {
    fn emit(&self, ctx: &RequestContext, event: TraceEvent) {
        if let Some(tx) = &ctx.progress
            && let Some(progress) = Progress::of(&event, self.style)
        {
            // A closed receiver means the caller stopped watching.
            let _ = tx.send(progress);
        }
        self.inner.emit(ctx, event);
    }
}
