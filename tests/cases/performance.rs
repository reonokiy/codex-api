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
async fn http_timings_include_body_upload_rejections_and_head() {
    let logs = InMemoryLogExporter::default();
    let telemetry = Arc::new(Telemetry::new(
        SdkTracerProvider::builder().build(),
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
    let payload = json!({"model":h.model,"input":"hello","stream":false}).to_string();
    let upload = reqwest::Body::wrap_stream(async_stream::stream! {
        yield Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"{"));
        tokio::time::sleep(Duration::from_millis(150)).await;
        yield Ok(bytes::Bytes::copy_from_slice(&payload.as_bytes()[1..]));
    });
    let response = client
        .post(format!("{}/v1/responses", h.url))
        .bearer_auth("client-key")
        .header("content-type", "application/json")
        .body(upload)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response.bytes().await.unwrap();
    for (method, path, auth, payload, status) in [
        (reqwest::Method::POST, "/v1/responses", false, "{}", 401),
        (
            reqwest::Method::GET,
            "/unreviewed/private-id",
            true,
            "",
            403,
        ),
        (reqwest::Method::POST, "/v1/responses", true, "{", 400),
        (reqwest::Method::DELETE, "/v1/responses", true, "", 403),
        (reqwest::Method::HEAD, "/healthz", true, "", 200),
    ] {
        let mut request = client
            .request(method, format!("{}{path}", h.url))
            .header("content-type", "application/json")
            .body(payload);
        if auth {
            request = request.bearer_auth("client-key");
        }
        let response = request.send().await.unwrap();
        assert_eq!(response.status().as_u16(), status);
        response.bytes().await.unwrap();
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let records = logs.get_emitted_logs().unwrap();
            let count = records
                .iter()
                .filter(|record| record.record.event_name() == Some("codex_api.http.request"))
                .count();
            if count == 6 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let bodies: Vec<Value> = logs
        .get_emitted_logs()
        .unwrap()
        .iter()
        .filter_map(|record| {
            let Some(AnyValue::String(body)) = record.record.body() else {
                return None;
            };
            let body: Value = serde_json::from_str(body.as_str()).unwrap();
            (body["event"] == "codex_api.http.request").then_some(body)
        })
        .collect();
    assert_eq!(bodies.len(), 6);
    assert!(bodies[0]["ttfb_ms"].as_f64().unwrap() >= 100.);
    assert!(bodies.iter().all(|body| {
        body["outcome"] == "completed"
            && body["total_duration_ms"].as_f64().unwrap() >= body["ttfb_ms"].as_f64().unwrap()
    }));
    assert!(bodies.iter().any(|body| body["method"] == "HEAD"));
    assert!(bodies.iter().any(|body| body["status_code"] == 401));
    assert!(bodies.iter().any(|body| body["status_code"] == 400));
    assert!(
        !serde_json::to_string(&bodies)
            .unwrap()
            .contains("private-id")
    );
}

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
        while logs.get_emitted_logs().unwrap().len() < 14 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let records = logs.get_emitted_logs().unwrap();
    assert_eq!(records.len(), 14);
    let mut transports = HashMap::new();
    let mut http_count = 0;
    for record in &records {
        let Some(AnyValue::String(body)) = record.record.body() else {
            panic!("JSON usage log expected");
        };
        let body: Value = serde_json::from_str(body.as_str()).unwrap();
        if body["event"] == "codex_api.http.request" {
            let ttfb = body["ttfb_ms"].as_f64().unwrap();
            let duration = body["total_duration_ms"].as_f64().unwrap();
            assert!(ttfb > 0. && duration >= ttfb);
            if body["status_code"] == 101 {
                assert_eq!(body["method"], "GET");
                assert_eq!(body["outcome"], "upgraded");
            } else {
                assert_eq!(body["method"], "POST");
                assert_eq!(body["status_code"], 200);
                assert_eq!(body["outcome"], "completed");
            }
            http_count += 1;
            continue;
        }
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
    assert_eq!(http_count, 7);
    assert_eq!(spans.get_finished_spans().unwrap().len(), 14);
}
