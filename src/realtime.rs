//! Realtime call negotiation and transparent native control sockets.
use crate::{
    backend::Backend,
    error::GatewayError,
    native::{AuthPolicy, Origin, ProxyRequest},
    server::{Gateway, timeout_error},
};
use axum::{
    body::Bytes,
    extract::{FromRequest, Multipart, OriginalUri, Request, State, WebSocketUpgrade},
    response::Response,
};
use codex_api::{ApiError, RealtimeWebsocketClient};
use codex_client::RequestBody;
use http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use serde_json::Value;
use std::sync::Arc;

pub async fn calls(
    State(gateway): State<Arc<Gateway>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, GatewayError> {
    crate::native::respond(gateway, headers, async move {
        let auth = credential_policy(request.headers());
        let mut headers = crate::native::request_headers(request.headers(), auth);
        let passthrough = matches!(auth, AuthPolicy::Passthrough);
        if uri.path().ends_with("/live/sessions") && !passthrough {
            return Err(modern_live_auth_error());
        }
        if passthrough {
            let body = Bytes::from_request(request, &())
                .await
                .map_err(|e| crate::standalone::extraction_error(e.status(), e.body_text()))?;
            return Ok(ProxyRequest {
                origin: Origin::Platform,
                method: Method::POST,
                path: relative_path(&uri)?,
                headers,
                body: Some(RequestBody::Raw(body)),
                auth,
            });
        }
        let media_type = request
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .unwrap_or_default()
            .trim()
            .to_owned();
        let body = match media_type.as_str() {
            "multipart/form-data" => {
                let mut multipart = Multipart::from_request(request, &())
                    .await
                    .map_err(|e| crate::standalone::extraction_error(e.status(), e.body_text()))?;
                let mut fields = serde_json::Map::new();
                while let Some(field) = multipart
                    .next_field()
                    .await
                    .map_err(|e| crate::standalone::extraction_error(e.status(), e.body_text()))?
                {
                    let name = field.name().unwrap_or_default().to_owned();
                    if !matches!(name.as_str(), "sdp" | "session") || fields.contains_key(&name) {
                        return Err(GatewayError::invalid(
                            "expected one sdp field and one session field",
                        ));
                    }
                    let text = field.text().await.map_err(|e| {
                        crate::standalone::extraction_error(e.status(), e.body_text())
                    })?;
                    let value = if name == "session" {
                        let value: Value = serde_json::from_str(&text).map_err(|e| {
                            GatewayError::invalid(format!("invalid session JSON: {e}"))
                        })?;
                        if !value.is_object() {
                            return Err(GatewayError::invalid("session must be an object"));
                        }
                        value
                    } else {
                        Value::String(text)
                    };
                    fields.insert(name, value);
                }
                if !fields.contains_key("sdp") || !fields.contains_key("session") {
                    return Err(GatewayError::invalid("sdp and session are required"));
                }
                headers.insert("content-type", HeaderValue::from_static("application/json"));
                RequestBody::Json(Value::Object(fields))
            }
            "application/json" | "application/sdp" => {
                let bytes = Bytes::from_request(request, &())
                    .await
                    .map_err(|e| crate::standalone::extraction_error(e.status(), e.body_text()))?;
                headers.insert("content-type", HeaderValue::from_str(&media_type).unwrap());
                RequestBody::Raw(bytes)
            }
            _ => {
                return Err(crate::standalone::extraction_error(
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "use application/sdp, application/json, or multipart/form-data",
                ));
            }
        };
        let query = uri.query().unwrap_or_default();
        let live = uri.path().ends_with("/live");
        let mut path = "realtime/calls".to_owned();
        if !query.is_empty() {
            path.push('?');
            path.push_str(query);
        }
        // The pinned native live/WebRTC client uses this backend AVAS route.
        if live && query.is_empty() {
            path.push_str("?intent=quicksilver&architecture=avas");
        }
        Ok(ProxyRequest {
            origin: Origin::Codex,
            method: Method::POST,
            path,
            headers,
            body: Some(body),
            auth: AuthPolicy::Subscription,
        })
    })
    .await
}

pub async fn upgrade(
    State(gateway): State<Arc<Gateway>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, GatewayError> {
    crate::native::authorize(&gateway, &headers)?;
    let permit = gateway
        .concurrency
        .clone()
        .try_acquire_owned()
        .map_err(|_| GatewayError {
            upstream_response: None,
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "gateway_busy",
            message: "gateway concurrency limit reached".into(),
        })?;
    let path = socket_path(&uri)?;
    let auth = credential_policy(&headers);
    if (path.split('?').next() == Some("live/sessions") || path.starts_with("live/sessions/"))
        && !matches!(auth, AuthPolicy::Passthrough)
    {
        return Err(modern_live_auth_error());
    }
    let mut headers = crate::native::request_headers(&headers, auth);
    // Each endpoint negotiates its own WebSocket framing and masking key.
    for name in [
        "sec-websocket-key",
        "sec-websocket-version",
        "sec-websocket-extensions",
    ] {
        headers.remove(name);
    }
    let (upstream, upstream_headers) = tokio::time::timeout(
        gateway.timeout,
        gateway.backend.realtime_socket(&path, headers, auth),
    )
    .await
    .map_err(|_| timeout_error())??;
    let idle = gateway.timeout;
    let protocols = upstream_headers
        .get("sec-websocket-protocol")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut response = ws
        .protocols(protocols)
        .max_message_size(16 * 1024 * 1024)
        .max_frame_size(16 * 1024 * 1024)
        .on_upgrade(move |client| async move {
            let _permit = permit;
            crate::websocket::relay(client, upstream, idle, false, &[], "").await;
        });
    response
        .headers_mut()
        .extend(crate::native::websocket_headers(&upstream_headers));
    Ok(response)
}

fn credential_policy(headers: &HeaderMap) -> AuthPolicy {
    if headers.contains_key("x-codex-gateway-authorization") {
        AuthPolicy::Passthrough
    } else {
        AuthPolicy::Subscription
    }
}

fn modern_live_auth_error() -> GatewayError {
    GatewayError::invalid(
        "current GPT-Live sessions require explicit API credentials: use X-Codex-Gateway-Authorization for the gateway key and Authorization for the upstream API key",
    )
}

fn socket_path(uri: &Uri) -> Result<String, GatewayError> {
    let relative = relative_path(uri)?;
    let path = relative.split('?').next().unwrap_or_default();
    let live_attach = path
        .strip_prefix("live/sessions/")
        .and_then(|path| path.strip_suffix("/attach"))
        .is_some_and(|id| !id.is_empty() && !id.contains('/'));
    if path != "realtime"
        && path != "live"
        && path != "live/sessions"
        && !live_attach
        && !path
            .strip_prefix("live/")
            .is_some_and(|id| !id.is_empty() && !id.contains('/'))
    {
        return Err(GatewayError::invalid("unknown realtime route"));
    }
    Ok(relative)
}

fn relative_path(uri: &Uri) -> Result<String, GatewayError> {
    let path = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or_default();
    ["/backend-api/codex/", "/codex/", "/v1/"]
        .into_iter()
        .find_map(|prefix| path.strip_prefix(prefix))
        .map(str::to_owned)
        .ok_or_else(|| GatewayError::invalid("unknown realtime route"))
}

impl Backend {
    async fn realtime_socket(
        &self,
        path: &str,
        headers: HeaderMap,
        policy: AuthPolicy,
    ) -> Result<(codex_api::RealtimeRawWebsocketConnection, HeaderMap), GatewayError> {
        let url = format!("{}/{}", self.platform_base_url.trim_end_matches('/'), path);
        if matches!(policy, AuthPolicy::Passthrough) {
            let provider = self
                .provider
                .api_provider()
                .await
                .map_err(GatewayError::internal)?;
            let (socket, response) = RealtimeWebsocketClient::new(provider)
                .connect_raw(
                    &url,
                    headers,
                    codex_login::default_client::default_headers(),
                )
                .await
                .map_err(GatewayError::from_api)?;
            return Ok((socket, response.headers().clone()));
        }
        let mut recovery = self.auth.unauthorized_recovery();
        let original = self.provider.auth().await.ok_or_else(GatewayError::auth)?;
        let account = (original.get_account_id(), original.get_chatgpt_user_id());
        let changes = self.auth.auth_change_receiver();
        let fallback = codex_model_provider::AgentIdentitySessionFallback::default();
        loop {
            let revision = *changes.borrow();
            let auth = self.provider.auth().await.ok_or_else(GatewayError::auth)?;
            if (self.subscription_only && !auth.is_chatgpt_auth())
                || (auth.get_account_id(), auth.get_chatgpt_user_id()) != account
            {
                return Err(GatewayError::auth());
            }
            let provider = self
                .provider
                .api_provider()
                .await
                .map_err(GatewayError::internal)?;
            let api_auth = self
                .provider
                .api_auth_for_scope(codex_model_provider::ProviderAuthScope {
                    agent_identity_policy: self.agent_identity_policy,
                    session_source: codex_protocol::protocol::SessionSource::Exec,
                    agent_identity_session_fallback: fallback.clone(),
                })
                .await
                .map_err(GatewayError::internal)?
                .auth;
            if *changes.borrow() != revision {
                continue;
            }
            let mut headers = headers.clone();
            // Native sidebands reuse call-creation auth, including the ChatGPT account id.
            api_auth.add_auth_headers(&mut headers);
            let client = RealtimeWebsocketClient::new(provider);
            match client
                .connect_raw(
                    &url,
                    headers,
                    codex_login::default_client::default_headers(),
                )
                .await
            {
                Ok((socket, response)) => return Ok((socket, response.headers().clone())),
                Err(ApiError::Transport(ref error))
                    if self.provider.is_recoverable_auth_error(error) && recovery.has_next() =>
                {
                    recovery.next().await.map_err(|_| GatewayError::auth())?;
                }
                Err(error) => return Err(GatewayError::from_api(error)),
            }
        }
    }
}
