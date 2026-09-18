use crate::{CODEX_REV, backend::Backend, error::GatewayError, request::CreateResponse};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Query, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use bytes::Bytes;
use codex_protocol::openai_models::ModelInfo;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

pub struct Gateway {
    pub backend: Backend,
    pub models: Vec<ModelInfo>,
    pub key: Option<String>,
    pub concurrency: Arc<Semaphore>,
    pub timeout: Duration,
    pub transfers: Option<crate::transfers::Transfers>,
}

pub fn router(gateway: Gateway) -> Router {
    Router::new()
        .route(
            "/healthz",
            get(|| async { Json(json!({"status":"ok", "codex_release":crate::CODEX_RELEASE, "codex_revision":CODEX_REV})) }),
        )
        .route("/v1/models", get(models))
        .route("/v1/files", post(crate::files::create))
        .route("/codex/memories/trace_summarize", post(crate::memories::summarize))
        .route("/backend-api/codex/memories/trace_summarize", post(crate::memories::summarize))
        .route("/codex/alpha/search", post(crate::search::search))
        .route("/backend-api/codex/alpha/search", post(crate::search::search))
        .route("/v1/images/generations", post(crate::images::generate))
        .route("/v1/images/edits", post(crate::images::edit))
        .route("/codex/images/generations", post(crate::images::generate))
        .route("/codex/images/edits", post(crate::images::edit))
        .route("/backend-api/codex/images/generations", post(crate::images::generate))
        .route("/backend-api/codex/images/edits", post(crate::images::edit))
        .route("/v1/responses", post(create).get(crate::websocket::upgrade))
        .route(
            "/codex/responses",
            post(native_create).get(crate::websocket::upgrade),
        )
        .route("/codex/models", get(native_models))
        .route(
            "/backend-api/codex/responses",
            post(native_create).get(crate::websocket::upgrade),
        )
        .route("/backend-api/codex/models", get(native_models))
        .route("/v1/responses/compact", post(compact))
        .route("/codex/responses/compact", post(compact))
        .route("/v1/realtime/calls", post(crate::realtime::calls))
        .route("/codex/realtime/calls", post(crate::realtime::calls))
        .route("/backend-api/codex/realtime/calls", post(crate::realtime::calls))
        .route("/v1/realtime", get(crate::realtime::upgrade))
        .route("/codex/realtime", get(crate::realtime::upgrade))
        .route("/backend-api/codex/realtime", get(crate::realtime::upgrade))
        .route("/v1/live", post(crate::realtime::calls).get(crate::realtime::upgrade))
        .route("/codex/live", post(crate::realtime::calls).get(crate::realtime::upgrade))
        .route("/backend-api/codex/live", post(crate::realtime::calls).get(crate::realtime::upgrade))
        .route("/v1/live/{call_id}", get(crate::realtime::upgrade))
        .route("/codex/live/{call_id}", get(crate::realtime::upgrade))
        .route("/backend-api/codex/live/{call_id}", get(crate::realtime::upgrade))
        .route("/v1/live/sessions", post(crate::realtime::calls).get(crate::realtime::upgrade))
        .route("/v1/live/sessions/{session_id}/attach", get(crate::realtime::upgrade))
        .route("/transfers/{handle}", axum::routing::any(transfer))
        .fallback(crate::native::handle)
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .with_state(Arc::new(gateway))
}
async fn transfer(
    State(gateway): State<Arc<Gateway>>,
    axum::extract::Path(handle): axum::extract::Path<String>,
    request: axum::extract::Request,
) -> Result<Response, GatewayError> {
    gateway
        .transfers
        .as_ref()
        .ok_or_else(|| {
            GatewayError::invalid("configure CODEX_GATEWAY_PUBLIC_URL to enable transfers")
        })?
        .serve(&handle, request)
        .await
}
pub(crate) fn authorize(gateway: &Gateway, headers: &HeaderMap) -> Result<(), GatewayError> {
    if let Some(key) = &gateway.key {
        let expected = format!("Bearer {key}");
        let actual = headers
            .get("x-codex-gateway-authorization")
            .or_else(|| headers.get("authorization"))
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        // Compare all equal-length bytes without an early mismatch exit.
        let equal = actual.len() == expected.len()
            && actual
                .bytes()
                .zip(expected.bytes())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0;
        if !equal {
            return Err(GatewayError {
                upstream_response: None,
                status: StatusCode::UNAUTHORIZED,
                code: "authentication_error",
                message: "invalid gateway API key".into(),
            });
        }
    }
    Ok(())
}
async fn models(
    State(gateway): State<Arc<Gateway>>,
    headers: HeaderMap,
) -> Result<Json<Value>, GatewayError> {
    authorize(&gateway, &headers)?;
    Ok(Json(
        json!({"object":"list", "data":gateway.models.iter().map(|v| json!({"id":v.slug,"object":"model","created":0,"owned_by":"openai"})).collect::<Vec<_>>()}),
    ))
}
async fn create(
    State(gateway): State<Arc<Gateway>>,
    headers: HeaderMap,
    body: Result<Json<CreateResponse>, JsonRejection>,
) -> Result<Response, GatewayError> {
    authorize(&gateway, &headers)?;
    let Json(input) = body.map_err(|e| GatewayError {
        upstream_response: None,
        status: e.status(),
        code: "invalid_request_error",
        message: e.body_text(),
    })?;
    let model = gateway
        .models
        .iter()
        .find(|v| v.slug == input.model)
        .ok_or_else(|| {
            GatewayError::invalid(
                "model is absent from the pinned Codex catalog; see GET /v1/models",
            )
        })?;
    let stream = input.stream;
    let session_id = headers
        .get("thread-id")
        .or_else(|| headers.get("session-id"))
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let request = input.into_codex(model, &session_id)?;
    if model.use_responses_lite {
        let mut headers = forward_request_headers(&headers);
        headers.insert(
            "x-openai-internal-codex-responses-lite",
            "true".parse().unwrap(),
        );
        headers.extend(codex_api::build_session_headers(
            Some(session_id.clone()),
            Some(session_id),
        ));
        respond(
            gateway,
            None,
            Some((
                serde_json::value::to_raw_value(&request).map_err(GatewayError::internal)?,
                headers,
            )),
            stream,
            true,
        )
        .await
    } else {
        respond(gateway, Some((request, session_id)), None, stream, true).await
    }
}

