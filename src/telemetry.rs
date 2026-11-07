//! Usage and timing OpenTelemetry. Never collect content or credentials.
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
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
    inference_duration: Histogram<f64>,
    ttft: Histogram<f64>,
    tps: Histogram<f64>,
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
            inference_duration: meter
                .f64_histogram("codex_api.inference.duration")
                .with_description("Inference start to terminal event, excluding downstream drain")
                .with_unit("s")
                .with_boundaries(vec![0.1, 0.5, 1., 2., 5., 10., 20., 40., 80., 160., 320.])
                .build(),
            ttft: meter
                .f64_histogram("codex_api.inference.time_to_first_token")
                .with_description("Inference start to first non-empty generated content delta")
                .with_unit("s")
                .with_boundaries(vec![
                    0.01, 0.05, 0.1, 0.25, 0.5, 1., 2., 5., 10., 20., 40., 80.,
                ])
                .build(),
            tps: meter
                .f64_histogram("codex_api.inference.output_tokens_per_second")
                .with_description("Reported output tokens divided by inference duration")
                .with_unit("{token}/s")
                .with_boundaries(vec![1., 2., 5., 10., 20., 40., 80., 160., 320., 640.])
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
            first_output: None,
            finished: None,
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
    first_output: Option<Duration>,
    finished: Option<Duration>,
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
        self.observe_at(event, Instant::now());
    }

    pub(crate) fn observe_at(&mut self, event: &Value, received_at: Instant) {
        if self.finished.is_some() {
            return;
        }
        let elapsed = received_at.saturating_duration_since(self.clock);
        if self.first_output.is_none() && is_output_delta(event) {
            self.first_output = Some(elapsed);
        }
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
        self.finished = Some(elapsed);
        self.usage = Usage::from_value(&response["usage"]);
    }

    fn tokens_per_second(&self) -> Option<f64> {
        if self.status != "completed" {
            return None;
        }
        let seconds = self.finished?.as_secs_f64();
        let tokens = self.usage.as_ref()?.output?;
        // No estimate from delta counts: one chunk can contain many tokens.
        (seconds > 0. && tokens > 0).then(|| tokens as f64 / seconds)
    }
}

fn is_output_delta(event: &Value) -> bool {
    matches!(
        event["type"].as_str(),
        Some(
            "response.output_text.delta"
                | "response.refusal.delta"
                | "response.reasoning_text.delta"
                | "response.reasoning_summary_text.delta"
                | "response.function_call_arguments.delta"
                | "response.custom_tool_call_input.delta"
        )
    ) && event["delta"]
        .as_str()
        .is_some_and(|delta| !delta.is_empty())
}

impl Drop for UsageGuard {
    fn drop(&mut self) {
        let Some(telemetry) = &self.request.telemetry else {
            return;
        };
        let elapsed = self.clock.elapsed();
        let inference_duration = self.finished.unwrap_or(elapsed);
        let tps = self.tokens_per_second();
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
        telemetry
            .inference_duration
            .record(inference_duration.as_secs_f64(), &attributes);
        if let Some(ttft) = self.first_output {
            telemetry.ttft.record(ttft.as_secs_f64(), &attributes);
        }
        if let Some(tps) = tps {
            telemetry.tps.record(tps, &attributes);
        }
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
            "codex_api.inference.duration",
            inference_duration.as_secs_f64(),
        ));
        if let Some(ttft) = self.first_output {
            attributes.push(KeyValue::new(
                "codex_api.inference.time_to_first_token",
                ttft.as_secs_f64(),
            ));
        }
        if let Some(tps) = tps {
            attributes.push(KeyValue::new(
                "codex_api.inference.output_tokens_per_second",
                tps,
            ));
        }
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
            "inference_duration_ms": inference_duration.as_secs_f64() * 1000.,
        });
        if let Some(ttft) = self.first_output {
            body["ttft_ms"] = json!(ttft.as_secs_f64() * 1000.);
        }
        if let Some(tps) = tps {
            body["tps"] = json!(tps);
        }
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
                opentelemetry::Value::F64(v) => AnyValue::Double(v),
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
    active: HashMap<String, ActiveUsage>,
    item_responses: HashMap<String, String>,
    completed: HashSet<String>,
    recent: VecDeque<String>,
}

struct ActiveUsage {
    lane: String,
    guard: UsageGuard,
}

