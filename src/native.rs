//! Transparent first-party routes over Codex's HTTP transport and authentication.
use crate::{
    backend::Backend,
    error::GatewayError,
    server::{Gateway, timeout_error},
};
use axum::{
    body::{Body, Bytes},
    extract::{FromRequest, Request, State, WebSocketUpgrade},
    response::Response,
};
use codex_api::{ApiError, RawClient};
use codex_client::{RequestBody, StreamResponse};
use codex_http_client::{ClientRouteClass, HttpClientBuilder};
use futures::StreamExt;
use http::{HeaderMap, Method, StatusCode};
use std::{future::Future, sync::Arc};

const GATEWAY_AUTH: &str = "x-codex-gateway-authorization";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Codex,
    ChatGpt,
    Platform,
    Auth,
    Costs,
    Metrics,
    Sentry,
    Distribution,
    Static,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AuthPolicy {
    Subscription,
    Passthrough,
    None,
}
pub struct ProxyRequest {
    pub origin: Origin,
    pub method: Method,
    pub path: String,
    pub headers: HeaderMap,
    pub body: Option<RequestBody>,
    pub auth: AuthPolicy,
}

pub(crate) use crate::server::authorize;

pub fn response_headers(headers: &HeaderMap) -> HeaderMap {
    let mut clean = crate::headers::transport_headers(headers);
    clean.remove("content-length");
    clean
}

pub(crate) fn websocket_headers(headers: &HeaderMap) -> HeaderMap {
    let mut clean = response_headers(headers);
    for name in [
        "sec-websocket-accept",
        "sec-websocket-protocol",
        "sec-websocket-extensions",
    ] {
        clean.remove(name);
    }
    clean
}

pub fn request_headers(headers: &HeaderMap, policy: AuthPolicy) -> HeaderMap {
    let mut clean = crate::headers::request_headers(headers);
    for name in ["host", "content-length"] {
        clean.remove(name);
    }
    if policy != AuthPolicy::Passthrough {
        for name in [
            "authorization",
            "chatgpt-account-id",
            "x-openai-actor-authorization",
            "x-openai-fedramp",
            "cookie",
            "openai-organization",
            "openai-project",
        ] {
            clean.remove(name);
        }
    }
    clean
}

fn route(path: &str) -> Result<(Origin, String, AuthPolicy), GatewayError> {
    use AuthPolicy::*;
    use Origin::*;
    let (origin, path) = if let Some(path) = path
        .strip_prefix("/backend-api/codex/")
        .or_else(|| path.strip_prefix("/codex/"))
    {
        (Codex, path.to_owned())
    } else if let Some(path) = path.strip_prefix("/backend-api/") {
        (ChatGpt, path.to_owned())
    } else if let Some(path) = path.strip_prefix("/api/codex/") {
        (
            ChatGpt,
            if path.starts_with("ps/mcp") {
                path.to_owned()
            } else {
                format!("wham/{path}")
            },
        )
    } else if let Some(path) = path.strip_prefix("/auth/") {
        (Auth, path.to_owned())
    } else if let Some(path) = path.strip_prefix("/platform/") {
        (Platform, path.to_owned())
    } else if let Some(path) = path.strip_prefix("/distribution/chatgpt/") {
        (Distribution, path.to_owned())
    } else if let Some(path) = path.strip_prefix("/distribution/static/") {
        (Static, path.to_owned())
    } else {
        match path {
            "/telemetry/costs" => (Costs, "v1/analytics/codex/turn-costs".into()),
            "/telemetry/metrics" => (Metrics, "otlp/v1/metrics".into()),
            "/telemetry/sentry" => (Sentry, "api/4510195390611458/envelope/".into()),
            _ => {
                return Err(GatewayError {
                    status: StatusCode::NOT_FOUND,
                    code: "invalid_request_error",
                    message: "unknown endpoint".into(),
                    upstream_response: Option::None,
                });
            }
        }
    };
    let auth = match origin {
        Codex => Subscription,
        ChatGpt => match path.as_str() {
            "wham/remote/control/server"
            | "wham/remote/control/server/pair"
            | "wham/remote/control/server/pair/status" => Passthrough,
            "wham/agent-identities/jwks" | "wham/app/appcast" | "plugins/export/curated" => None,
            _ => Subscription,
        },
        Auth => match path.as_str() {
            "api/accounts/v1/agent/register" => Subscription,
            "api/accounts/v1/user-auth-credential/whoami" | "oauth/authorize" => Passthrough,
            _ => None,
        },
        Platform | Costs => Passthrough,
        Metrics | Sentry | Distribution | Static => None,
    };
    Ok((origin, path, auth))
}