async fn native_models(
    State(gateway): State<Arc<Gateway>>,
    headers: HeaderMap,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<Response, GatewayError> {
    authorize(&gateway, &headers)?;
    let version = query
        .get("client_version")
        .map(String::as_str)
        .unwrap_or(crate::CODEX_RELEASE);
    if version.len() > 128
        || !version
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".-_+".contains(&c))
    {
        return Err(GatewayError::invalid("invalid client_version"));
    }
    let (body, etag) = tokio::time::timeout(
        gateway.timeout,
        gateway
            .backend
            .models(version, forward_request_headers(&headers)),
    )
    .await
    .map_err(|_| timeout_error())??;
    let mut response = Response::new(Body::from(body));
    response
        .headers_mut()
        .insert("content-type", "application/json".parse().unwrap());
    if let Some(etag) = etag {
        response
            .headers_mut()
            .insert("etag", etag.parse().map_err(GatewayError::internal)?);
    }
    Ok(response)
}

async fn native_create(
    State(gateway): State<Arc<Gateway>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, GatewayError> {
    authorize(&gateway, &headers)?;
    let body = match headers
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
    {
        None | Some("identity") => body.to_vec(),
        Some("zstd") => tokio::task::spawn_blocking(move || {
            use std::io::Read;
            let decoder = zstd::stream::read::Decoder::new(body.as_ref())
                .map_err(|_| GatewayError::invalid("invalid zstd request"))?;
            let mut decoded = Vec::new();
            decoder
                .take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut decoded)
                .map_err(|_| GatewayError::invalid("invalid zstd request"))?;
            if decoded.len() > 16 * 1024 * 1024 {
                return Err(GatewayError {
                    upstream_response: None,
                    status: StatusCode::PAYLOAD_TOO_LARGE,
                    code: "invalid_request_error",
                    message: "decoded body exceeds 16 MiB".into(),
                });
            }
            Ok(decoded)
        })
        .await
        .map_err(GatewayError::internal)??,
        _ => {
            return Err(GatewayError {
                upstream_response: None,
                status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
                code: "invalid_request_error",
                message: "supported content encodings: identity, zstd".into(),
            });
        }
    };
    let raw: Box<serde_json::value::RawValue> =
        serde_json::from_slice(&body).map_err(|e| GatewayError::invalid(e.to_string()))?;
    let body: Value =
        serde_json::from_str(raw.get()).map_err(|e| GatewayError::invalid(e.to_string()))?;
    if !body.is_object() || body["stream"] != true || !body["model"].is_string() {
        return Err(GatewayError::invalid(
            "native endpoint requires an object with model and stream: true",
        ));
    }
    let forwarded = forward_request_headers(&headers);
    respond(gateway, None, Some((raw, forwarded)), true, false).await
}

