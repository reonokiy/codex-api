//! Expiring capabilities for storage URLs returned by the authenticated Codex backend.
use crate::error::GatewayError;
use axum::{body::Body, extract::Request, response::Response};
use codex_http_client::{ClientRouteClass, HttpClientFactory, RouteAwareClientPool};
use futures::StreamExt;
use http::{HeaderMap, Method, StatusCode, Uri};
use serde_json::Value;
use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

const MAX_TRANSFER_BYTES: u64 = codex_api::OPENAI_FILE_UPLOAD_LIMIT_BYTES;
const MAX_URL_BYTES: usize = 16 * 1024;

#[derive(Clone)]
struct Transfer {
    url: String,
    method: Method,
    expires: Instant,
}

/// Shares only opaque, method-bound capabilities; account credentials never enter this store.
pub struct Transfers {
    public_base_url: String,
    client: RouteAwareClientPool,
    handles: Mutex<HashMap<String, Transfer>>,
    concurrency: Arc<Semaphore>,
    ttl: Duration,
    capacity: usize,
    timeout: Duration,
}

impl Transfers {
    pub fn new(
        factory: HttpClientFactory,
        public_base_url: &str,
        timeout: Duration,
    ) -> Result<Self, GatewayError> {
        Self::with_limits(
            factory,
            public_base_url,
            timeout,
            Duration::from_secs(900),
            1024,
        )
    }

    /// Explicit limits allow deployments and tests to bound retained capabilities.
    pub fn with_limits(
        factory: HttpClientFactory,
        public_base_url: &str,
        timeout: Duration,
        ttl: Duration,
        capacity: usize,
    ) -> Result<Self, GatewayError> {
        let public_base_url = public_base_url.trim_end_matches('/');
        let uri = validate_url(public_base_url)?;
        if uri.query().is_some() || timeout.is_zero() || ttl.is_zero() || capacity == 0 {
            return Err(GatewayError::invalid("invalid transfer configuration"));
        }
        Ok(Self {
            public_base_url: public_base_url.to_owned(),
            // Original route selection, custom CA, redirects and streaming transport, with no cookies
            // or request logging. Neither the factory nor this pool receives account auth headers.
            client: RouteAwareClientPool::new_without_request_logging(
                factory,
                ClientRouteClass::Api,
            )
            .with_legacy_custom_ca_fallback(),
            handles: Mutex::new(HashMap::new()),
            concurrency: Arc::new(Semaphore::new(8)),
            ttl,
            capacity,
            timeout,
        })
    }

    /// Call only for a successful authenticated upstream response, using its original method/path.
    /// Matches known response schemas, never arbitrary nested URLs or a client-supplied target.
    pub fn rewrite_response(
        &self,
        upstream_method: &Method,
        upstream_path: &str,
        body: &mut Value,
    ) -> Result<bool, GatewayError> {
        let mut fields = Vec::new();
        if *upstream_method == Method::POST
            && matches!(
                upstream_path,
                "/backend-api/files" | "/backend-api/public/plugins/workspace/upload-url"
            )
            && body.get("file_id").and_then(Value::as_str).is_some()
        {
            if body.get("upload_url").and_then(Value::as_str).is_some() {
                fields.push(("/upload_url".to_owned(), Method::PUT));
            }
        } else if *upstream_method == Method::GET
            && let Some(path) = upstream_path.strip_prefix("/backend-api/ps/plugins/")
        {
            if matches!(
                path,
                "list" | "installed" | "search" | "workspace/shared" | "workspace/created"
            ) {
                if let Some(plugins) = body.get("plugins").and_then(Value::as_array) {
                    for (index, plugin) in plugins.iter().enumerate() {
                        if plugin.get("id").and_then(Value::as_str).is_some()
                            && plugin
                                .pointer("/release/bundle_download_url")
                                .and_then(Value::as_str)
                                .is_some()
                        {
                            fields.push((
                                format!("/plugins/{index}/release/bundle_download_url"),
                                Method::GET,
                            ));
                        }
                    }
                }
            } else if !path.is_empty()
                && !path.contains('/')
                && body.get("id").and_then(Value::as_str) == Some(path)
                && body
                    .pointer("/release/bundle_download_url")
                    .and_then(Value::as_str)
                    .is_some()
            {
                fields.push(("/release/bundle_download_url".to_owned(), Method::GET));
            }
        }
        if fields.is_empty() {
            return Ok(false);
        }
        if fields.len() > self.capacity {
            return Err(unavailable());
        }
        let mut registrations = Vec::with_capacity(fields.len());
        for (pointer, method) in fields {
            let url = body
                .pointer(&pointer)
                .and_then(Value::as_str)
                .expect("selected URL");
            validate_url(url)?;
            registrations.push((pointer, method, url.to_owned()));
        }
        let now = Instant::now();
        let mut handles = self.handles.lock().map_err(|_| unavailable())?;
        handles.retain(|_, transfer| transfer.expires > now);
        if registrations.len() > self.capacity.saturating_sub(handles.len()) {
            return Err(unavailable());
        }
        for (pointer, method, url) in registrations {
            let handle = uuid::Uuid::new_v4().simple().to_string();
            handles.insert(
                handle.clone(),
                Transfer {
                    url,
                    method,
                    expires: now + self.ttl,
                },
            );
            *body.pointer_mut(&pointer).expect("selected URL") =
                Value::String(format!("{}/transfers/{handle}", self.public_base_url));
        }
        Ok(true)
    }

