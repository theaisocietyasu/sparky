//! Trace doubles: an in-memory sink.

use std::sync::Mutex;

use crate::core::traits::trace::TraceSink;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::trace::{TraceEvent, TraceRecord};

/// Keeps every trace record for tests to inspect.
#[derive(Default)]
pub struct MemorySink {
    records: Mutex<Vec<TraceRecord>>,
}

impl MemorySink {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
        }
    }

    pub fn records(&self) -> Vec<TraceRecord> {
        self.records.lock().map_or_default(|r| r.clone())
    }
}

impl TraceSink for MemorySink {
    fn emit(&self, ctx: &RequestContext, event: TraceEvent) {
        if let Ok(mut records) = self.records.lock() {
            records.push(TraceRecord {
                request_id: ctx.request_id,
                conversation_id: ctx.conversation_id,
                at: chrono::Utc::now(),
                event,
            });
        }
    }
}