impl WebSocketUsage {
    pub fn new(request: RequestTelemetry) -> Self {
        Self {
            request,
            pending: HashMap::new(),
            active: HashMap::new(),
            item_responses: HashMap::new(),
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
        self.observe_at(event, Instant::now());
    }

    pub(crate) fn observe_at(&mut self, event: &Value, received_at: Instant) {
        let lane = label(event["stream_id"].as_str()).unwrap_or_default();
        let id = label(event["response"]["id"].as_str());
        match event["type"].as_str() {
            Some("response.created") => {
                if let Some(id) = id
                    && !self.active.contains_key(&id)
                    && let Some(mut guard) = self.take_pending(&lane)
                {
                    guard.observe_at(event, received_at);
                    self.active.insert(id, ActiveUsage { lane, guard });
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
                    .map(|active| active.guard)
                    .or_else(|| self.take_pending(&lane))
                {
                    guard.observe_at(event, received_at);
                }
                if let Some(id) = id {
                    self.item_responses.retain(|_, response| response != &id);
                }
            }
            _ => {
                if let Some(id) = self.active_id(event) {
                    if self.item_responses.len() < 1024
                        && let Some(item) = label(
                            event["item_id"]
                                .as_str()
                                .or_else(|| event["item"]["id"].as_str()),
                        )
                    {
                        self.item_responses.insert(item, id.clone());
                    }
                    if let Some(active) = self.active.get_mut(&id) {
                        active.guard.observe_at(event, received_at);
                    }
                }
            }
        }
    }

    fn active_id(&self, event: &Value) -> Option<String> {
        if let Some(id) = event
            .get("response_id")
            .or_else(|| event["response"].get("id"))
        {
            let id = label(id.as_str())?;
            return self.active.contains_key(&id).then_some(id);
        }
        if let Some(item) = event["item_id"].as_str() {
            // An unknown item can belong to a finished or untracked response;
            // do not let a late delta start another turn's TTFT clock.
            return self.item_responses.get(item).cloned();
        }
        // Responses deltas often omit response_id. Attribute by an item's ID
        // or an unambiguous lane; never guess between multiplexed responses.
        let lane = event.get("stream_id").map(|v| label(v.as_str()));
        let lane = match lane {
            Some(Some(lane)) => Some(lane),
            Some(None) => return None,
            None => None,
        };
        let mut candidates = self
            .active
            .iter()
            .filter(|(_, active)| lane.as_ref().is_none_or(|lane| &active.lane == lane));
        let (id, _) = candidates.next()?;
        candidates.next().is_none().then(|| id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry_sdk::{
        logs::InMemoryLogExporter,
        metrics::{
            InMemoryMetricExporter,
            data::{AggregatedMetrics, MetricData},
        },
        trace::InMemorySpanExporter,
    };

    fn setup(trust: bool) -> (RequestTelemetry, InMemoryLogExporter) {
        let (request, logs, _, _) = setup_exporters(trust);
        (request, logs)
    }

    fn setup_exporters(
        trust: bool,
    ) -> (
        RequestTelemetry,
        InMemoryLogExporter,
        InMemorySpanExporter,
        InMemoryMetricExporter,
    ) {
        let exporter = InMemoryLogExporter::default();
        let spans = InMemorySpanExporter::default();
        let metrics = InMemoryMetricExporter::default();
        let telemetry = Arc::new(Telemetry::new(
            SdkTracerProvider::builder()
                .with_simple_exporter(spans.clone())
                .build(),
            SdkLoggerProvider::builder()
                .with_simple_exporter(exporter.clone())
                .build(),
            SdkMeterProvider::builder()
                .with_periodic_exporter(metrics.clone())
                .build(),
            trust,
        ));
        let mut headers = HeaderMap::new();
        headers.insert("x-user-id", "user-1".parse().unwrap());
        headers.insert("x-api-key-id", "key-1".parse().unwrap());
        (
            RequestTelemetry::new(Some(telemetry), &headers),
            exporter,
            spans,
            metrics,
        )
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

    #[test]
    fn timings_export_to_logs_spans_and_histograms_without_identity_dimensions() {
        let (request, logs, spans, metrics) = setup_exporters(true);
        let mut guard = request.start("gpt-5", "responses", "http");
        guard.clock = Instant::now() - Duration::from_secs(10);
        let clock = guard.clock;
        guard.observe_at(
            &json!({"type":"response.created","response":{"id":"resp-1"}}),
            clock + Duration::from_millis(100),
        );
        guard.observe_at(
            &json!({"type":"response.output_text.delta","delta":""}),
            clock + Duration::from_millis(500),
        );
        guard.observe_at(
            &json!({"type":"response.output_text.delta","delta":"several tokens in one chunk"}),
            clock + Duration::from_secs(1),
        );
        guard.observe_at(
            &json!({"type":"response.output_text.delta","delta":"more"}),
            clock + Duration::from_secs(2),
        );
        guard.observe_at(&completed("resp-1"), clock + Duration::from_secs(4));
        guard.observe_at(&completed("resp-1"), clock + Duration::from_secs(8));
        drop(guard);

        let body = &bodies(&logs)[0];
        assert_eq!(body["ttft_ms"], 1000.);
        assert_eq!(body["inference_duration_ms"], 4000.);
        assert_eq!(body["tps"], 0.75);
        assert!(body["duration_ms"].as_u64().unwrap() >= 10_000);
        let values = [
            ("codex_api.inference.duration", "s", 4.),
            ("codex_api.inference.time_to_first_token", "s", 1.),
            (
                "codex_api.inference.output_tokens_per_second",
                "{token}/s",
                0.75,
            ),
        ];
        let spans = spans.get_finished_spans().unwrap();
        assert_eq!(spans.len(), 1);
        let records = logs.get_emitted_logs().unwrap();
        for (name, _, expected) in values {
            assert!(spans[0].attributes.contains(&KeyValue::new(name, expected)));
            assert!(records[0].record.attributes_iter().any(|(key, value)| {
                key.as_str() == name && value == &AnyValue::Double(expected)
            }));
        }
        request
            .telemetry
            .as_ref()
            .unwrap()
            .metrics
            .force_flush()
            .unwrap();
        let mut checked = 0;
        for resource in metrics.get_finished_metrics().unwrap() {
            for scope in resource.scope_metrics() {
                for metric in scope.metrics() {
                    let Some((_, unit, expected)) =
                        values.iter().find(|(name, _, _)| *name == metric.name())
                    else {
                        continue;
                    };
                    assert_eq!(metric.unit(), *unit);
                    let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data()
                    else {
                        panic!("f64 histogram expected");
                    };
                    let points: Vec<_> = histogram.data_points().collect();
                    assert_eq!(points.len(), 1);
                    assert_eq!(points[0].count(), 1);
                    assert_eq!(points[0].sum(), *expected);
                    assert!(points[0].attributes().all(|attribute| {
                        !matches!(
                            attribute.key.as_str(),
                            "user.id"
                                | "codex_api.key.id"
                                | "codex_api.request.id"
                                | "gen_ai.response.id"
                        )
                    }));
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 3);
        assert!(!body.to_string().contains("several tokens"));
    }

    #[test]
    fn first_output_supports_reasoning_refusals_and_tool_only_responses() {
        let (request, exporter) = setup(false);
        for kind in [
            "response.output_text.delta",
            "response.refusal.delta",
            "response.reasoning_text.delta",
            "response.reasoning_summary_text.delta",
            "response.function_call_arguments.delta",
            "response.custom_tool_call_input.delta",
        ] {
            let mut guard = request.start("gpt-5", "responses", "http");
            guard.clock = Instant::now() - Duration::from_secs(10);
            let clock = guard.clock;
            guard.observe_at(
                &json!({"type":"response.output_item.added","item":{"id":"item-1"}}),
                clock + Duration::from_millis(10),
            );
            guard.observe_at(
                &json!({"type":kind,"delta":"generated content"}),
                clock + Duration::from_millis(250),
            );
            guard.observe_at(&completed("resp-1"), clock + Duration::from_secs(1));
        }
        let records = bodies(&exporter);
        assert_eq!(records.len(), 6);
        assert!(
            records
                .iter()
                .all(|body| body["ttft_ms"] == 250. && body["tps"] == 3.)
        );
    }

    #[test]
    fn missing_deltas_usage_and_zero_values_do_not_invent_rates() {
        let (request, exporter) = setup(false);
        for case in [
            "no_deltas",
            "no_usage",
            "zero_tokens",
            "zero_time",
            "failed",
            "incomplete",
            "cancelled",
            "parser_error",
        ] {
            let mut guard = request.start("gpt-5", "responses", "http");
            guard.clock = Instant::now() - Duration::from_secs(10);
            let clock = guard.clock;
            if case != "no_deltas" && case != "zero_time" {
                guard.observe_at(
                    &json!({"type":"response.output_text.delta","delta":"generated"}),
                    clock + Duration::from_millis(500),
                );
            }
            let mut event = completed(case);
            match case {
                "no_usage" => event["response"]["usage"] = Value::Null,
                "zero_tokens" => event["response"]["usage"]["output_tokens"] = json!(0),
                "failed" => event["type"] = json!("response.failed"),
                "incomplete" => event["type"] = json!("response.incomplete"),
                _ => {}
            }
            if case != "cancelled" {
                let duration = if case == "zero_time" {
                    Duration::ZERO
                } else {
                    Duration::from_secs(2)
                };
                guard.observe_at(&event, clock + duration);
            }
            if case == "parser_error" {
                guard.failure("upstream_error");
            }
            drop(guard);
            let records = bodies(&exporter);
            let body = records.last().unwrap();
            if case == "no_deltas" {
                assert!(body.get("ttft_ms").is_none());
                assert_eq!(body["tps"], 1.5);
            } else {
                assert!(body.get("tps").is_none(), "{case}: {body}");
            }
        }
    }

    #[test]
    fn websocket_multiplexed_items_lanes_and_response_ids_have_independent_timings() {
        let (request, exporter) = setup(false);
        let mut socket = WebSocketUsage::new(request);
        let clock = Instant::now() - Duration::from_secs(10);
        for lane in ["a", "b"] {
            socket.request(
                &json!({"type":"response.create","model":"gpt-5","stream_id":lane}).to_string(),
            );
            socket.pending.get_mut(lane).unwrap()[0].clock = clock;
            socket.observe_at(
                &json!({"type":"response.created","stream_id":lane,"response":{"id":format!("resp-{lane}")}}),
                clock + Duration::from_millis(100),
            );
        }
        socket.observe_at(
            &json!({"type":"response.output_text.delta","delta":"ambiguous"}),
            clock + Duration::from_millis(200),
        );
        socket.observe_at(
            &json!({"type":"response.output_text.delta","response_id":"unknown","delta":"unrelated"}),
            clock + Duration::from_millis(300),
        );
        socket.observe_at(
            &json!({"type":"response.output_item.added","stream_id":"a","item":{"id":"item-a"}}),
            clock + Duration::from_millis(400),
        );
        socket.observe_at(
            &json!({"type":"response.output_text.delta","item_id":"item-a","delta":"a"}),
            clock + Duration::from_secs(1),
        );
        socket.observe_at(
            &json!({"type":"response.function_call_arguments.delta","response_id":"resp-b","delta":"{}"}),
            clock + Duration::from_secs(3),
        );
        for (id, seconds) in [("resp-b", 4), ("resp-a", 6)] {
            socket.observe_at(&completed(id), clock + Duration::from_secs(seconds));
            socket.observe_at(&completed(id), clock + Duration::from_secs(9));
        }
        assert!(socket.item_responses.is_empty());
        drop(socket);
        let records = bodies(&exporter);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["response_id"], "resp-b");
        assert_eq!(records[0]["ttft_ms"], 3000.);
        assert_eq!(records[0]["tps"], 0.75);
        assert_eq!(records[1]["response_id"], "resp-a");
        assert_eq!(records[1]["ttft_ms"], 1000.);
        assert_eq!(records[1]["tps"], 0.5);
    }

    #[test]
    fn websocket_lane_deltas_require_one_active_response_in_that_lane() {
        let (request, exporter) = setup(false);
        let mut socket = WebSocketUsage::new(request);
        let clock = Instant::now() - Duration::from_secs(10);
        for (id, lane) in [
            ("resp-1", "shared"),
            ("resp-2", "shared"),
            ("resp-3", "other"),
        ] {
            socket.request(
                &json!({"type":"response.create","model":"gpt-5","stream_id":lane}).to_string(),
            );
            socket.pending.get_mut(lane).unwrap()[0].clock = clock;
            socket.observe_at(
                &json!({"type":"response.created","stream_id":lane,"response":{"id":id}}),
                clock + Duration::from_millis(100),
            );
        }
        socket.observe_at(
            &json!({"type":"response.output_text.delta","stream_id":"other","item_id":"old-item","delta":"late"}),
            clock + Duration::from_millis(500),
        );
        socket.observe_at(
            &json!({"type":"response.output_text.delta","stream_id":"shared","delta":"ambiguous"}),
            clock + Duration::from_secs(1),
        );
        socket.observe_at(
            &json!({"type":"response.output_text.delta","stream_id":"other","delta":"clear"}),
            clock + Duration::from_secs(2),
        );
        for id in ["resp-1", "resp-2", "resp-3"] {
            socket.observe_at(&completed(id), clock + Duration::from_secs(4));
        }
        drop(socket);
        let records = bodies(&exporter);
        assert!(records[0].get("ttft_ms").is_none());
        assert!(records[1].get("ttft_ms").is_none());
        assert_eq!(records[2]["ttft_ms"], 2000.);
    }
}