pub(crate) fn forward_request_headers(headers: &HeaderMap) -> HeaderMap {
    // Forward protocol metadata; credentials and routing authority belong to the gateway.
    let mut forwarded = HeaderMap::new();
    for name in [
        "session-id",
        "thread-id",
        "x-client-request-id",
        "originator",
        "user-agent",
        "openai-beta",
        "version",
        "x-codex-installation-id",
        "x-codex-routing-hint",
        "x-codex-turn-state",
        "x-codex-image-turn-id",
        "x-codex-turn-metadata",
        "x-codex-parent-thread-id",
        "x-codex-window-id",
        "x-openai-memgen-request",
        "x-openai-subagent",
        "x-responsesapi-include-timing-metrics",
        "x-codex-beta-features",
        "x-oai-attestation",
        "x-openai-internal-codex-responses-lite",
        "traceparent",
        "tracestate",
    ] {
        if let Some(value) = headers.get(name) {
            forwarded.insert(name, value.clone());
        }
    }
    forwarded
}

async fn respond(
    gateway: Arc<Gateway>,
    adapted: Option<(codex_api::ResponsesApiRequest, String)>,
    native: Option<(Box<serde_json::value::RawValue>, HeaderMap)>,
    stream: bool,
    aggregate_output: bool,
) -> Result<Response, GatewayError> {
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
    let deadline = tokio::time::Instant::now() + gateway.timeout;
    let start = async {
        if let Some((body, headers)) = native {
            gateway.backend.start_native(body, headers).await
        } else {
            let (body, session) = adapted.expect("one request variant");
            gateway.backend.start(body, session).await
        }
    };
    let mut running = tokio::time::timeout_at(deadline, start)
        .await
        .map_err(|_| timeout_error())??;
    let upstream_headers = forward_response_headers(&running.headers);
    let mut accumulator = crate::output::OutputAccumulator::default();
    if stream {
        let output = async_stream::stream! {
            let _permit = permit;
            let mut terminal = None;
            let mut sequence = 0u64;
            let mut timed_out = false;
            loop {
                match tokio::time::timeout_at(deadline, running.events.recv()).await {
                    Ok(Some(mut event)) => {
                        if aggregate_output && accumulator.observe(&mut event.value) {
                            event.bytes = Bytes::from(format!("event: {}\ndata: {}\n\n",event.value["type"].as_str().unwrap_or("message"),event.value));
                        }
                        if let Some(number) = event.value["sequence_number"].as_u64() { sequence = sequence.max(number + 1); }
                        if event.terminal() { terminal = Some(event); break; }
                        yield Ok::<Bytes, std::io::Error>(event.bytes);
                    }
                    Ok(None) => break,
                    Err(_) => { timed_out = true; break; }
                }
            }
            let parsed = if timed_out { Err("upstream deadline exceeded".into()) } else {
                match tokio::time::timeout_at(deadline, &mut running.finished).await {
                    Ok(Ok(result)) => result,
                    Ok(Err(_)) => Err("upstream parser stopped".into()),
                    Err(_) => Err("upstream deadline exceeded".into()),
                }
            };
            match terminal {
                Some(event) if !event.successful() || parsed.is_ok() => { yield Ok(event.bytes); }
                _ => {
                    let event = json!({"type":"error", "sequence_number":sequence, "code":"upstream_error", "message":parsed.err().unwrap_or_else(|| "upstream closed without a terminal response".into()), "param":null});
                    yield Ok(Bytes::from(format!("event: error\ndata: {event}\n\n")));
                }
            }
        };
        let mut response = Response::new(Body::from_stream(output));
        response.headers_mut().extend(upstream_headers);
        response
            .headers_mut()
            .insert("content-type", "text/event-stream".parse().unwrap());
        response
            .headers_mut()
            .insert("cache-control", "no-cache".parse().unwrap());
        response
            .headers_mut()
            .insert("x-accel-buffering", "no".parse().unwrap());
        Ok(response)
    } else {
        let _permit = permit;
        let terminal = loop {
            let mut event = tokio::time::timeout_at(deadline, running.events.recv())
                .await
                .map_err(|_| timeout_error())?
                .ok_or_else(|| {
                    GatewayError::internal("upstream closed without terminal response")
                })?;
            if aggregate_output {
                accumulator.observe(&mut event.value);
            }
            if event.terminal() {
                break event;
            }
        };
        let parsed = tokio::time::timeout_at(deadline, &mut running.finished)
            .await
            .map_err(|_| timeout_error())?
            .map_err(GatewayError::internal)?;
        if terminal.successful() {
            parsed.map_err(GatewayError::internal)?;
        }
        let response = terminal
            .value
            .get("response")
            .filter(|v| v.is_object())
            .ok_or_else(|| GatewayError::internal("terminal response missing object"))?;
        Ok(Json(response.clone()).into_response())
    }
}
pub(crate) fn timeout_error() -> GatewayError {
    GatewayError {
        upstream_response: None,
        status: StatusCode::GATEWAY_TIMEOUT,
        code: "upstream_timeout",
        message: "upstream deadline exceeded".into(),
    }
}

