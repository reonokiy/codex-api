//! Usage-only OpenTelemetry. Never collect prompts, tool arguments, or credentials.
use http::HeaderMap;
use opentelemetry::{
    Context, KeyValue,
    logs::{AnyValue, LogRecord, Logger, LoggerProvider, Severity},
    metrics::{Counter, Histogram, MeterProvider},
    propagation::{Extractor, TextMapPropagator},
    trace::{Span, SpanKind, Status, Tracer, TracerProvider},
};
use opentelemetry_sdk::{
    Resource, logs::SdkLoggerProvider, metrics::SdkMeterProvider,
    propagation::TraceContextPropagator, trace::SdkTracerProvider,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub struct Telemetry {
    traces: SdkTracerProvider,
    logs: SdkLoggerProvider,
    metrics: SdkMeterProvider,
    input: Counter<u64>,
    output: Counter<u64>,
    cached: Counter<u64>,
    reasoning: Counter<u64>,
    duration: Histogram<f64>,
    missing: Counter<u64>,
    trust_identity_headers: bool,
}

impl Telemetry {
    /// Enable only when an OTLP endpoint is explicitly configured. Exporters use
    /// standard OTEL_EXPORTER_OTLP_* endpoint, header, and timeout variables.
    pub fn from_env() -> anyhow::Result<Option<Arc<Self>>> {
        if std::env::var("OTEL_SDK_DISABLED").is_ok_and(|v| v.eq_ignore_ascii_case("true"))
            || ![
                "OTEL_EXPORTER_OTLP_ENDPOINT",
                "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
                "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
                "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
            ]
            .iter()
            .any(|name| std::env::var(name).is_ok_and(|v| !v.is_empty()))
        {
            return Ok(None);
        }
        for name in [
            "OTEL_EXPORTER_OTLP_PROTOCOL",
            "OTEL_EXPORTER_OTLP_TRACES_PROTOCOL",
            "OTEL_EXPORTER_OTLP_METRICS_PROTOCOL",
            "OTEL_EXPORTER_OTLP_LOGS_PROTOCOL",
        ] {
            if std::env::var(name).is_ok_and(|v| v != "http/protobuf") {
                anyhow::bail!("{name} must be http/protobuf");
            }
        }
        let resource = Resource::builder()
            .with_service_name(
                std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "codex-api".into()),
            )
            .with_attributes([KeyValue::new("service.version", crate::CODEX_RELEASE)])
            .build();
        let traces = SdkTracerProvider::builder()
            .with_resource(resource.clone())
            .with_batch_exporter(
                opentelemetry_otlp::SpanExporter::builder()
                    .with_http()
                    .build()?,
            )
            .build();
        let logs = SdkLoggerProvider::builder()
            .with_resource(resource.clone())
            .with_batch_exporter(
                opentelemetry_otlp::LogExporter::builder()
                    .with_http()
                    .build()?,
            )
            .build();
        let metrics = SdkMeterProvider::builder()
            .with_resource(resource)
            .with_periodic_exporter(
                opentelemetry_otlp::MetricExporter::builder()
                    .with_http()
                    .build()?,
            )
            .build();
        let trust = std::env::var("CODEX_GATEWAY_TRUST_IDENTITY_HEADERS")
            .is_ok_and(|v| v.eq_ignore_ascii_case("true"));
        Ok(Some(Arc::new(Self::new(traces, logs, metrics, trust))))
    }

    /// Providers are explicit so applications and tests do not share global state.
    pub fn new(
        traces: SdkTracerProvider,
        logs: SdkLoggerProvider,
        metrics: SdkMeterProvider,
        trust_identity_headers: bool,
    ) -> Self {
        let meter = metrics.meter("codex-api.usage");
        Self {
            input: meter
                .u64_counter("gen_ai.client.inference.usage.input_tokens")
                .with_unit("{token}")
                .build(),
            output: meter
                .u64_counter("gen_ai.client.inference.usage.output_tokens")
                .with_unit("{token}")
                .build(),
            cached: meter
                .u64_counter("gen_ai.client.inference.usage.cache_read.input_tokens")
                .with_unit("{token}")
                .build(),
            reasoning: meter
                .u64_counter("gen_ai.client.inference.usage.reasoning.output_tokens")
                .with_unit("{token}")
                .build(),
            duration: meter
                .f64_histogram("gen_ai.client.inference.duration")
                .with_unit("s")
                .with_boundaries(vec![
                    0.01, 0.02, 0.04, 0.08, 0.16, 0.32, 0.64, 1.28, 2.56, 5.12, 10.24, 20.48,
                    40.96, 81.92,
                ])
                .build(),
            missing: meter
                .u64_counter("codex_api.usage.missing")
                .with_unit("{request}")
                .build(),
            traces,
            logs,
            metrics,
            trust_identity_headers,
        }
    }

    pub fn shutdown(&self) {
        // Workers have bounded queues; an unavailable collector must not stop inference.
        let _ = self.logs.shutdown();
        let _ = self.metrics.shutdown();
        let _ = self.traces.shutdown();
    }
}

