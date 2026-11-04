//! engine import-platform: copies the platform agents export into the SQLite file once.

use std::time::Duration;

use chrono::Utc;

use super::store::{open, seal};
use crate::core::config::Config;
use crate::stores::platform::{PlatformClient, PlatformExport};
use crate::stores::sqlite::{ImportCounts, Importer};

/// The imports row of this source.
const SOURCE: &str = "platform";

/// Pages through the export and writes it; refuses a second run unless again is true.
pub async fn import_platform(cfg: &Config, again: bool) -> anyhow::Result<()> {
    let (Some(url), Some(token)) = (cfg.platform.url.as_deref(), cfg.platform.token.clone()) else {
        anyhow::bail!("set SPARKY_PLATFORM__URL and SPARKY_PLATFORM__TOKEN");
    };
    let client = PlatformClient::new(url, token, Duration::from_secs(cfg.platform.timeout_secs))
        .map_err(|e| anyhow::anyhow!("platform client: {e}"))?;
    let export = PlatformExport::new(client);
    let store = open(cfg).await?;
    let cutoff = Utc::now() - chrono::Duration::days(i64::from(cfg.store.import_days));
    let tenant = cfg.discord.guild_id.to_string();
    let mut importer = Importer::new(store.pool().clone(), seal(cfg)?, &tenant, cutoff);
    importer.begin(SOURCE, again).await?;
    let size = cfg.store.import_page_size;

    let mut after: Option<String> = None;
    loop {
        let page = export
            .conversations(after.as_deref(), size)
            .await
            .map_err(|e| anyhow::anyhow!("export conversations: {e}"))?;
        importer.conversations(&page).await?;
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    let mut after: Option<String> = None;
    loop {
        let page = export
            .memories(after.as_deref(), size)
            .await
            .map_err(|e| anyhow::anyhow!("export memories: {e}"))?;
        importer.memories(&page).await?;
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    let counts: ImportCounts = importer.finish(SOURCE).await?;
    tracing::info!(
        conversations = counts.conversations,
        messages = counts.messages,
        memories = counts.memories,
        skipped = counts.skipped,
        "platform import finished"
    );
    Ok(())
}
