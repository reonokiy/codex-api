//! A connection-preserving relay. The caller's Codex parser consumes original messages.
use crate::{
    error::GatewayError,
    server::{
        Gateway, authorize, forward_request_headers, forward_response_headers, timeout_error,
    },
};
use axum::{
    extract::{
        OriginalUri, State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::Response,
};
use futures::{SinkExt, StreamExt};
use std::{sync::Arc, time::Duration};
use tokio_tungstenite::tungstenite::Message as Upstream;

pub async fn upgrade(
    State(gateway): State<Arc<Gateway>>,
    headers: HeaderMap,
    uri: OriginalUri,
    ws: WebSocketUpgrade,
) -> Result<Response, GatewayError> {
    authorize(&gateway, &headers)?;
    let headers = crate::headers::request_headers(&headers);
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
    // Resolve the upstream handshake before accepting the client so HTTP fallback still works.
    let (upstream, response_headers) = tokio::time::timeout(
        gateway.timeout,
        gateway
            .backend
            .connect_websocket(forward_request_headers(&headers)),
    )
    .await
    .map_err(|_| timeout_error())??;
    let idle = gateway.timeout;
    let public = uri.path().starts_with("/v1/");
    let session = headers
        .get("thread-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut response = ws
        .max_message_size(16 * 1024 * 1024)
        .max_frame_size(16 * 1024 * 1024)
        .on_upgrade(move |client| async move {
            let _permit = permit;
            relay(client, upstream, idle, public, &gateway.models, &session).await;
        });
    response
        .headers_mut()
        .extend(forward_response_headers(&response_headers));
    Ok(response)
}

pub(crate) async fn relay<S, E>(
    mut client: WebSocket,
    mut upstream: S,
    idle: Duration,
    public: bool,
    models: &[codex_protocol::openai_models::ModelInfo],
    session: &str,
) where
    S: futures::Stream<Item = Result<Upstream, E>> + futures::Sink<Upstream> + Unpin,
{
    let mut accumulator = crate::output::OutputAccumulator::default();
    loop {
        let step = async {
            tokio::select! {
                incoming = client.next() => {
                    let Some(Ok(message)) = incoming else { return false; };
                    let close = matches!(message, Message::Close(_));
                    let message = match message {
                        Message::Text(v) => {
                            let text=if public {
                                match adapt_public_request(v.as_str(),models,session) {
                                    Ok(text)=>text,
                                    Err(error)=> {
                                        let event=serde_json::json!({"type":"error","error":error.payload()["error"]});
                                        return client.send(Message::Text(event.to_string().into())).await.is_ok();
                                    }
                                }
                            } else { v.as_str().to_owned() };
                            Upstream::Text(text.into())
                        },
                        Message::Binary(v) => Upstream::Binary(v),
                        // Each WebSocket library handles connection-local ping/pong itself.
                        Message::Ping(_) | Message::Pong(_) => return true,
                        Message::Close(v) => Upstream::Close(v.map(|f| tokio_tungstenite::tungstenite::protocol::CloseFrame { code: f.code.into(), reason: f.reason.as_str().to_owned().into() })),
                    };
                    upstream.send(message).await.is_ok() && !close
                }
                incoming = upstream.next() => {
                    let Some(Ok(message)) = incoming else { return false; };
                    let close = matches!(message, Upstream::Close(_));
                    let message = match message {
                        Upstream::Text(v) => {
                            let mut text = v.as_str().to_owned();
                            if public && let Ok(mut event) = serde_json::from_str::<serde_json::Value>(&text) && accumulator.observe(&mut event) { text = event.to_string(); }
                            Message::Text(text.into())
                        },
                        Upstream::Binary(v) => Message::Binary(v),
                        Upstream::Ping(v) => { return upstream.send(Upstream::Pong(v)).await.is_ok(); }
                        Upstream::Pong(_) | Upstream::Frame(_) => return true,
                        Upstream::Close(v) => Message::Close(v.map(|f| CloseFrame { code: f.code.into(), reason: f.reason.as_str().to_owned().into() })),
                    };
                    client.send(message).await.is_ok() && !close
                }
            }
        };
        match tokio::time::timeout(idle, step).await {
            Ok(true) => {}
            Ok(false) => break,
            Err(_) => {
                let _ = tokio::time::timeout(
                    Duration::from_secs(1),
                    client.send(Message::Close(Some(CloseFrame {
                        code: 1011,
                        reason: "gateway idle timeout".into(),
                    }))),
                )
                .await;
                break;
            }
        }
    }
}

fn adapt_public_request(
    text: &str,
    models: &[codex_protocol::openai_models::ModelInfo],
    session: &str,
) -> Result<String, GatewayError> {
    use codex_api::{ResponseCreateWsRequest, ResponsesWsRequest};
    use serde_json::{Value, json};
    let mut value: Value =
        serde_json::from_str(text).map_err(|e| GatewayError::invalid(e.to_string()))?;
    // A fully prepared Codex frame uses stream:true. Preserve it byte-for-byte.
    // Other event types (cancel/steer/etc.) belong to the upstream protocol.
    if value["type"] != "response.create" || value["stream"] == true {
        return Ok(text.to_owned());
    }
    let object = value
        .as_object_mut()
        .ok_or_else(|| GatewayError::invalid("expected response.create object"))?;
    object.remove("type");
    let previous = object.remove("previous_response_id");
    let generate = object.remove("generate");
    let lane = object.remove("stream_id");
    let metadata = object.remove("client_metadata");
    object.insert("stream".into(), json!(true));
    let input: crate::request::CreateResponse =
        serde_json::from_value(value).map_err(|e| GatewayError::invalid(e.to_string()))?;
    let model = models
        .iter()
        .find(|m| m.slug == input.model)
        .ok_or_else(|| GatewayError::invalid("model is absent from the pinned Codex catalog"))?;
    let request = input.into_codex(model, session)?;
    let mut wire = ResponseCreateWsRequest::from(&request);
    if let Some(previous) = previous {
        wire.previous_response_id = Some(
            previous
                .as_str()
                .ok_or_else(|| GatewayError::invalid("previous_response_id must be a string"))?
                .to_owned(),
        );
    }
    if let Some(generate) = generate {
        wire.generate = Some(
            generate
                .as_bool()
                .ok_or_else(|| GatewayError::invalid("generate must be a boolean"))?,
        );
    }
    if let Some(metadata) = metadata {
        wire.client_metadata = Some(
            serde_json::from_value(metadata)
                .map_err(|e| GatewayError::invalid(format!("invalid client_metadata: {e}")))?,
        );
    }
    if model.use_responses_lite {
        wire.client_metadata.get_or_insert_default().insert(
            "ws_request_header_x_openai_internal_codex_responses_lite".into(),
            "true".into(),
        );
    }
    let mut value = serde_json::to_value(ResponsesWsRequest::ResponseCreate(wire))
        .map_err(GatewayError::internal)?;
    if let Some(lane) = lane {
        value["stream_id"] = lane;
    }
    Ok(value.to_string())
}
