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

/// Keep the full response while the original standalone client validates its schema.
pub struct StandaloneTransport {
    pub inner: codex_api::ReqwestTransport,
    pub response: std::sync::Arc<std::sync::OnceLock<(http::HeaderMap, Bytes)>>,
}

/// Preserve error bodies and redirects which ReqwestTransport converts to UTF-8 errors.
/// Request body preparation, HTTP, TLS and proxy routing still use the original client.
pub struct ProxyTransport {
    pub http: codex_http_client::HttpClient,
    pub audio_headers: Option<http::HeaderMap>,
    pub configured_chatgpt_cookies_present: bool,
    pub failed: std::sync::Arc<std::sync::Mutex<Option<Response>>>,
}
impl HttpTransport for ProxyTransport {
    async fn execute(&self, request: Request) -> Result<Response, TransportError> {
        collect_response(self.stream(request).await?, MAX_RESPONSE_BYTES).await
    }
    async fn stream(&self, request: Request) -> Result<StreamResponse, TransportError> {
        let prepared = request
            .prepare_body_for_send()
            .map_err(TransportError::Build)?;
        if let Some(default_headers) = &self.audio_headers {
            let headers = &prepared.headers;
            tracing::debug!(
                user_agent = headers
                    .get("user-agent")
                    .or_else(|| default_headers.get("user-agent"))
                    .and_then(|v| v.to_str().ok()),
                originator = headers
                    .get("originator")
                    .or_else(|| default_headers.get("originator"))
                    .and_then(|v| v.to_str().ok()),
                accept = headers
                    .get("accept")
                    .or_else(|| default_headers.get("accept"))
                    .map(|v| v.to_str().unwrap_or("<non-UTF8>"))
                    .unwrap_or("*/*"),
                content_type = headers
                    .get("content-type")
                    .or_else(|| default_headers.get("content-type"))
                    .and_then(|v| v.to_str().ok()),
                request_bytes = prepared.body.as_ref().map_or(0, Bytes::len),
                explicit_cookie_header_present = prepared.headers.contains_key("cookie"),
                shared_cookie_jar_enabled = true,
                configured_chatgpt_cookies_present = self.configured_chatgpt_cookies_present,
                "audio upstream wire request"
            );
        }
        let mut outgoing = self
            .http
            .request(request.method, &request.url)
            .headers(prepared.headers);
        if let Some(timeout) = request.timeout {
            outgoing = outgoing.timeout(timeout);
        }
        if let Some(body) = prepared.body {
            outgoing = outgoing.body(body);
        }
        let response = outgoing
            .send()
            .await
            .map_err(|e| TransportError::Network(e.without_url().to_string()))?;
        if self.audio_headers.is_some() {
            let headers = response.headers();
            tracing::debug!(
                http_version = ?response.version(),
                server = headers.get("server").and_then(|v| v.to_str().ok()),
                cf_mitigated = headers.get("cf-mitigated").and_then(|v| v.to_str().ok()),
                request_id = headers.get("x-oai-request-id").or_else(|| headers.get("x-request-id")).and_then(|v| v.to_str().ok()),
                cf_ray = headers.get("cf-ray").and_then(|v| v.to_str().ok()),
                "audio upstream wire response"
            );
        }
        let response = StreamResponse {
            status: response.status(),
            headers: response.headers().clone(),
            bytes: Box::pin(
                response
                    .bytes_stream()
                    .map(|r| r.map_err(|e| TransportError::Network(e.without_url().to_string()))),
            ),
        };
        if !response.status.is_success() {
            let response = collect_response(response, MAX_RESPONSE_BYTES).await?;
            let error = TransportError::Http {
                status: response.status,
                url: None,
                headers: Some(response.headers.clone()),
                retry_after: codex_http_client::RetryAfter::from_headers(&response.headers),
                body: std::str::from_utf8(&response.body).ok().map(str::to_owned),
            };
            *self
                .failed
                .lock()
                .map_err(|_| TransportError::Network("response capture failed".into()))? =
                Some(response);
            return Err(error);
        }
        Ok(response)
    }
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

pub(crate) async fn collect_response(
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
