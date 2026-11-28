//! Platform composition: search, the page sink and readiness over the platform HTTP API.

use std::sync::Arc;
use std::time::Duration;

use crate::core::config::Config;
use crate::core::traits::health::Probe;
use crate::core::traits::knowledge::discovery::Discovery;
use crate::core::traits::knowledge::query::SourceQueries;
use crate::stores::platform::{PlatformClient, PlatformDiscovery, PlatformProbe, PlatformQueries};

/// What the engine reads from the platform.
pub(super) struct Platform {
    /// Platform search.
    pub queries: Arc<dyn SourceQueries>,
    /// Where pages read_page reads are suggested, when read_page.save_pages is on.
    pub discovery: Option<Arc<dyn Discovery>>,
    /// Readiness of the platform.
    pub probe: Arc<dyn Probe>,
}

/// The platform adapters. Config::validate guarantees the url and the token.
pub(super) fn platform(cfg: &Config) -> anyhow::Result<Platform> {
    let (Some(url), Some(token)) = (cfg.platform.url.as_deref(), cfg.platform.token.clone()) else {
        anyhow::bail!("set SPARKY_PLATFORM__URL and SPARKY_PLATFORM__TOKEN");
    };
    let client = PlatformClient::new(url, token, Duration::from_secs(cfg.platform.timeout_secs))
        .map_err(|e| anyhow::anyhow!("platform client: {e}"))?;
    tracing::info!(url, "search is the platform's");
    Ok(Platform {
        queries: Arc::new(PlatformQueries::new(client.clone())),
        discovery: cfg
            .read_page
            .save_pages
            .then(|| Arc::new(PlatformDiscovery::new(client.clone())) as Arc<dyn Discovery>),
        probe: Arc::new(PlatformProbe::new(client)),
    })
}
