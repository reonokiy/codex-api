//! Shared authentication, limits and native dispatch for standalone Codex tools.
use crate::{
    error::GatewayError,
    server::{
        Gateway, authorize, forward_request_headers, forward_response_headers, timeout_error,
    },
};
use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use std::{future::Future, sync::Arc};

pub enum ToolRequest {
    Generate(codex_api::ImageGenerationRequest),
    Edit(codex_api::ImageEditRequest),
    Search(Box<codex_api::SearchRequest>),
    Memory(serde_json::Value),
}
impl ToolRequest {
    pub fn path(&self) -> &'static str {
        match self {
            Self::Generate(_) => "images/generations",
            Self::Edit(_) => "images/edits",
            Self::Search(_) => "alpha/search",
            Self::Memory(_) => "memories/trace_summarize",
        }
    }
}

pub async fn handle(
    gateway: Arc<Gateway>,
    headers: HeaderMap,
    input: impl Future<Output = Result<ToolRequest, GatewayError>>,
) -> Result<Response, GatewayError> {
    authorize(&gateway, &headers)?;
    let _permit = gateway
        .concurrency
        .clone()
        .try_acquire_owned()
        .map_err(|_| GatewayError {
            upstream_response: None,
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "gateway_busy",
            message: "gateway concurrency limit reached".into(),
        })?;
    let work = async {
        let request = input.await?;
        let (headers, body) = gateway
            .backend
            .standalone(&request, forward_request_headers(&headers))
            .await?;
        let mut response = body.into_response();
        response.headers_mut().insert(
            "content-type",
            http::HeaderValue::from_static("application/json"),
        );
        response
            .headers_mut()
            .extend(forward_response_headers(&headers));
        Ok(response)
    };
    tokio::time::timeout(gateway.timeout, work)
        .await
        .map_err(|_| timeout_error())?
}

pub async fn json<T: serde::de::DeserializeOwned>(request: Request) -> Result<T, GatewayError> {
    let is_json = request
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim() == "application/json");
    if !is_json {
        return Err(extraction_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "use application/json",
        ));
    }
    let bytes = Bytes::from_request(request, &())
        .await
        .map_err(|e| extraction_error(e.status(), e.body_text()))?;
    let mut decoder = serde_json::Deserializer::from_slice(&bytes);
    let mut ignored = Vec::new();
    let value = serde_ignored::deserialize(&mut decoder, |path| ignored.push(path.to_string()))
        .map_err(|e| GatewayError::invalid(e.to_string()))?;
    decoder
        .end()
        .map_err(|e| GatewayError::invalid(e.to_string()))?;
    if !ignored.is_empty() {
        return Err(GatewayError::invalid(format!(
            "unsupported fields: {}",
            ignored.join(", ")
        )));
    }
    Ok(value)
}

pub fn extraction_error(status: StatusCode, message: impl Into<String>) -> GatewayError {
    GatewayError {
        upstream_response: None,
        status,
        code: "invalid_request_error",
        message: message.into(),
    }
}