struct HeaderExtractor<'a>(&'a HeaderMap);
impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.to_str().ok()
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

fn label(value: Option<&str>) -> Option<String> {
    value
        .filter(|v| {
            !v.is_empty()
                && v.len() <= 256
                && v.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.:/".contains(&c))
        })
        .map(str::to_owned)
}

#[derive(Clone)]
pub struct RequestTelemetry {
    telemetry: Option<Arc<Telemetry>>,
    user: String,
    key: Option<String>,
    parent: Context,
}
impl RequestTelemetry {
    pub fn new(telemetry: Option<Arc<Telemetry>>, headers: &HeaderMap) -> Self {
        let trust = telemetry.as_ref().is_some_and(|t| t.trust_identity_headers);
        let get = |name| headers.get(name).and_then(|v| v.to_str().ok());
        Self {
            telemetry,
            user: if trust {
                label(get("x-user-id")).unwrap_or_else(|| "unknown".into())
            } else {
                "unknown".into()
            },
            key: if trust {
                label(get("x-api-key-id"))
            } else {
                None
            },
            parent: TraceContextPropagator::new().extract(&HeaderExtractor(headers)),
        }
    }

    pub fn start(
        &self,
        model: &str,
        operation: &'static str,
        transport: &'static str,
    ) -> UsageGuard {
        UsageGuard {
            request: self.clone(),
            request_id: uuid::Uuid::new_v4().to_string(),
            model: label(Some(model)).unwrap_or_else(|| "unknown".into()),
            response_model: None,
            response_id: None,
            operation,
            transport,
            started: SystemTime::now(),
            clock: Instant::now(),
            status: "cancelled",
            error: Some("client_disconnect"),
            usage: None,
        }
    }
}

#[derive(Default, Debug)]
struct Usage {
    input: Option<u64>,
    output: Option<u64>,
    cached: Option<u64>,
    reasoning: Option<u64>,
    total: Option<u64>,
}
impl Usage {
    fn from_value(value: &Value) -> Option<Self> {
        if !value.is_object() {
            return None;
        }
        let input = value["input_tokens"].as_u64();
        let output = value["output_tokens"].as_u64();
        if input.is_none() && output.is_none() {
            return None;
        }
        Some(Self {
            input,
            output,
            cached: value["input_tokens_details"]["cached_tokens"].as_u64(),
            reasoning: value["output_tokens_details"]["reasoning_tokens"].as_u64(),
            total: value["total_tokens"]
                .as_u64()
                .or_else(|| input?.checked_add(output?)),
        })
    }
}

pub struct UsageGuard {
    request: RequestTelemetry,
    request_id: String,
    model: String,
    response_model: Option<String>,
    response_id: Option<String>,
    operation: &'static str,
    transport: &'static str,
    started: SystemTime,
    clock: Instant,
    status: &'static str,
    error: Option<&'static str>,
    usage: Option<Usage>,
}
impl UsageGuard {
    pub fn failure(&mut self, code: &'static str) {
        self.status = "error";
        self.error = Some(code);
    }

    pub fn observe(&mut self, event: &Value) {
        let response = &event["response"];
        if let Some(id) = label(response["id"].as_str()) {
            self.response_id = Some(id);
        }
        if let Some(model) = label(response["model"].as_str()) {
            self.response_model = Some(model);
        }
        match event["type"].as_str() {
            Some("response.completed") => {
                self.status = "completed";
                self.error = None;
            }
            Some("response.failed") => {
                self.status = "failed";
                self.error = Some("response_failed");
            }
            Some("response.incomplete") => {
                self.status = "incomplete";
                self.error = Some("response_incomplete");
            }
            _ => return,
        }
        self.usage = Usage::from_value(&response["usage"]);
    }
}