pub async fn handle(
    State(gateway): State<Arc<Gateway>>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, GatewayError> {
    authorize(&gateway, &headers)?;
    let (origin, mut path, auth) = route(request.uri().path())?;
    if auth == AuthPolicy::Passthrough
        && gateway.key.is_some()
        && !headers.contains_key(GATEWAY_AUTH)
    {
        return Err(GatewayError::invalid(
            "use X-Codex-Gateway-Authorization for the gateway key and Authorization for the upstream credential",
        ));
    }
    if let Some(query) = request.uri().query() {
        path.push('?');
        path.push_str(query);
    }
    let mut upstream_headers = request_headers(&headers, auth);
    if headers
        .get("upgrade")
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"websocket"))
    {
        let (mut parts, body) = request.into_parts();
        use axum::extract::FromRequestParts;
        let ws = WebSocketUpgrade::from_request_parts(&mut parts, &())
            .await
            .map_err(|e| GatewayError::invalid(e.to_string()))?;
        for name in [
            "sec-websocket-key",
            "sec-websocket-version",
            "sec-websocket-extensions",
        ] {
            upstream_headers.remove(name);
        }
        drop(body);
        let permit = gateway
            .concurrency
            .clone()
            .try_acquire_owned()
            .map_err(|_| busy())?;
        let endpoint = match (origin, path.split('?').next().unwrap_or_default()) {
            (Origin::Codex, "guardian") => Some("/guardian"),
            (Origin::Codex, "guardian-classifier") => Some("/guardian-classifier"),
            _ => None,
        };
        if let Some(endpoint) = endpoint {
            let (socket, upstream) = tokio::time::timeout(
                gateway.timeout,
                gateway.backend.connect_websocket_endpoint(
                    upstream_headers,
                    endpoint,
                    path.split_once('?').map(|(_, q)| q),
                ),
            )
            .await
            .map_err(|_| timeout_error())??;
            let idle = gateway.timeout;
            let protocol = upstream
                .get("sec-websocket-protocol")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let mut response = ws.protocols(protocol).on_upgrade(move |client| async move {
                let _permit = permit;
                crate::websocket::relay(client, socket, idle, false, &[], "").await;
            });
            response.headers_mut().extend(websocket_headers(&upstream));
            return Ok(response);
        }
        let (socket, upstream) = tokio::time::timeout(gateway.timeout, async {
            let (provider, auth) = gateway.backend.proxy_config(origin, auth).await?;
            auth.add_auth_headers(&mut upstream_headers);
            let url = provider.url_for_path(&path);
            let defaults = if matches!(origin, Origin::Codex | Origin::ChatGpt) {
                codex_login::default_client::default_headers()
            } else {
                HeaderMap::new()
            };
            codex_api::RealtimeWebsocketClient::new(provider, gateway.backend.factory.clone())
                .connect_raw(&url, upstream_headers, defaults)
                .await
                .map_err(GatewayError::from_api)
        })
        .await
        .map_err(|_| timeout_error())??;
        let protocol = upstream
            .headers()
            .get("sec-websocket-protocol")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let idle = gateway.timeout;
        let mut response = ws.protocols(protocol).on_upgrade(move |client| async move {
            let _permit = permit;
            crate::websocket::relay(client, socket, idle, false, &[], "").await;
        });
        response
            .headers_mut()
            .extend(websocket_headers(upstream.headers()));
        return Ok(response);
    }
    // Authentication and limits are enforced before reading any request body.
    respond(gateway, headers, async move {
        let method = request.method().clone();
        let body = Bytes::from_request(request, &())
            .await
            .map_err(|e| crate::standalone::extraction_error(e.status(), e.body_text()))?;
        Ok(ProxyRequest {
            origin,
            method,
            path,
            headers: upstream_headers,
            body: (!body.is_empty()).then_some(RequestBody::Raw(body)),
            auth,
        })
    })
    .await
}

