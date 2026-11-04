//! A source the upcoming tool reads: memory tasks, Canvas, or a calendar.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::core::types::agent::context::RequestContext;
use crate::core::types::tools::upcoming::UpcomingItem;

/// Lists the caller's dated items in a window.
#[async_trait]
pub trait UpcomingSource: Send + Sync {
    /// Name shown beside each item and in a failure line.
    fn label(&self) -> &'static str;
    /// The tool this source belongs to. A disabled tool takes its source with it.
    fn tool(&self) -> &'static str;
    /// Items from from until until, or one line saying why there are none.
    async fn upcoming(
        &self,
        ctx: &RequestContext,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<UpcomingItem>, String>;
}