pub(crate) fn forward_response_headers(headers: &HeaderMap) -> HeaderMap {
    let mut forwarded = HeaderMap::new();
    for (name, value) in headers {
        if name.as_str().starts_with("x-codex-")
            || name.as_str().starts_with("x-ratelimit-")
            || matches!(
                name.as_str(),
                "x-request-id"
                    | "x-models-etag"
                    | "openai-model"
                    | "x-reasoning-included"
                    | "retry-after"
            )
        {
            forwarded.insert(name.clone(), value.clone());
        }
    }
    forwarded
}

/// Public compact facade over the pinned Codex remote-compaction v2 request.
async fn compact(
    State(gateway): State<Arc<Gateway>>,
    headers: HeaderMap,
    body: Result<Json<CreateResponse>, JsonRejection>,
) -> Result<Response, GatewayError> {
    authorize(&gateway, &headers)?;
    let Json(input) = body.map_err(|e| GatewayError::invalid(e.body_text()))?;
    let model = gateway
        .models
        .iter()
        .find(|m| m.slug == input.model)
        .ok_or_else(|| GatewayError::invalid("model is absent from the pinned Codex catalog"))?;
    let session_id = headers
        .get("thread-id")
        .or_else(|| headers.get("session-id"))
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let lite = model.use_responses_lite;
    let mut request = input.into_codex(model, &session_id)?;
    request
        .input
        .push(codex_protocol::models::ResponseItem::CompactionTrigger {});
    let mut forwarded = forward_request_headers(&headers);
    forwarded.extend(codex_api::build_session_headers(
        Some(session_id.clone()),
        Some(session_id),
    ));
    if lite {
        forwarded.insert(
            "x-openai-internal-codex-responses-lite",
            "true".parse().unwrap(),
        );
    }
    let result = respond(
        gateway,
        None,
        Some((
            serde_json::value::to_raw_value(&request).map_err(GatewayError::internal)?,
            forwarded,
        )),
        false,
        true,
    )
    .await?;
    let bytes = axum::body::to_bytes(result.into_body(), 64 * 1024 * 1024)
        .await
        .map_err(GatewayError::internal)?;
    let response: Value = serde_json::from_slice(&bytes).map_err(GatewayError::internal)?;
    if response["status"] != "completed" {
        return Ok(Json(response).into_response());
    }
    let output = response["output"]
        .as_array()
        .ok_or_else(|| GatewayError::internal("missing compaction output"))?;
    if output
        .iter()
        .filter(|v| {
            matches!(
                v["type"].as_str(),
                Some("compaction" | "compaction_summary")
            )
        })
        .count()
        != 1
    {
        return Err(GatewayError::internal(
            "remote compaction must return exactly one compaction item",
        ));
    }
    Ok(Json(json!({"id":response["id"],"object":"response.compaction","created_at":response["created_at"],"output":output,"usage":response["usage"]})).into_response())
}