fn busy() -> GatewayError {
    GatewayError {
        upstream_response: None,
        status: StatusCode::TOO_MANY_REQUESTS,
        code: "gateway_busy",
        message: "gateway concurrency limit reached".into(),
    }
}

pub async fn respond(
    gateway: Arc<Gateway>,
    headers: HeaderMap,
    input: impl Future<Output = Result<ProxyRequest, GatewayError>>,
) -> Result<Response, GatewayError> {
    authorize(&gateway, &headers)?;
    let permit = gateway
        .concurrency
        .clone()
        .try_acquire_owned()
        .map_err(|_| busy())?;
    let timeout = gateway.timeout;
    let work = async {
        let request = input.await?;
        let rewrite_path = (request.origin == Origin::ChatGpt).then(|| {
            format!(
                "/backend-api/{}",
                request.path.split('?').next().unwrap_or_default()
            )
        });
        let method = request.method.clone();
        let mut upstream = gateway.backend.proxy(request).await?;
        let mut headers = response_headers(&upstream.headers);
        if method == Method::HEAD
            && let Some(length) = upstream.headers.get("content-length")
        {
            headers.insert("content-length", length.clone());
        }
        if let (Some(transfers), Some(path)) = (&gateway.transfers, rewrite_path)
            && upstream.status.is_success()
            && headers
                .get("content-type")
                .is_some_and(|v| v.as_bytes().starts_with(b"application/json"))
        {
            let response = crate::transport::collect_response(upstream, 64 * 1024 * 1024)
                .await
                .map_err(|e| GatewayError::from_api(e.into()))?;
            let mut body = response.body;
            if let Ok(mut value) = serde_json::from_slice(&body)
                && transfers.rewrite_response(&method, &path, &mut value)?
            {
                body = serde_json::to_vec(&value)
                    .map_err(GatewayError::internal)?
                    .into();
                headers.remove("etag");
            }
            let mut out = Response::new(Body::from(body));
            *out.status_mut() = response.status;
            *out.headers_mut() = headers;
            return Ok(out);
        }
        let status = upstream.status;
        let stream = async_stream::try_stream! {
            let _permit = permit;
            while let Some(chunk) = tokio::time::timeout(timeout, upstream.bytes.next()).await.map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "upstream stream idle timeout"))? {
                yield chunk.map_err(|e| std::io::Error::other(e.to_string()))?;
            }
        };
        let stream: std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<Bytes, std::io::Error>> + Send>,
        > = Box::pin(stream);
        let mut response = Response::new(Body::from_stream(stream));
        *response.status_mut() = status;
        *response.headers_mut() = headers;
        Ok(response)
    };
    tokio::time::timeout(timeout, work)
        .await
        .map_err(|_| timeout_error())?
}

