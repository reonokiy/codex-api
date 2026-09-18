//! Public multipart uploads delegated to Codex's complete original file-upload pipeline.
use crate::{
    error::GatewayError,
    native::{self, AuthPolicy, Origin},
    server::{Gateway, timeout_error},
    standalone::extraction_error,
};
use axum::{
    Json,
    extract::{FromRequest, Multipart, Request, State},
};
use codex_api::OpenAiFileError;
use codex_http_client::{ClientRouteClass, RouteAwareClientPool};
use http::{HeaderMap, StatusCode};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

pub async fn create(
    State(gateway): State<Arc<Gateway>>,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<Value>, GatewayError> {
    native::authorize(&gateway, &headers)?;
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
        let mut form = Multipart::from_request(request, &())
            .await
            .map_err(|e| extraction_error(e.status(), e.body_text()))?;
        let mut file = None;
        let mut purpose = None;
        while let Some(field) = form
            .next_field()
            .await
            .map_err(|e| extraction_error(e.status(), e.body_text()))?
        {
            match field.name() {
                Some("file") if file.is_none() => {
                    let filename = field
                        .file_name()
                        .filter(|name| !name.trim().is_empty())
                        .ok_or_else(|| GatewayError::invalid("file requires a filename"))?
                        .to_owned();
                    let bytes = field
                        .bytes()
                        .await
                        .map_err(|e| extraction_error(e.status(), e.body_text()))?;
                    file = Some((filename, bytes));
                }
                Some("purpose") if purpose.is_none() => {
                    purpose = Some(
                        field
                            .text()
                            .await
                            .map_err(|e| extraction_error(e.status(), e.body_text()))?,
                    );
                }
                _ => {
                    return Err(GatewayError::invalid(
                        "expected one file and one purpose; duplicate or unsupported multipart field",
                    ));
                }
            }
        }
        if purpose.as_deref() != Some("user_data") {
            return Err(GatewayError::invalid(
                "Codex file uploads support purpose=user_data",
            ));
        }
        let (filename, bytes) = file.ok_or_else(|| GatewayError::invalid("file is required"))?;
        let size = bytes.len() as u64;
        let changes = gateway.backend.auth.auth_change_receiver();
        let revision = *changes.borrow();
        let (provider, auth) = gateway
            .backend
            .proxy_config(Origin::ChatGpt, AuthPolicy::Subscription)
            .await?;
        if *changes.borrow() != revision {
            return Err(GatewayError::auth());
        }
        // This is the pool Codex constructs in core/src/session/session.rs for uploads.
        // The captured auth belongs to this entire pipeline; never replay a partially uploaded file.
        let pool = RouteAwareClientPool::new_without_request_logging(
            gateway.backend.factory.clone(),
            ClientRouteClass::Api,
        )
        .with_legacy_custom_ca_fallback();
        let uploaded = codex_api::upload_openai_file(
            &provider.base_url,
            auth.as_ref(),
            &pool,
            filename,
            size,
            futures::stream::once(async move { Ok(bytes) }),
            None,
        )
        .await
        .map_err(file_error)?;
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(GatewayError::internal)?
            .as_secs();
        Ok(Json(json!({
            "id": uploaded.file_id,
            "object": "file",
            "bytes": uploaded.file_size_bytes,
            "created_at": created_at,
            "filename": uploaded.file_name,
            "purpose": "user_data",
            "status": "processed"
        })))
    };
    tokio::time::timeout(gateway.timeout, work)
        .await
        .map_err(|_| timeout_error())?
}

fn file_error(error: OpenAiFileError) -> GatewayError {
    match error {
        OpenAiFileError::UnexpectedStatus { status, body, .. } => {
            let content_type = if serde_json::from_str::<Value>(&body).is_ok() {
                "application/json"
            } else {
                "text/plain; charset=utf-8"
            };
            GatewayError {
                upstream_response: Some((
                    HeaderMap::from_iter([(
                        http::header::CONTENT_TYPE,
                        content_type.parse().expect("static content type"),
                    )]),
                    body.into(),
                )),
                status,
                code: "upstream_error",
                message: format!("Codex file upload returned HTTP {status}"),
            }
        }
        OpenAiFileError::FileTooLarge { .. } => extraction_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "file exceeds the Codex upload limit",
        ),
        OpenAiFileError::BlobUploadStatus { status, .. } => {
            extraction_error(status, "upstream storage rejected the file")
        }
        OpenAiFileError::UploadNotReady { .. } => timeout_error(),
        OpenAiFileError::Request { source, .. }
        | OpenAiFileError::BlobUploadRequest { source, .. } => {
            if source.is_timeout() {
                timeout_error()
            } else {
                GatewayError::internal("Codex file transport failed")
            }
        }
        OpenAiFileError::Decode { .. } => GatewayError::internal("Codex file response was invalid"),
        OpenAiFileError::UploadFailed { message, .. } => GatewayError {
            upstream_response: None,
            status: StatusCode::BAD_GATEWAY,
            code: "upstream_error",
            message,
        },
    }
}
