//! Codec for sync protocol messages over libp2p request-response.

use crate::protocol::SyncMessage;
use async_trait::async_trait;
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::request_response;
use std::io;

/// Maximum message size (16 MB).
const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// The sync protocol codec for request-response.
#[derive(Debug, Clone, Default)]
pub struct SyncCodec;

/// Sync protocol request (wraps SyncMessage).
#[derive(Debug, Clone)]
pub struct SyncRequest(pub SyncMessage);

/// Sync protocol response (wraps SyncMessage).
#[derive(Debug, Clone)]
pub struct SyncResponse(pub SyncMessage);

#[async_trait]
impl request_response::Codec for SyncCodec {
    type Protocol = &'static str;
    type Request = SyncRequest;
    type Response = SyncResponse;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let message = read_message(io).await?;
        Ok(SyncRequest(message))
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let message = read_message(io).await?;
        Ok(SyncResponse(message))
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_message(io, &req.0).await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_message(io, &res.0).await
    }
}

/// Reads a length-prefixed JSON message.
pub async fn read_message<T: AsyncRead + Unpin>(io: &mut T) -> io::Result<SyncMessage> {
    // Read 4-byte length prefix
    let mut len_bytes = [0u8; 4];
    io.read_exact(&mut len_bytes).await?;
    let len = u32::from_be_bytes(len_bytes) as usize;

    // Validate size
    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message too large: {len} bytes"),
        ));
    }

    // Read message body
    let mut buf = vec![0u8; len];
    io.read_exact(&mut buf).await?;

    // Deserialize
    serde_json::from_slice(&buf).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("JSON decode error: {e}"),
        )
    })
}

/// Writes a length-prefixed JSON message.
pub async fn write_message<T: AsyncWrite + Unpin>(io: &mut T, message: &SyncMessage) -> io::Result<()> {
    // Serialize to JSON
    let data = serde_json::to_vec(message).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("JSON encode error: {e}"),
        )
    })?;

    // Validate size
    if data.len() > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message too large: {} bytes", data.len()),
        ));
    }

    // Write length prefix
    let len_bytes = (data.len() as u32).to_be_bytes();
    io.write_all(&len_bytes).await?;

    // Write message body
    io.write_all(&data).await?;
    io.flush().await?;

    Ok(())
}
