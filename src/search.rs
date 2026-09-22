//! Adapter for Codex's standalone web.run endpoint (not an OpenAI public API).
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
use codex_api::{Reasoning, SearchCommands, SearchInput, SearchRequest, SearchSettings};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    id: String,
    model: String,
    reasoning: Option<SearchReasoning>,
    input: Option<Value>,
    commands: Option<SearchCommands>,
    settings: Option<SearchSettings>,
    max_output_tokens: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchReasoning {
    effort: Option<codex_protocol::openai_models::ReasoningEffort>,
    summary: Option<codex_protocol::config_types::ReasoningSummary>,
    context: Option<crate::request::ReasoningContextInput>,
}
impl SearchReasoning {
    fn into_codex(self) -> Reasoning {
        Reasoning {
            effort: self.effort,
            summary: self.summary,
            context: self.context.map(Into::into),
        }
    }
}

pub async fn search(
    State(gateway): State<Arc<Gateway>>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, GatewayError> {
    standalone::handle(gateway, headers, async move {
        let input: Input = standalone::json(request).await?;
        if input.id.trim().is_empty() || input.model.trim().is_empty() {
            return Err(GatewayError::invalid(
                "search requires non-empty id and model",
            ));
        }
        if input.input.is_none() && input.commands.is_none() {
            return Err(GatewayError::invalid("search requires input or commands"));
        }
        let content = input
            .input
            .map(|value| match value {
                Value::String(text) => Ok(SearchInput::Text(text)),
                Value::Array(items) => serde_json::from_value(Value::Array(items))
                    .map(SearchInput::Items)
                    .map_err(|e| GatewayError::invalid(format!("invalid search input: {e}"))),
                _ => Err(GatewayError::invalid(
                    "search input must be text or Codex input items",
                )),
            })
            .transpose()?;
        Ok(ToolRequest::Search(Box::new(SearchRequest {
            id: input.id,
            model: input.model,
            reasoning: input.reasoning.map(SearchReasoning::into_codex),
            input: content,
            commands: input.commands,
            settings: input.settings,
            max_output_tokens: input.max_output_tokens,
        })))
    })
    .await
}
