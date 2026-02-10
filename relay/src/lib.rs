//! Shared types and HTTP API for the PrivStack relay.

use std::sync::Arc;
use axum::{extract::State, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IdentityResponse {
    pub peer_id: String,
    pub addresses: Vec<String>,
    pub protocol_version: String,
    pub agent_version: String,
}

async fn identity_handler(
    State(identity): State<Arc<IdentityResponse>>,
) -> Json<IdentityResponse> {
    Json((*identity).clone())
}

/// Build the HTTP API router with the given identity state.
pub fn build_router(identity: Arc<IdentityResponse>) -> Router {
    Router::new()
        .route("/api/v1/identity", get(identity_handler))
        .with_state(identity)
}
