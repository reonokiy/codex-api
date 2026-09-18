//! Validate the public tool schema, then let Codex serialize both wire formats.
use crate::error::GatewayError;
use codex_protocol::config_types::{WebSearchContextSize, WebSearchFilters, WebSearchUserLocation};
use codex_tools::{ResponsesApiNamespace, ResponsesApiNamespaceTool, ResponsesApiTool, ToolSpec};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Input {
    Function {
        name: String,
        #[serde(default)]
        description: String,
        parameters: Value,
        #[serde(default)]
        strict: bool,
    },
    WebSearch {
        external_web_access: Option<bool>,
        indexed_web_access: Option<bool>,
        filters: Option<WebSearchFilters>,
        user_location: Option<WebSearchUserLocation>,
        search_context_size: Option<WebSearchContextSize>,
        search_content_types: Option<Vec<String>>,
    },
    Custom {
        name: String,
        #[serde(default)]
        description: String,
        format: codex_tools::FreeformToolFormat,
    },
    Namespace {
        name: String,
        #[serde(default)]
        description: String,
        tools: Vec<Value>,
    },
}

pub fn parse(value: &Value) -> Result<ToolSpec, GatewayError> {
    parse_inner(value, false)
}

fn parse_inner(value: &Value, nested: bool) -> Result<ToolSpec, GatewayError> {
    // These upstream deserializers accept unknown fields. Reject them here so
    // public API options cannot silently disappear during native serialization.
    for (field, allowed) in [
        ("filters", &["allowed_domains"][..]),
        (
            "user_location",
            &["type", "country", "region", "city", "timezone"][..],
        ),
        ("format", &["type", "syntax", "definition"][..]),
    ] {
        if value
            .get(field)
            .and_then(Value::as_object)
            .is_some_and(|o| o.keys().any(|key| !allowed.contains(&key.as_str())))
        {
            return Err(GatewayError::invalid(format!(
                "unsupported tool.{field} field"
            )));
        }
    }
    let input: Input = serde_json::from_value(value.clone())
        .map_err(|e| GatewayError::invalid(format!("unsupported or invalid tool: {e}")))?;
    match input {
        Input::Function {
            name,
            description,
            parameters,
            strict,
        } => {
            validate_name(&name)?;
            if !parameters.is_object() {
                return Err(GatewayError::invalid("tool.parameters must be an object"));
            }
            Ok(ToolSpec::Function(ResponsesApiTool {
                name,
                description,
                strict,
                defer_loading: None,
                output_schema: None,
                parameters: codex_tools::parse_tool_input_schema(&parameters)
                    .map_err(|e| GatewayError::invalid(format!("invalid tool schema: {e}")))?,
            }))
        }
        Input::WebSearch {
            external_web_access,
            indexed_web_access,
            filters,
            user_location,
            search_context_size,
            search_content_types,
        } => {
            if nested {
                return Err(GatewayError::invalid(
                    "namespaces accept only function and custom tools",
                ));
            }
            if search_content_types.as_ref().is_some_and(|types| {
                types.is_empty() || types.iter().any(|t| t != "text" && t != "image")
            }) {
                return Err(GatewayError::invalid(
                    "search_content_types accepts text and image",
                ));
            }
            Ok(ToolSpec::WebSearch {
                external_web_access,
                indexed_web_access,
                filters: filters.map(Into::into),
                user_location: user_location.map(Into::into),
                search_context_size,
                search_content_types,
            })
        }
        Input::Custom {
            name,
            description,
            format,
        } => {
            validate_name(&name)?;
            if format.r#type != "grammar"
                || !["lark", "regex"].contains(&format.syntax.as_str())
                || format.definition.is_empty()
            {
                return Err(GatewayError::invalid(
                    "custom tools require a non-empty lark or regex grammar",
                ));
            }
            Ok(ToolSpec::Freeform(codex_tools::FreeformTool {
                name,
                description,
                defer_loading: None,
                format,
            }))
        }
        Input::Namespace {
            name,
            description,
            tools,
        } => {
            validate_name(&name)?;
            if nested || tools.is_empty() {
                return Err(GatewayError::invalid(
                    "namespaces must contain function or custom tools and cannot be nested",
                ));
            }
            let tools = tools
                .iter()
                .map(|tool| match parse_inner(tool, true)? {
                    ToolSpec::Function(tool) => Ok(ResponsesApiNamespaceTool::Function(tool)),
                    ToolSpec::Freeform(tool) => Ok(ResponsesApiNamespaceTool::Custom(tool)),
                    _ => Err(GatewayError::invalid("unsupported namespace tool")),
                })
                .collect::<Result<_, _>>()?;
            Ok(ToolSpec::Namespace(ResponsesApiNamespace {
                name,
                description,
                tools,
            }))
        }
    }
}

fn validate_name(name: &str) -> Result<(), GatewayError> {
    if name.trim().is_empty() {
        return Err(GatewayError::invalid("tool name must not be empty"));
    }
    Ok(())
}
