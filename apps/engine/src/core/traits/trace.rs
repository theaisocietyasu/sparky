//! TraceSink and TraceReader traits.

use crate::core::types::admin::TraceSummary;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::trace::{TraceEvent, TraceRecord};

/// Where trace events go. Must not block the loop.
pub trait TraceSink: Send + Sync {
    /// Records one event.
    fn emit(&self, ctx: &RequestContext, event: TraceEvent);
}

/// Reads recorded traces back. Blocking; callers run it off the async runtime.
pub trait TraceReader: Send + Sync {
    /// The newest traces of tenant, newest first, at most limit.
    fn list(&self, tenant: &str, limit: usize) -> std::io::Result<Vec<TraceSummary>>;

    /// Every recorded event of one request in tenant, or None when there is no such trace.
    fn read(
        &self,
        tenant: &str,
        request_id: uuid::Uuid,
    ) -> std::io::Result<Option<Vec<TraceRecord>>>;
}
