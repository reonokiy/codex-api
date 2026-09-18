use axum::{
    Json,
    response::{IntoResponse, Response},
};
use http::StatusCode;
use serde_json::{Value, json};

#[derive(Debug)]
pub struct GatewayError {
    pub upstream_response: Option<(http::HeaderMap, bytes::Bytes)>,
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}
impl GatewayError {
    pub fn from_api(error: codex_api::ApiError) -> Self {
        if let codex_api::ApiError::Transport(codex_api::TransportError::Http {
            status,
            headers,
            body,
            ..
        }) = error
        {
            Self {
                status,
                code: "upstream_error",
                message: format!("Codex upstream returned HTTP {status}"),
                upstream_response: body.map(|body| (headers.unwrap_or_default(), body.into())),
            }
        } else {
            Self::internal(error)
        }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            upstream_response: None,
            status: StatusCode::BAD_REQUEST,
            code: "invalid_request_error",
            message: message.into(),
        }
    }
    pub fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(error = %error, "gateway internal failure");
        Self {
            upstream_response: None,
            status: StatusCode::BAD_GATEWAY,
            code: "upstream_error",
            message: "upstream request could not be completed".into(),
        }
    }
    pub fn auth() -> Self {
        Self {
            upstream_response: None,
            status: StatusCode::UNAUTHORIZED,
            code: "authentication_error",
            message: "Codex ChatGPT authentication is unavailable; run codex login".into(),
        }
    }
    pub fn payload(&self) -> Value {
        json!({"error":{"type":self.code,"code":self.code,"message":self.message,"param":null}})
    }
}
impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for GatewayError {}
impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        if let Some((headers, body)) = self.upstream_response {
            let mut response = (self.status, body).into_response();
            response
                .headers_mut()
                .extend(crate::server::forward_response_headers(&headers));
            if let Some(content_type) = headers.get("content-type") {
                response
                    .headers_mut()
                    .insert("content-type", content_type.clone());
            }
            response
        } else {
            (self.status, Json(self.payload())).into_response()
        }
    }
}
