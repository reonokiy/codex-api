use super::*;
use codex_api_gateway::telemetry::Telemetry;
use opentelemetry::logs::AnyValue;
use opentelemetry_sdk::{
    logs::{InMemoryLogExporter, SdkLoggerProvider},
    metrics::SdkMeterProvider,
    trace::{InMemorySpanExporter, SdkTracerProvider},
};
use std::collections::HashMap;

#[tokio::test]
async fn inference_timings_reach_exporters_for_http_sse_and_websocket_aliases() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let logs = InMemoryLogExporter::default();
    let spans = InMemorySpanExporter::default();
    let telemetry = Arc::new(Telemetry::new(
        SdkTracerProvider::builder()
            .with_simple_exporter(spans.clone())
            .build(),
        SdkLoggerProvider::builder()
            .with_simple_exporter(logs.clone())
            .build(),
        SdkMeterProvider::builder().build(),
        false,
    ));
    let h = Harness::with_api_policy(
        events(),
        StatusCode::OK,
        false,
        Duration::from_secs(5),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        false,
        16 * 1024 * 1024,
        false,
        Some(telemetry),
    )
    .await;
    let client = reqwest::Client::new();
    for (prefix, stream) in [
        ("v1", false),
        ("v1", true),
        ("codex", true),
        ("backend-api/codex", true),
    ] {
        let response = client
            .post(format!("{}/{prefix}/responses", h.url))
            .bearer_auth("client-key")
            .json(&json!({"model":h.model,"input":"hello","stream":stream}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.text().await.unwrap().contains("completed"));
    }
    for prefix in ["v1", "codex", "backend-api/codex"] {
        let mut request = format!("{}/{prefix}/responses", h.url)
            .replacen("http:", "ws:", 1)
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("authorization", "Bearer client-key".parse().unwrap());
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        socket
            .send(Message::Text(
                json!({"type":"response.create","model":h.model,"input":[],"stream":true})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let event = socket.next().await.unwrap().unwrap().into_text().unwrap();
                if serde_json::from_str::<Value>(&event).unwrap()["type"] == "response.completed" {
                    break;
                }
            }
        })
        .await
        .unwrap();
        socket.close(None).await.unwrap();
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        while logs.get_emitted_logs().unwrap().len() < 7 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let records = logs.get_emitted_logs().unwrap();
    assert_eq!(records.len(), 7);
    let mut transports = HashMap::new();
    for record in &records {
        let Some(AnyValue::String(body)) = record.record.body() else {
            panic!("JSON usage log expected");
        };
        let body: Value = serde_json::from_str(body.as_str()).unwrap();
        let ttft = body["ttft_ms"].as_f64().unwrap();
        let duration = body["inference_duration_ms"].as_f64().unwrap();
        let tps = body["tps"].as_f64().unwrap();
        assert!(ttft > 0. && duration >= ttft);
        assert!(tps.is_finite() && tps > 0.);
        assert!((tps * duration / 1000. - 3.).abs() < 1e-6);
        assert_eq!(body["status"], "completed");
        *transports
            .entry(body["transport"].as_str().unwrap().to_owned())
            .or_insert(0) += 1;
    }
    assert_eq!(transports["http"], 4);
    assert_eq!(transports["websocket"], 3);
    assert_eq!(spans.get_finished_spans().unwrap().len(), 7);
}