impl Drop for UsageGuard {
    fn drop(&mut self) {
        let Some(telemetry) = &self.request.telemetry else {
            return;
        };
        let elapsed = self.clock.elapsed();
        let mut attributes = vec![
            KeyValue::new("gen_ai.operation.name", "chat"),
            KeyValue::new("gen_ai.provider.name", "openai"),
            KeyValue::new("gen_ai.request.model", self.model.clone()),
            KeyValue::new("codex_api.operation", self.operation),
            KeyValue::new("codex_api.transport", self.transport),
            KeyValue::new("codex_api.status", self.status),
        ];
        if let Some(model) = &self.response_model {
            attributes.push(KeyValue::new("gen_ai.response.model", model.clone()));
        }
        if let Some(error) = self.error {
            attributes.push(KeyValue::new("error.type", error));
        }
        // User and request identifiers never become metric dimensions.
        telemetry
            .duration
            .record(elapsed.as_secs_f64(), &attributes);
        if let Some(usage) = &self.usage {
            for (counter, count) in [
                (&telemetry.input, usage.input),
                (&telemetry.output, usage.output),
                (&telemetry.cached, usage.cached),
                (&telemetry.reasoning, usage.reasoning),
            ] {
                if let Some(count) = count {
                    counter.add(count, &attributes);
                }
            }
        } else {
            telemetry.missing.add(1, &attributes);
        }
        attributes.push(KeyValue::new("user.id", self.request.user.clone()));
        attributes.push(KeyValue::new(
            "codex_api.request.id",
            self.request_id.clone(),
        ));
        attributes.push(KeyValue::new(
            "codex_api.usage.available",
            self.usage.is_some(),
        ));
        if let Some(key) = &self.request.key {
            attributes.push(KeyValue::new("codex_api.key.id", key.clone()));
        }
        if let Some(id) = &self.response_id {
            attributes.push(KeyValue::new("gen_ai.response.id", id.clone()));
        }
        let mut body = json!({
            "event": "codex_api.usage", "request_id": self.request_id,
            "user_id": self.request.user, "key_id": self.request.key,
            "request_model": self.model, "model": self.response_model.as_ref().unwrap_or(&self.model),
            "response_id": self.response_id, "operation": self.operation, "transport": self.transport,
            "status": self.status, "error_type": self.error,
            "started_at_unix_ms": self.started.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            "duration_ms": elapsed.as_millis() as u64, "usage_available": self.usage.is_some(),
        });
        if let Some(usage) = &self.usage {
            for (field, key, count) in [
                ("input_tokens", "gen_ai.usage.input_tokens", usage.input),
                ("output_tokens", "gen_ai.usage.output_tokens", usage.output),
                (
                    "cached_tokens",
                    "gen_ai.usage.cache_read.input_tokens",
                    usage.cached,
                ),
                (
                    "reasoning_tokens",
                    "gen_ai.usage.reasoning.output_tokens",
                    usage.reasoning,
                ),
                ("total_tokens", "codex_api.usage.total_tokens", usage.total),
            ] {
                if let Some(count) = count {
                    body[field] = json!(count);
                    if let Ok(count) = i64::try_from(count) {
                        attributes.push(KeyValue::new(key, count));
                    }
                }
            }
        }
        let tracer = telemetry.traces.tracer("codex-api.usage");
        let mut span = tracer
            .span_builder(format!("chat {}", self.model))
            .with_kind(SpanKind::Client)
            .with_start_time(self.started)
            .with_attributes(attributes.clone())
            .start_with_context(&tracer, &self.request.parent);
        if let Some(error) = self.error {
            span.set_status(Status::error(error));
        }
        let logger = telemetry.logs.logger("codex-api.usage");
        let mut log = logger.create_log_record();
        log.set_event_name("gen_ai.client.inference.operation.details");
        log.set_timestamp(SystemTime::now());
        log.set_severity_number(Severity::Info);
        log.set_body(AnyValue::from(body.to_string()));
        for attr in attributes {
            let value = match attr.value {
                opentelemetry::Value::String(v) => AnyValue::String(v),
                opentelemetry::Value::I64(v) => AnyValue::Int(v),
                opentelemetry::Value::Bool(v) => AnyValue::Boolean(v),
                _ => continue,
            };
            log.add_attribute(attr.key, value);
        }
        let context = span.span_context();
        log.set_trace_context(
            context.trace_id(),
            context.span_id(),
            Some(context.trace_flags()),
        );
        logger.emit(log);
        span.end();
    }
}

