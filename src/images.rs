//! Image API adapters; original Codex types and ImagesClient own the upstream protocol.
use crate::{
    error::GatewayError,
    server::Gateway,
    standalone::{self, ToolRequest, extraction_error},
};
use axum::{
    extract::{FromRequest, Multipart, OriginalUri, Request, State},
    http::HeaderMap,
    response::Response,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use codex_api::{
    ImageBackground, ImageEditRequest, ImageGenerationRequest, ImageQuality, ImageUrl,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    prompt: String,
    model: Option<String>,
    background: Option<ImageBackground>,
    quality: Option<ImageQuality>,
    size: Option<String>,
    n: Option<u64>,
    images: Option<Vec<ImageUrl>>,
    response_format: Option<String>,
    output_format: Option<String>,
}
impl Input {
    fn into_codex(self, edit: bool, native: bool) -> Result<ToolRequest, GatewayError> {
        if self.prompt.trim().is_empty() {
            return Err(GatewayError::invalid("prompt must not be empty"));
        }
        let model = match self.model {
            Some(model) if !model.trim().is_empty() => model,
            None if !native => "gpt-image-2".into(),
            _ => return Err(GatewayError::invalid("model must not be empty")),
        };
        if self.n.is_some_and(|n| !(1..=10).contains(&n)) {
            return Err(GatewayError::invalid("n must be between 1 and 10"));
        }
        if self.size.as_ref().is_some_and(|s| s.trim().is_empty()) {
            return Err(GatewayError::invalid("size must not be empty"));
        }
        if self
            .response_format
            .as_deref()
            .is_some_and(|f| f != "b64_json")
        {
            return Err(GatewayError::invalid(
                "this Codex image endpoint returns b64_json; URL responses are unsupported",
            ));
        }
        if self.output_format.as_deref().is_some_and(|f| f != "png") {
            return Err(GatewayError::invalid(
                "this Codex release only exposes default PNG output",
            ));
        }
        if native && (self.response_format.is_some() || self.output_format.is_some()) {
            return Err(GatewayError::invalid(
                "native image requests accept only the pinned Codex fields",
            ));
        }
        // Mirror the image extension's defaults for public SDK requests.
        let background = self
            .background
            .or((!native).then_some(ImageBackground::Auto));
        let quality = self.quality.or((!native).then_some(ImageQuality::Auto));
        let size = self.size.or_else(|| (!native).then(|| "auto".into()));
        if edit {
            let images = self
                .images
                .ok_or_else(|| GatewayError::invalid("images are required for editing"))?;
            if images.is_empty() || images.len() > 5 {
                return Err(GatewayError::invalid("provide between 1 and 5 images"));
            }
            for image in &images {
                if !image.image_url.starts_with("https://")
                    && !image.image_url.starts_with("data:image/")
                {
                    return Err(GatewayError::invalid(
                        "images require HTTPS or image data URLs",
                    ));
                }
            }
            Ok(ToolRequest::Edit(ImageEditRequest {
                images,
                prompt: self.prompt,
                model,
                background,
                quality,
                size,
                n: self.n,
            }))
        } else {
            if self.images.is_some() {
                return Err(GatewayError::invalid(
                    "use images/edits when providing images",
                ));
            }
            Ok(ToolRequest::Generate(ImageGenerationRequest {
                prompt: self.prompt,
                model,
                background,
                quality,
                size,
                n: self.n,
            }))
        }
    }
}

pub async fn generate(
    State(gateway): State<Arc<Gateway>>,
    uri: OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, GatewayError> {
    handle(gateway, uri, headers, request, false).await
}
pub async fn edit(
    State(gateway): State<Arc<Gateway>>,
    uri: OriginalUri,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, GatewayError> {
    handle(gateway, uri, headers, request, true).await
}
async fn handle(
    gateway: Arc<Gateway>,
    uri: OriginalUri,
    headers: HeaderMap,
    request: Request,
    edit: bool,
) -> Result<Response, GatewayError> {
    let native = !uri.path().starts_with("/v1/");
    let mut headers = headers;
    if !native {
        headers.entry("x-codex-image-turn-id").or_insert_with(|| {
            uuid::Uuid::new_v4()
                .to_string()
                .parse()
                .expect("UUID is a valid header")
        });
    }
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    standalone::handle(gateway, headers, async move {
        let input = if content_type.starts_with("multipart/form-data") && edit && !native {
            multipart_input(request).await?
        } else {
            standalone::json::<Input>(request).await?
        };
        input.into_codex(edit, native)
    })
    .await
}
async fn multipart_input(request: Request) -> Result<Input, GatewayError> {
    let mut multipart = Multipart::from_request(request, &())
        .await
        .map_err(|e| extraction_error(e.status(), e.body_text()))?;
    let mut values = Map::new();
    let mut images = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| extraction_error(e.status(), e.body_text()))?
    {
        let name = field.name().unwrap_or("").to_owned();
        if name == "image" || name == "image[]" {
            if images.len() == 5 {
                return Err(GatewayError::invalid("provide at most 5 images"));
            }
            let bytes = field
                .bytes()
                .await
                .map_err(|e| extraction_error(e.status(), e.body_text()))?;
            let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                "image/png"
            } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
                "image/jpeg"
            } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
                "image/webp"
            } else {
                return Err(GatewayError::invalid(
                    "uploaded images must be PNG, JPEG or WebP",
                ));
            };
            images.push(serde_json::json!({"image_url":format!("data:{mime};base64,{}",STANDARD.encode(bytes))}));
        } else {
            if ![
                "prompt",
                "model",
                "background",
                "quality",
                "size",
                "n",
                "response_format",
                "output_format",
            ]
            .contains(&name.as_str())
            {
                return Err(GatewayError::invalid(format!(
                    "unsupported image field: {name}"
                )));
            }
            if values.contains_key(&name) {
                return Err(GatewayError::invalid(format!(
                    "duplicate image field: {name}"
                )));
            }
            let text = field
                .text()
                .await
                .map_err(|e| extraction_error(e.status(), e.body_text()))?;
            let value = if name == "n" {
                Value::from(
                    text.parse::<u64>()
                        .map_err(|_| GatewayError::invalid("n must be an integer"))?,
                )
            } else {
                Value::String(text)
            };
            values.insert(name, value);
        }
    }
    values.insert("images".into(), Value::Array(images));
    serde_json::from_value(Value::Object(values)).map_err(|e| GatewayError::invalid(e.to_string()))
}
