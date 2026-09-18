//! Native memory summarization; the original MemoriesClient owns the schema.
use crate::{
    error::GatewayError,
    server::Gateway,
    standalone::{self, ToolRequest},
};
use axum::{
    extract::{Request, State},
    http::HeaderMap,
    response::Response,
};
use std::sync::Arc;

pub async fn summarize(
    State(gateway): State<Arc<Gateway>>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, GatewayError> {
    standalone::handle(gateway, headers, async move {
        let body: serde_json::Value = standalone::json(request).await?;
        if !body.is_object() {
            return Err(GatewayError::invalid(
                "memory summarization requires a JSON object",
            ));
        }
        Ok(ToolRequest::Memory(body))
    })
    .await
}