/// One record per inference, not per WebSocket connection. Match response IDs
/// and lanes so terminal duplicates and multiplexed turns cannot double-count.
pub struct WebSocketUsage {
    request: RequestTelemetry,
    pending: HashMap<String, VecDeque<UsageGuard>>,
    active: HashMap<String, UsageGuard>,
    completed: HashSet<String>,
    recent: VecDeque<String>,
}
impl WebSocketUsage {
    pub fn new(request: RequestTelemetry) -> Self {
        Self {
            request,
            pending: HashMap::new(),
            active: HashMap::new(),
            completed: HashSet::new(),
            recent: VecDeque::new(),
        }
    }
    pub fn request(&mut self, text: &str) {
        let Ok(value) = serde_json::from_str::<Value>(text) else {
            return;
        };
        if value["type"] != "response.create" || value["generate"] == false {
            return;
        }
        let mut guard = self.request.start(
            value["model"].as_str().unwrap_or("unknown"),
            "responses",
            "websocket",
        );
        if self.active.len() + self.pending.values().map(VecDeque::len).sum::<usize>() >= 128 {
            guard.failure("telemetry_capacity");
            return;
        }
        let lane = label(value["stream_id"].as_str()).unwrap_or_default();
        self.pending.entry(lane).or_default().push_back(guard);
    }
    fn take_pending(&mut self, lane: &str) -> Option<UsageGuard> {
        let queue = self.pending.get_mut(lane)?;
        let guard = queue.pop_front();
        if queue.is_empty() {
            self.pending.remove(lane);
        }
        guard
    }
    pub fn observe(&mut self, event: &Value) {
        let lane = label(event["stream_id"].as_str()).unwrap_or_default();
        let id = label(event["response"]["id"].as_str());
        match event["type"].as_str() {
            Some("response.created") => {
                if let Some(id) = id
                    && !self.active.contains_key(&id)
                    && let Some(mut guard) = self.take_pending(&lane)
                {
                    guard.observe(event);
                    self.active.insert(id, guard);
                }
            }
            Some("response.completed" | "response.failed" | "response.incomplete") => {
                if let Some(id) = &id {
                    if !self.completed.insert(id.clone()) {
                        return;
                    }
                    self.recent.push_back(id.clone());
                    if self.recent.len() > 128
                        && let Some(old) = self.recent.pop_front()
                    {
                        self.completed.remove(&old);
                    }
                }
                if let Some(mut guard) = id
                    .as_ref()
                    .and_then(|id| self.active.remove(id))
                    .or_else(|| self.take_pending(&lane))
                {
                    guard.observe(event);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry_sdk::logs::InMemoryLogExporter;

    fn setup(trust: bool) -> (RequestTelemetry, InMemoryLogExporter) {
        let exporter = InMemoryLogExporter::default();
        let telemetry = Arc::new(Telemetry::new(
            SdkTracerProvider::builder().build(),
            SdkLoggerProvider::builder()
                .with_simple_exporter(exporter.clone())
                .build(),
            SdkMeterProvider::builder().build(),
            trust,
        ));
        let mut headers = HeaderMap::new();
        headers.insert("x-user-id", "user-1".parse().unwrap());
        headers.insert("x-api-key-id", "key-1".parse().unwrap());
        (RequestTelemetry::new(Some(telemetry), &headers), exporter)
    }

    fn bodies(exporter: &InMemoryLogExporter) -> Vec<Value> {
        exporter
            .get_emitted_logs()
            .unwrap()
            .iter()
            .map(|log| {
                let Some(AnyValue::String(body)) = log.record.body() else {
                    panic!("JSON body expected")
                };
                serde_json::from_str(body.as_str()).unwrap()
            })
            .collect()
    }

    fn completed(id: &str) -> Value {
        json!({"type":"response.completed", "response":{
        "id":id, "model":"gpt-5", "usage":{
            "input_tokens":12,"output_tokens":3,
            "input_tokens_details":{"cached_tokens":2},
            "output_tokens_details":{"reasoning_tokens":1}
        }}})
    }

    #[test]
    fn identity_usage_and_subset_totals_are_exported() {
        let (request, exporter) = setup(true);
        let mut guard = request.start("gpt-5", "responses", "http");
        guard.observe(&completed("resp-1"));
        drop(guard);
        let body = &bodies(&exporter)[0];
        assert_eq!(body["user_id"], "user-1");
        assert_eq!(body["key_id"], "key-1");
        assert_eq!(body["total_tokens"], 15);
        assert_eq!(body["cached_tokens"], 2);
        assert_eq!(body["reasoning_tokens"], 1);
        assert_eq!(body["status"], "completed");
    }

    #[test]
    fn untrusted_identity_and_missing_usage_remain_explicit() {
        let (request, exporter) = setup(false);
        drop(request.start("gpt-5", "responses", "http"));
        let body = &bodies(&exporter)[0];
        assert_eq!(body["user_id"], "unknown");
        assert!(body["key_id"].is_null());
        assert_eq!(body["usage_available"], false);
        assert!(body.get("total_tokens").is_none());
        assert_eq!(body["status"], "cancelled");
    }

    #[test]
    fn websocket_turns_count_once_and_skip_non_generation() {
        let (request, exporter) = setup(true);
        let mut socket = WebSocketUsage::new(request);
        socket.request(r#"{"type":"response.create","model":"gpt-5","generate":false}"#);
        for id in ["resp-1", "resp-2"] {
            socket.request(r#"{"type":"response.create","model":"gpt-5"}"#);
            socket.observe(&json!({"type":"response.created","response":{"id":id}}));
            socket.observe(&completed(id));
            socket.observe(&completed(id));
        }
        drop(socket);
        let records = bodies(&exporter);
        assert_eq!(records.len(), 2);
        assert!(records.iter().all(|body| body["total_tokens"] == 15));
        assert_ne!(records[0]["request_id"], records[1]["request_id"]);
    }
}
