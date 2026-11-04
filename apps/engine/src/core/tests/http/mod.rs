//! The HTTP surface: health, login, Canvas feed links, rate limiting, and the sandbox routes.

mod admin;
mod canvas;
mod health;
mod login;
mod memories;
mod rate_limit;
mod sandbox;
pub(crate) mod web;

#[test]
fn a_secret_matches_only_when_every_byte_does() {
    use crate::routes::auth::same_secret;

    assert!(same_secret(b"change-me", b"change-me"));
    assert!(!same_secret(b"change-mf", b"change-me"));
    assert!(!same_secret(b"change", b"change-me"));
    assert!(!same_secret(b"", b"change-me"));
}

#[test]
fn an_agent_error_maps_to_the_status_every_route_returns() {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use uuid::Uuid;

    use crate::core::types::agent::AgentError;
    use crate::core::types::model::ModelError;
    use crate::routes::failure::Failure;

    let status = |error: AgentError| {
        Failure::from_agent(Uuid::nil(), error)
            .into_response()
            .status()
    };
    assert_eq!(
        status(AgentError::Model(ModelError::Busy)),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        status(AgentError::Model(ModelError::Timeout)),
        StatusCode::BAD_GATEWAY
    );
    assert_eq!(
        status(AgentError::Store("down".into())),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
