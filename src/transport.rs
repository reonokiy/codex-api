//! Tap the wire before Codex's unchanged SSE parser consumes it.
use bytes::Bytes;
use codex_client::{HttpTransport, Request, Response, StreamResponse, TransportError};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde_json::Value;
use tokio::sync::mpsc;

const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
pub struct WireEvent {
    pub bytes: Bytes,
    pub value: Value,
}
impl WireEvent {
    pub fn terminal(&self) -> bool {
        matches!(
            self.value["type"].as_str(),
            Some("response.completed" | "response.failed" | "response.incomplete")
        )
    }
    pub fn successful(&self) -> bool {
        self.value["type"] == "response.completed"
    }
}

pub struct TapTransport<T> {
    pub response_headers: std::sync::Arc<std::sync::OnceLock<http::HeaderMap>>,
    pub inner: T,
    pub sender: mpsc::Sender<WireEvent>,
}
impl<T: HttpTransport> HttpTransport for TapTransport<T> {
    async fn execute(&self, request: Request) -> Result<Response, TransportError> {
        self.inner.execute(request).await
    }
    async fn stream(&self, request: Request) -> Result<StreamResponse, TransportError> {
        let upstream = self.inner.stream(request).await?;
        let _ = self.response_headers.set(upstream.headers.clone());
        let sender = self.sender.clone();
        let mut source = upstream.bytes;
        let bounded: codex_client::ByteStream = Box::pin(async_stream::try_stream! {
            let mut total = 0usize;
            while let Some(chunk) = source.next().await {
                let chunk = chunk?;
                total = total.saturating_add(chunk.len());
                if total > MAX_RESPONSE_BYTES {
                    Err(TransportError::Network("upstream response exceeded 64 MiB".into()))?;
                }
                yield chunk;
            }
        });
        let bytes = async_stream::try_stream! {
            let events = bounded.eventsource();
            futures::pin_mut!(events);
            while let Some(event) = events.next().await {
                let event = event.map_err(|e| TransportError::Network(format!("invalid upstream SSE: {e}")))?;
                if event.data == "[DONE]" { continue; }
                let value: Value = serde_json::from_str(&event.data)
                    .map_err(|e| TransportError::Network(format!("invalid upstream event JSON: {e}")))?;
                // Preserve the complete JSON payload, including fields Codex's typed events omit.
                let mut encoded = String::new();
                if !event.event.is_empty() { encoded.push_str(&format!("event: {}\n", event.event)); }
                if !event.id.is_empty() { encoded.push_str(&format!("id: {}\n", event.id)); }
                for line in event.data.lines() { encoded.push_str(&format!("data: {line}\n")); }
                encoded.push('\n');
                let bytes = Bytes::from(encoded);
                if sender.send(WireEvent { bytes: bytes.clone(), value }).await.is_err() { break; }
                // The original ResponsesClient receives and parses this same event.
                yield bytes;
            }
        };
        Ok(StreamResponse {
            status: upstream.status,
            headers: upstream.headers,
            bytes: Box::pin(bytes),
        })
    }
}

/// Keep the raw catalog while the release's ModelsClient performs native decoding.
/// 0.155.0 has no list_models_raw method, so capture at its transport boundary.
pub struct ModelCatalogTransport {
    pub inner: codex_api::ReqwestTransport,
    pub body: std::sync::Arc<std::sync::OnceLock<Bytes>>,
}
impl HttpTransport for ModelCatalogTransport {
    async fn execute(&self, request: Request) -> Result<Response, TransportError> {
        let response =
            collect_response(self.inner.stream(request).await?, 16 * 1024 * 1024).await?;
        let _ = self.body.set(response.body.clone());
        Ok(response)
    }
    async fn stream(&self, request: Request) -> Result<StreamResponse, TransportError> {
        self.inner.stream(request).await
    }
}

/// Keep the full response while the original standalone client validates its schema.
pub struct StandaloneTransport {
    pub inner: codex_api::ReqwestTransport,
    pub response: std::sync::Arc<std::sync::OnceLock<(http::HeaderMap, Bytes)>>,
}
impl HttpTransport for StandaloneTransport {
    async fn execute(&self, request: Request) -> Result<Response, TransportError> {
        let response =
            collect_response(self.inner.stream(request).await?, MAX_RESPONSE_BYTES).await?;
        let _ = self
            .response
            .set((response.headers.clone(), response.body.clone()));
        Ok(response)
    }
    async fn stream(&self, request: Request) -> Result<StreamResponse, TransportError> {
        self.inner.stream(request).await
    }
}

async fn collect_response(
    mut response: StreamResponse,
    limit: usize,
) -> Result<Response, TransportError> {
    let mut body = Vec::new();
    while let Some(chunk) = response.bytes.next().await {
        let chunk = chunk?;
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(TransportError::Network(format!(
                "upstream response exceeded {limit} bytes"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(Response {
        status: response.status,
        headers: response.headers,
        body: Bytes::from(body),
    })
}