    /// Serve `/transfers/{handle}` directly: possession of the handle authorizes its fixed method.
    pub async fn serve(&self, handle: &str, request: Request) -> Result<Response, GatewayError> {
        let transfer = {
            let now = Instant::now();
            let mut handles = self.handles.lock().map_err(|_| unavailable())?;
            handles.retain(|_, transfer| transfer.expires > now);
            handles
                .get(handle)
                .cloned()
                .ok_or_else(|| error(StatusCode::NOT_FOUND, "unknown or expired transfer"))?
        };
        if request.method() != transfer.method {
            return Err(error(
                StatusCode::METHOD_NOT_ALLOWED,
                "transfer method is not allowed",
            ));
        }
        let permit = self
            .concurrency
            .clone()
            .try_acquire_owned()
            .map_err(|_| unavailable())?;
        let (parts, body) = request.into_parts();
        let headers = transfer_headers(&parts.headers, true);
        if headers
            .get(http::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|len| len > MAX_TRANSFER_BYTES)
        {
            return Err(error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "transfer exceeds 512 MiB",
            ));
        }
        let mut source = body.into_data_stream();
        let upload: futures::stream::BoxStream<'static, io::Result<bytes::Bytes>> = Box::pin(
            async_stream::try_stream! {
                let mut total = 0u64;
                while let Some(chunk) = source.next().await {
                    let chunk = chunk.map_err(|_| io::Error::other("transfer body could not be read"))?;
                    total = total.saturating_add(chunk.len() as u64);
                    if total > MAX_TRANSFER_BYTES { Err(io::Error::other("transfer exceeds 512 MiB"))?; }
                    yield chunk;
                }
            },
        );
        let deadline = tokio::time::Instant::now() + self.timeout;
        let upstream = tokio::time::timeout_at(
            deadline,
            self.client
                .request(transfer.method, &transfer.url)
                .headers(headers)
                .timeout(self.timeout)
                .body_stream(upload)
                .send(),
        )
        .await
        .map_err(|_| error(StatusCode::GATEWAY_TIMEOUT, "transfer timed out"))?
        .map_err(|_| error(StatusCode::BAD_GATEWAY, "upstream transfer failed"))?;
        let status = upstream.status();
        let headers = transfer_headers(upstream.headers(), false);
        if upstream
            .content_length()
            .is_some_and(|len| len > MAX_TRANSFER_BYTES)
        {
            return Err(error(
                StatusCode::BAD_GATEWAY,
                "upstream transfer exceeds 512 MiB",
            ));
        }
        let mut source = upstream.bytes_stream();
        let download: futures::stream::BoxStream<'static, io::Result<bytes::Bytes>> = Box::pin(
            async_stream::try_stream! {
                let _permit = permit;
                let mut total = 0u64;
                loop {
                    let chunk = tokio::time::timeout_at(deadline, source.next()).await
                        .map_err(|_| io::Error::other("transfer timed out"))?;
                    let Some(chunk) = chunk else { break; };
                    let chunk = chunk.map_err(|_| io::Error::other("upstream transfer body failed"))?;
                    total = total.saturating_add(chunk.len() as u64);
                    if total > MAX_TRANSFER_BYTES { Err(io::Error::other("upstream transfer exceeds 512 MiB"))?; }
                    yield chunk;
                }
            },
        );
        let mut response = Response::new(Body::from_stream(download));
        *response.status_mut() = status;
        *response.headers_mut() = headers;
        Ok(response)
    }
}

fn validate_url(url: &str) -> Result<Uri, GatewayError> {
    let uri = url
        .parse::<Uri>()
        .map_err(|_| GatewayError::invalid("invalid transfer URL"))?;
    let host = uri.host().unwrap_or("");
    let valid_scheme = uri.scheme_str() == Some("https")
        || (uri.scheme_str() == Some("http")
            && (host.eq_ignore_ascii_case("localhost")
                || host
                    .trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())));
    if url.len() > MAX_URL_BYTES
        || !valid_scheme
        || host.is_empty()
        || url.contains('#')
        || uri.authority().is_some_and(|v| v.as_str().contains('@'))
    {
        return Err(GatewayError::invalid("invalid transfer URL"));
    }
    Ok(uri)
}

fn transfer_headers(headers: &HeaderMap, request: bool) -> HeaderMap {
    let headers = crate::headers::transport_headers(headers);
    let mut forwarded = HeaderMap::new();
    for (name, value) in &headers {
        let name_str = name.as_str();
        if name_str == "host"
            || (request
                && (crate::headers::client_header(name_str)
                    || matches!(
                        name_str,
                        "authorization"
                            | "cookie"
                            | "x-api-key"
                            | "api-key"
                            | "x-gateway-api-key"
                            | "chatgpt-account-id"
                            | "x-openai-actor-authorization"
                            | "x-openai-fedramp"
                    )
                    || name_str.starts_with("x-codex-")
                    || name_str.starts_with("x-openai-internal-")
                    || name_str.starts_with("x-gateway-")))
        {
            continue;
        }
        forwarded.append(name.clone(), value.clone());
    }
    forwarded
}

fn unavailable() -> GatewayError {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "transfer capacity is exhausted",
    )
}

fn error(status: StatusCode, message: &str) -> GatewayError {
    GatewayError {
        upstream_response: None,
        status,
        code: "transfer_error",
        message: message.to_owned(),
    }
}
