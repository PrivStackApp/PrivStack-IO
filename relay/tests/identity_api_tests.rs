use std::sync::Arc;
use privstack_relay::{build_router, IdentityResponse};

fn test_identity() -> Arc<IdentityResponse> {
    Arc::new(IdentityResponse {
        peer_id: "12D3KooWTestPeerId".to_string(),
        addresses: vec![
            "/ip4/0.0.0.0/udp/4001/quic-v1".to_string(),
        ],
        protocol_version: "/privstack/relay/1.0.0".to_string(),
        agent_version: "privstack-relay/0.1.0".to_string(),
    })
}

/// Spin up the HTTP server on an OS-assigned port, returning the base URL.
async fn spawn_test_server() -> String {
    let app = build_router(test_identity());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://127.0.0.1:{}", port)
}

#[tokio::test]
async fn identity_endpoint_returns_correct_json() {
    let base = spawn_test_server().await;
    let resp = reqwest::get(format!("{}/api/v1/identity", base))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: IdentityResponse = resp.json().await.unwrap();
    assert_eq!(body.peer_id, "12D3KooWTestPeerId");
    assert_eq!(body.addresses, vec!["/ip4/0.0.0.0/udp/4001/quic-v1"]);
    assert_eq!(body.protocol_version, "/privstack/relay/1.0.0");
    assert_eq!(body.agent_version, "privstack-relay/0.1.0");
}

#[tokio::test]
async fn identity_endpoint_content_type_is_json() {
    let base = spawn_test_server().await;
    let resp = reqwest::get(format!("{}/api/v1/identity", base))
        .await
        .unwrap();

    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("application/json"));
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let base = spawn_test_server().await;
    let resp = reqwest::get(format!("{}/api/v1/nonexistent", base))
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn identity_endpoint_multiple_addresses() {
    let identity = Arc::new(IdentityResponse {
        peer_id: "12D3KooWMulti".to_string(),
        addresses: vec![
            "/ip4/0.0.0.0/udp/4001/quic-v1".to_string(),
            "/ip6/::/udp/4001/quic-v1".to_string(),
        ],
        protocol_version: "/privstack/relay/1.0.0".to_string(),
        agent_version: "privstack-relay/0.1.0".to_string(),
    });

    let app = build_router(identity);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let resp = reqwest::get(format!("http://127.0.0.1:{}/api/v1/identity", port))
        .await
        .unwrap();

    let body: IdentityResponse = resp.json().await.unwrap();
    assert_eq!(body.addresses.len(), 2);
    assert_eq!(body.peer_id, "12D3KooWMulti");
}
