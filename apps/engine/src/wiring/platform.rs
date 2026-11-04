//! Platform composition: every store over the platform HTTP API. The store composition may move
//! conversations, memories and pending actions to the SQLite file.

use std::sync::Arc;
use std::time::Duration;

use super::Stores;
use crate::core::config::Config;
use crate::core::traits::knowledge::retrieval::Embedder;
use crate::stores::platform::{
    PlatformAccounts, PlatformClient, PlatformConfirmations, PlatformConversations, PlatformMemory,
    PlatformProbe, PlatformQueries, PlatformRetriever, SearchTuning,
};

/// Every store over the platform. Config::validate guarantees the url and the token.
pub(super) fn stores(cfg: &Config, embedder: &Arc<dyn Embedder>) -> anyhow::Result<Stores> {
    let (Some(url), Some(token)) = (cfg.platform.url.as_deref(), cfg.platform.token.clone()) else {
        anyhow::bail!("set SPARKY_PLATFORM__URL and SPARKY_PLATFORM__TOKEN");
    };
    let client = PlatformClient::new(url, token, Duration::from_secs(cfg.platform.timeout_secs))
        .map_err(|e| anyhow::anyhow!("platform client: {e}"))?;
    let embedding_model = match cfg.platform.embedding_model.trim() {
        "" => cfg.embedding.name.clone(),
        named => named.to_owned(),
    };
    let tuning = SearchTuning {
        embedding_model,
        window: cfg.retrieval.window,
        max_query_chars: cfg.platform.max_query_chars,
    };
    // The query vector is the dense leg; without it the platform searches as it is configured to.
    let query_embedder = cfg.retrieval.dense.then(|| Arc::clone(embedder));
    tracing::info!(url, "stores are the platform's");
    Ok(Stores {
        retriever: Arc::new(PlatformRetriever::new(
            client.clone(),
            query_embedder,
            tuning,
        )),
        queries: Arc::new(PlatformQueries::new(client.clone())),
        conversations: Arc::new(PlatformConversations::new(client.clone())),
        memory: Arc::new(PlatformMemory::new(client.clone())),
        confirmations: Arc::new(PlatformConfirmations::new(client.clone())),
        oauth_store: Arc::new(PlatformAccounts::new(client.clone())),
        canvas: None,
        probes: vec![Arc::new(PlatformProbe::new(client))],
    })
}
