use crate::error::GatewayError;
use codex_api::{Reasoning, ResponsesApiRequest, create_text_param_for_request};
use codex_protocol::{
    config_types::{ReasoningSummary, Verbosity},
    models::ResponseItem,
    openai_models::{ModelInfo, ReasoningEffort},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateResponse {
    pub model: String,
    pub input: Value,
    pub instructions: Option<String>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub store: bool,
    #[serde(default)]
    pub tools: Vec<Value>,
    pub tool_choice: Option<String>,
    pub parallel_tool_calls: Option<bool>,
    pub reasoning: Option<ReasoningInput>,
    pub text: Option<TextInput>,
    pub service_tier: Option<String>,
    pub prompt_cache_key: Option<String>,
    pub include: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReasoningInput {
    pub effort: Option<ReasoningEffort>,
    #[serde(alias = "generate_summary")]
    pub summary: Option<ReasoningSummary>,
    pub context: Option<ReasoningContextInput>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningContextInput {
    Auto,
    CurrentTurn,
    AllTurns,
}

impl From<ReasoningContextInput> for codex_api::ReasoningContext {
    fn from(value: ReasoningContextInput) -> Self {
        match value {
            ReasoningContextInput::Auto => Self::Auto,
            ReasoningContextInput::CurrentTurn => Self::CurrentTurn,
            ReasoningContextInput::AllTurns => Self::AllTurns,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextInput {
    pub verbosity: Option<Verbosity>,
    pub format: Option<TextFormatInput>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum TextFormatInput {
    Text {},
    JsonSchema(JsonFormat),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonFormat {
    pub name: String,
    pub schema: Value,
    pub strict: Option<bool>,
}

impl CreateResponse {
    /// Lite requires all-turn reasoning. Other explicit modes use regular Responses.
    pub fn uses_responses_lite(&self, model: &ModelInfo) -> bool {
        model.use_responses_lite
            && !matches!(
                self.reasoning
                    .as_ref()
                    .and_then(|reasoning| reasoning.context),
                Some(ReasoningContextInput::Auto | ReasoningContextInput::CurrentTurn)
            )
    }

    pub fn into_codex(
        self,
        model: &ModelInfo,
        session_id: &str,
    ) -> Result<ResponsesApiRequest, GatewayError> {
        let lite = self.uses_responses_lite(model);
        if self.store {
            return Err(GatewayError::invalid(
                "store=true is not supported; send full input history",
            ));
        }
        if self.tool_choice.as_deref().is_some_and(|v| v != "auto") {
            return Err(GatewayError::invalid(
                "Codex uses tool_choice=auto; other choices are not supported",
            ));
        }
        if self.include.as_ref().is_some_and(|values| {
            values.iter().any(|v| {
                ![
                    "reasoning.encrypted_content",
                    "web_search_call.action.sources",
                    "web_search_call.results",
                ]
                .contains(&v.as_str())
            })
        }) {
            return Err(GatewayError::invalid(
                "include supports reasoning.encrypted_content, web_search_call.action.sources and web_search_call.results",
            ));
        }
        let tool_specs = self
            .tools
            .iter()
            .map(crate::tools::parse)
            .collect::<Result<Vec<_>, _>>()?;
        let mut input = normalize_input(self.input)?;
        let reasoning_input = self.reasoning;
        let effort = reasoning_input
            .as_ref()
            .and_then(|v| v.effort.clone())
            .or_else(|| model.default_reasoning_level.clone());
        if let Some(effort) = &effort
            && !model
                .supported_reasoning_levels
                .iter()
                .any(|v| &v.effort == effort)
        {
            return Err(GatewayError::invalid(
                "reasoning effort is not supported by this Codex model",
            ));
        }
        let context = reasoning_input.as_ref().and_then(|v| v.context);
        let summary = reasoning_input
            .and_then(|v| v.summary)
            .unwrap_or(model.default_reasoning_summary);
        let reasoning = Reasoning {
            effort: effort.map(|v| model.resolve_reasoning_effort(v)),
            summary: (model.supports_reasoning_summary_parameter
                && summary != ReasoningSummary::None)
                .then_some(summary),
            context: context
                .map(Into::into)
                .or_else(|| lite.then_some(codex_api::ReasoningContext::AllTurns)),
        };
        let text_input = self.text;
        let requested_verbosity = text_input.as_ref().and_then(|v| v.verbosity);
        if requested_verbosity.is_some() && !model.support_verbosity {
            return Err(GatewayError::invalid(
                "text.verbosity is not supported by this model",
            ));
        }
        let verbosity = model
            .support_verbosity
            .then(|| requested_verbosity.or(model.default_verbosity))
            .flatten();
        let format = text_input
            .and_then(|v| v.format)
            .and_then(|format| match format {
                TextFormatInput::Text {} => None,
                TextFormatInput::JsonSchema(format) => Some(format),
            });
        if let Some(format) = &format
            && (format.name.is_empty() || !format.schema.is_object())
        {
            return Err(GatewayError::invalid(
                "text.format requires type=json_schema, name, and an object schema",
            ));
        }
        let schema = format.as_ref().map(|v| v.schema.clone());
        let mut text = create_text_param_for_request(
            verbosity,
            &schema,
            format.as_ref().and_then(|v| v.strict).unwrap_or(true),
        );
        if let (Some(text), Some(format)) = (&mut text, &format)
            && let Some(output_format) = &mut text.format
        {
            output_format.name = format.name.clone();
        }
        if self
            .service_tier
            .as_ref()
            .is_some_and(|v| v != "auto" && !model.supports_service_tier(v))
        {
            return Err(GatewayError::invalid(
                "service_tier is not supported by this Codex model",
            ));
        }
        let mut instructions = self.instructions.unwrap_or_else(|| {
            model
                .model_messages
                .as_ref()
                .and_then(|v| v.instructions_template.clone())
                .unwrap_or_else(|| codex_models_manager::model_info::BASE_INSTRUCTIONS.to_owned())
        });
        let tools = if lite {
            let tools = codex_tools::create_tools_json_for_responses_lite(&tool_specs)
                .map_err(GatewayError::internal)?;
            let namespace = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, session_id.as_bytes());
            let mut prefix = vec![ResponseItem::AdditionalTools {
                id: Some(codex_protocol::ResponseItemId::with_suffix(
                    "at",
                    uuid::Uuid::new_v5(
                        &namespace,
                        &serde_json::to_vec(&tools).map_err(GatewayError::internal)?,
                    ),
                )),
                role: "developer".into(),
                tools,
            }];
            if !instructions.is_empty() {
                use codex_context_fragments::ContextualUserFragment;
                let mut item = ContextualUserFragment::into(
                    crate::lite_instructions::BaseInstructionsFragment(instructions.clone()),
                );
                item.set_id(Some(codex_protocol::ResponseItemId::with_suffix(
                    "msg",
                    uuid::Uuid::new_v5(&namespace, instructions.as_bytes()),
                )));
                prefix.push(item);
            }
            input.splice(0..0, prefix);
            instructions.clear();
            None
        } else {
            Some(
                codex_tools::create_tools_raw_json_for_responses_api(&tool_specs)
                    .map_err(GatewayError::internal)?
                    .into(),
            )
        };
        let mut include = self.include.unwrap_or_default();
        if !include.iter().any(|v| v == "reasoning.encrypted_content") {
            include.push("reasoning.encrypted_content".into());
        }
        Ok(ResponsesApiRequest {
            model: model.slug.clone(),
            instructions,
            input,
            tools,
            tool_choice: "auto".into(),
            parallel_tool_calls: self.parallel_tool_calls.unwrap_or(true) && !lite,
            reasoning: Some(reasoning),
            store: false,
            stream: true,
            stream_options: None,
            include,
            service_tier: model.service_tier_for_request(self.service_tier),
            prompt_cache_key: Some(
                self.prompt_cache_key
                    .unwrap_or_else(|| session_id.to_owned()),
            ),
            text,
            client_metadata: Some(HashMap::new()),
            access_programs: None,
        })
    }
}

fn normalize_input(value: Value) -> Result<Vec<ResponseItem>, GatewayError> {
    let items = match value {
        Value::String(text) => vec![
            json!({"type":"message", "role":"user", "content":[{"type":"input_text","text":text}]}),
        ],
        Value::Array(items) if !items.is_empty() => items,
        _ => {
            return Err(GatewayError::invalid(
                "input must be a string or a non-empty array",
            ));
        }
    };
    items
        .into_iter()
        .map(|mut item| {
            let object = item
                .as_object_mut()
                .ok_or_else(|| GatewayError::invalid("input items must be objects"))?;
            if !object.contains_key("type") && object.contains_key("role") {
                object.insert("type".into(), json!("message"));
            }
            if object.get("type").and_then(Value::as_str) == Some("message") {
                let role = object.get("role").and_then(Value::as_str).unwrap_or("");
                if !["user", "assistant", "system", "developer"].contains(&role) {
                    return Err(GatewayError::invalid("unsupported message role"));
                }
                let content_type = if role == "assistant" {
                    "output_text"
                } else {
                    "input_text"
                };
                if let Some(Value::String(text)) = object.get("content") {
                    object.insert("content".into(), json!([{"type":content_type,"text":text}]));
                }
            }
            let parsed: ResponseItem = serde_json::from_value(item)
                .map_err(|e| GatewayError::invalid(format!("invalid Codex input item: {e}")))?;
            match parsed {
                ResponseItem::Message { .. }
                | ResponseItem::Reasoning { .. }
                | ResponseItem::FunctionCall { .. }
                | ResponseItem::FunctionCallOutput { .. }
                | ResponseItem::Compaction { .. }
                | ResponseItem::CompactionTrigger { .. }
                | ResponseItem::ContextCompaction { .. }
                | ResponseItem::CustomToolCall { .. }
                | ResponseItem::CustomToolCallOutput { .. }
                | ResponseItem::WebSearchCall { .. } => Ok(parsed),
                _ => Err(GatewayError::invalid("unsupported input item type")),
            }
        })
        .collect()
}