impl Backend {
    pub(crate) async fn proxy_config(
        &self,
        origin: Origin,
        policy: AuthPolicy,
    ) -> Result<(codex_api::Provider, codex_api::SharedAuthProvider), GatewayError> {
        let mut provider = self
            .provider
            .api_provider()
            .await
            .map_err(GatewayError::internal)?;
        if origin != Origin::Codex {
            provider.base_url = match origin {
                Origin::ChatGpt => &self.chatgpt_base_url,
                Origin::Platform => &self.platform_base_url,
                Origin::Auth => &self.auth_base_url,
                Origin::Costs => "https://api.chatgpt.com",
                Origin::Metrics => "https://ab.chatgpt.com",
                Origin::Sentry => "https://o33249.ingest.us.sentry.io",
                Origin::Distribution => "https://chatgpt.com",
                Origin::Static => "https://persistent.oaistatic.com",
                Origin::Codex => unreachable!(),
            }
            .to_owned();
            provider.headers.clear();
            provider.query_params = None;
            provider.retry.max_attempts = 0;
        }
        let auth = if policy == AuthPolicy::Subscription {
            let auth = self.provider.auth().await.ok_or_else(GatewayError::auth)?;
            if self.subscription_only && !auth.is_chatgpt_auth() {
                return Err(GatewayError::auth());
            }
            if origin == Origin::Codex {
                self.provider
                    .api_auth()
                    .await
                    .map_err(GatewayError::internal)?
            } else {
                codex_model_provider::auth_provider_from_auth(&auth)
            }
        } else {
            codex_model_provider::unauthenticated_auth_provider()
        };
        Ok((provider, auth))
    }

    pub async fn proxy(&self, request: ProxyRequest) -> Result<StreamResponse, GatewayError> {
        let mut recovery = self.auth.unauthorized_recovery();
        let original = self.provider.auth().await;
        let account = original
            .as_ref()
            .map(|a| (a.get_account_id(), a.get_chatgpt_user_id()));
        loop {
            let (mut provider, auth) = self.proxy_config(request.origin, request.auth).await?;
            if request.auth == AuthPolicy::Subscription
                && self
                    .provider
                    .auth()
                    .await
                    .as_ref()
                    .map(|a| (a.get_account_id(), a.get_chatgpt_user_id()))
                    != account
            {
                return Err(GatewayError::auth());
            }
            let mut path = request.path.clone();
            if let Some(params) = provider.query_params.take() {
                for (key, value) in params {
                    path.push(if path.contains('?') { '&' } else { '?' });
                    path.push_str(&key);
                    path.push('=');
                    path.push_str(&value);
                }
            }
            let url = provider.url_for_path(&path);
            let mut builder = HttpClientBuilder::new()
                .without_redirects()
                .without_request_logging();
            if matches!(request.origin, Origin::Codex | Origin::ChatGpt) {
                builder = builder.with_chatgpt_cookies(&self.factory);
            }
            if request.origin == Origin::Codex {
                builder = builder.default_headers(codex_login::default_client::default_headers());
            }
            let http = builder
                .build_respecting_outbound_proxy_policy(&self.factory, &url, ClientRouteClass::Api)
                .map_err(GatewayError::internal)?;
            let mut headers = request.headers.clone();
            if request.origin == Origin::ChatGpt && !headers.contains_key("user-agent") {
                headers.insert(
                    "user-agent",
                    codex_login::default_client::get_codex_user_agent()
                        .parse()
                        .map_err(GatewayError::internal)?,
                );
            }
            let failed = Arc::new(std::sync::Mutex::new(None));
            let client = RawClient::new(
                crate::transport::ProxyTransport {
                    http,
                    failed: failed.clone(),
                },
                provider,
                auth,
            );
            match client
                .stream(request.method.clone(), &path, headers, request.body.clone())
                .await
            {
                Ok(response) => return Ok(response),
                Err(ApiError::Transport(ref e))
                    if request.auth == AuthPolicy::Subscription
                        && self.provider.is_recoverable_auth_error(e)
                        && recovery.has_next() =>
                {
                    recovery.next().await.map_err(|_| GatewayError::auth())?;
                }
                Err(e) => {
                    if matches!(
                        e,
                        ApiError::Transport(codex_client::TransportError::Http { .. })
                    ) && let Some(response) =
                        failed.lock().map_err(GatewayError::internal)?.take()
                    {
                        return Ok(StreamResponse {
                            status: response.status,
                            headers: response.headers,
                            bytes: Box::pin(futures::stream::once(
                                async move { Ok(response.body) },
                            )),
                        });
                    }
                    return Err(GatewayError::from_api(e));
                }
            }
        }
    }
}
