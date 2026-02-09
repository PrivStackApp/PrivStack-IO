//! Tests for p2p codec — targeting error handling and edge cases.

use futures::io::Cursor;
use privstack_sync::p2p::codec::{read_message, write_message};
use privstack_sync::protocol::SyncMessage;

/// Helper: write a raw length-prefixed payload into a buffer.
fn make_length_prefixed(payload: &[u8]) -> Vec<u8> {
    let len = payload.len() as u32;
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

#[tokio::test]
async fn test_roundtrip_ping() {
    let msg = SyncMessage::Ping(42);
    let mut buf = Cursor::new(Vec::new());
    write_message(&mut buf, &msg).await.unwrap();

    let written = buf.into_inner();
    let mut reader = Cursor::new(written);
    let decoded = read_message(&mut reader).await.unwrap();

    match decoded {
        SyncMessage::Ping(v) => assert_eq!(v, 42),
        other => panic!("expected Ping, got {:?}", other),
    }
}

#[tokio::test]
async fn test_roundtrip_pong() {
    let msg = SyncMessage::Pong(99);
    let mut buf = Cursor::new(Vec::new());
    write_message(&mut buf, &msg).await.unwrap();

    let written = buf.into_inner();
    let mut reader = Cursor::new(written);
    let decoded = read_message(&mut reader).await.unwrap();

    match decoded {
        SyncMessage::Pong(v) => assert_eq!(v, 99),
        other => panic!("expected Pong, got {:?}", other),
    }
}

#[tokio::test]
async fn test_read_message_too_large() {
    // Craft a length prefix that exceeds 16 MB
    let huge_len: u32 = 16 * 1024 * 1024 + 1;
    let mut data = Vec::new();
    data.extend_from_slice(&huge_len.to_be_bytes());

    let mut reader = Cursor::new(data);
    let result = read_message(&mut reader).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(err.to_string().contains("message too large"));
}

#[tokio::test]
async fn test_read_message_invalid_json() {
    let payload = b"this is not json";
    let data = make_length_prefixed(payload);

    let mut reader = Cursor::new(data);
    let result = read_message(&mut reader).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(err.to_string().contains("JSON decode error"));
}

#[tokio::test]
async fn test_read_message_truncated_length() {
    let data = vec![0u8, 1];
    let mut reader = Cursor::new(data);
    let result = read_message(&mut reader).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_read_message_truncated_body() {
    let mut data = Vec::new();
    data.extend_from_slice(&100u32.to_be_bytes());
    data.extend_from_slice(&[1, 2, 3, 4, 5]);

    let mut reader = Cursor::new(data);
    let result = read_message(&mut reader).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_read_message_zero_length_invalid_json() {
    let data = make_length_prefixed(b"");
    let mut reader = Cursor::new(data);
    let result = read_message(&mut reader).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(err.to_string().contains("JSON decode error"));
}

#[tokio::test]
async fn test_read_message_empty_stream() {
    let mut reader = Cursor::new(Vec::<u8>::new());
    let result = read_message(&mut reader).await;
    assert!(result.is_err());
}
