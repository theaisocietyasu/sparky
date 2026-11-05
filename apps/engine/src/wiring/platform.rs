//! Platform composition: every store over the platform HTTP API. The store composition may move
//! conversations, memories and pending actions to the SQLite file.

use std::sync::Arc;
use std::time::Duration;

use super::Stores;
use crate::core::config::Config;
use crate::core::traits::knowledge::discovery::Discovery;
use crate::stores::platform::{
    PlatformAccounts, PlatformClient, PlatformConfirmations, PlatformConversations,
    PlatformDiscovery, PlatformMemory, PlatformProbe, PlatformQueries,
};

/// Every store over the platform. Config::validate guarantees the url and the token.
pub(super) fn stores(cfg: &Config) -> anyhow::Result<Stores> {
    let (Some(url), Some(token)) = (cfg.platform.url.as_deref(), cfg.platform.token.clone()) else {
        anyhow::bail!("set SPARKY_PLATFORM__URL and SPARKY_PLATFORM__TOKEN");
    };
    let client = PlatformClient::new(url, token, Duration::from_secs(cfg.platform.timeout_secs))
        .map_err(|e| anyhow::anyhow!("platform client: {e}"))?;
    tracing::info!(url, "stores are the platform's");
    Ok(Stores {
        queries: Arc::new(PlatformQueries::new(client.clone())),
        conversations: Arc::new(PlatformConversations::new(client.clone())),
        memory: Arc::new(PlatformMemory::new(client.clone())),
        confirmations: Arc::new(PlatformConfirmations::new(client.clone())),
        oauth_store: Arc::new(PlatformAccounts::new(client.clone())),
        canvas: None,
        reminders: None,
        nudges: None,
        admin: None,
        discovery: cfg
            .read_page
            .save_pages
            .then(|| Arc::new(PlatformDiscovery::new(client.clone())) as Arc<dyn Discovery>),
        probes: vec![Arc::new(PlatformProbe::new(client))],
    })
}
