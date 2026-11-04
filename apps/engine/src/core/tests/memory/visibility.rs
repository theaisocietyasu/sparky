//! A public request loads no personal memory; a private one does and says which memories it used.

use std::sync::Arc;

use crate::core::tests::support::{Recalling, Scripted, agent_recalling, ctx, text};
use crate::core::types::agent::AgentConfig;
use crate::core::types::conversation::Visibility;

/// What one turn recalled: memory store calls and the memories the answer reports.
async fn recalls(visibility: Visibility) -> (usize, Vec<String>) {
    let memory = Arc::new(Recalling::default());
    let cfg = AgentConfig::default();
    let agent = agent_recalling(Scripted::new(vec![Ok(text("hi"))]), cfg, memory.clone());
    let answered = agent
        .run(&ctx().with_visibility(visibility), "what do I study")
        .await;
    let memories = answered.map(|a| a.memories).unwrap_or_default();
    (memory.calls(), memories)
}

#[tokio::test]
async fn a_public_request_recalls_no_memory() {
    let (memory, memories) = recalls(Visibility::Public).await;
    assert_eq!(memory, 0);
    assert_eq!(memories.len(), 0);
}

#[tokio::test]
async fn a_private_request_recalls_memory_and_reports_it() {
    let (memory, memories) = recalls(Visibility::Private).await;
    assert_eq!(memory, 1);
    assert_eq!(memories, vec!["studies CSE 310".to_owned()]);
}

#[test]
fn a_context_starts_public() {
    assert_eq!(ctx().visibility, Visibility::Public);
}

#[tokio::test]
async fn a_private_turn_makes_only_the_model_calls_of_the_loop() {
    let memory = Arc::new(Recalling::default());
    let model = Scripted::new(vec![Ok(text("hi"))]);
    let sent = model.sent();
    let agent = agent_recalling(model, AgentConfig::default(), memory);
    let ctx = ctx().with_visibility(Visibility::Private);
    assert!(agent.run(&ctx, "I am a CS junior at Poly").await.is_ok());
    tokio::task::yield_now().await;
    assert_eq!(sent.lock().map(|s| s.len()).unwrap_or_default(), 1);
}
