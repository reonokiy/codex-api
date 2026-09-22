//! Opt-in real-subscription smoke test. Uses only a local gateway key, never account tokens.
#[path = "support/live_gateway.rs"]
mod live_gateway;
#[path = "support/python.rs"]
mod python;
use codex_api::{
    ResponseCreateWsRequest, ResponseEvent, ResponsesWebsocketClient, ResponsesWsRequest,
};
use codex_api_gateway::request::CreateResponse;
use codex_http_client::{HttpClientFactory, OutboundProxyPolicy};
use futures::StreamExt;
use http::HeaderMap;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
struct Key(String);
impl codex_api::AuthProvider for Key {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        headers.insert(
            "authorization",
            format!("Bearer {}", self.0).parse().unwrap(),
        );
    }
}

#[tokio::test]
#[ignore = "requires codex login; consumes subscription usage"]
async fn real_subscription_http_ws_lite_and_compaction() {
    let gateway = live_gateway::LiveGateway::start().await;
    let base = &gateway.url;
    let key = gateway.key.clone();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(100))
        .build()
        .unwrap();
    let models = codex_models_manager::bundled_models_response()
        .unwrap()
        .models;
    let mut results = Vec::new();
    for name in ["gpt-5.5", "gpt-5.6-terra"] {
        let model = models.iter().find(|m| m.slug == name).unwrap();
        let input = json!({"model":name,"input":"Reply exactly OK.","instructions":"Reply briefly. Do not use tools.","reasoning":{"effort":"low"}});
        let body: CreateResponse = serde_json::from_value(input.clone()).unwrap();
        let request = body.into_codex(model, "live-test-session").unwrap();
        for endpoint in ["v1", "codex"] {
            let response = if endpoint == "v1" {
                client
                    .post(format!("{base}/v1/responses"))
                    .bearer_auth(&key)
                    .json(&input)
                    .send()
                    .await
                    .unwrap()
            } else {
                let mut call = client
                    .post(format!("{base}/codex/responses"))
                    .bearer_auth(&key)
                    .json(&request);
                if model.use_responses_lite {
                    call = call.header("x-openai-internal-codex-responses-lite", "true");
                }
                call.send().await.unwrap()
            };
            let status = response.status();
            let text = response.text().await.unwrap();
            let success = if endpoint == "v1" {
                serde_json::from_str::<Value>(&text).is_ok_and(|v| {
                    v["status"] == "completed"
                        && v["output"]
                            .as_array()
                            .is_some_and(|items| items.iter().any(|i| i["type"] == "message"))
                })
            } else {
                text.contains("\"type\":\"response.completed\"")
                    || text.contains("\"type\": \"response.completed\"")
            };
            results.push(json!({"transport":"http","endpoint":endpoint,"model":name,"status":status.as_u16(),"completed":success}));
            save(&results);
            assert!(
                status.is_success() && success,
                "{endpoint} {name} failed: HTTP {status}; expected a completed response"
            );
        }
        for endpoint in ["codex", "v1"] {
            let provider = codex_api::Provider {
                name: "OpenAI".into(),
                base_url: format!("{base}/{endpoint}"),
                query_params: None,
                headers: HeaderMap::new(),
                retry: codex_api::RetryConfig {
                    max_attempts: 1,
                    base_delay: Duration::from_millis(1),
                    retry_429: false,
                    retry_5xx: false,
                    retry_transport: false,
                },
                stream_idle_timeout: Duration::from_secs(90),
            };
            let ws = ResponsesWebsocketClient::new(provider, Arc::new(Key(key.clone())));
            let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
            let mut headers = HeaderMap::new();
            headers.insert(
                "openai-beta",
                "responses_websockets=2026-02-06".parse().unwrap(),
            );
            let connection = ws
                .connect(
                    &factory,
                    headers,
                    codex_login::default_client::default_headers(),
                    None,
                    None,
                )
                .await;
            let connection = match connection {
                Ok(v) => v,
                Err(e) => {
                    results.push(json!({"transport":"websocket","endpoint":endpoint,"model":name,"connected":false,"error":e.to_string()}));
                    save(&results);
                    panic!("{e}");
                }
            };
            let mut previous = None;
            for phase in ["warmup", "generate", "compact"] {
                let mut turn = request.clone();
                if phase == "compact" {
                    turn.input = vec![codex_protocol::models::ResponseItem::CompactionTrigger {}];
                } else if previous.is_some() {
                    // Warmup already cached the complete prompt.
                    turn.input.clear();
                }
                let mut wire = ResponseCreateWsRequest::from(&turn);
                wire.generate = Some(phase != "warmup");
                wire.previous_response_id = previous.clone();
                if model.use_responses_lite {
                    wire.client_metadata = Some(std::collections::HashMap::from([(
                        "ws_request_header_x_openai_internal_codex_responses_lite".into(),
                        "true".into(),
                    )]));
                }
                let mut events = connection
                    .stream_request(
                        ResponsesWsRequest::ResponseCreate(wire),
                        previous.is_some(),
                        None,
                    )
                    .await
                    .unwrap();
                let mut completed = false;
                let mut compact = false;
                let mut text_received = false;
                while let Some(event) = events.next().await {
                    match event.unwrap() {
                        ResponseEvent::Completed { response_id, .. } => {
                            completed = true;
                            previous = Some(response_id);
                        }
                        ResponseEvent::OutputItemDone(
                            codex_protocol::models::ResponseItem::Compaction { .. },
                        ) => compact = true,
                        ResponseEvent::OutputTextDelta(_) => text_received = true,
                        _ => {}
                    }
                }
                let correct = completed
                    && (phase != "compact" || compact)
                    && (phase != "generate" || text_received);
                results.push(json!({"transport":"websocket","endpoint":endpoint,"model":name,"operation":phase,"completed":completed,"expected_output_received":correct}));
                save(&results);
                assert!(correct, "{endpoint} {name} {phase}");
            }
        }
        for endpoint in ["v1", "codex"] {
            let response = client
                .post(format!("{base}/{endpoint}/responses/compact"))
                .bearer_auth(&key)
                .json(&input)
                .send()
                .await
                .unwrap();
            let status = response.status();
            let body: Value = response.json().await.unwrap();
            let success = body["object"] == "response.compaction"
                && body["output"]
                    .as_array()
                    .is_some_and(|v| v.iter().any(|i| i["type"] == "compaction"));
            results.push(json!({"transport":"http","operation":"compact","endpoint":endpoint,"model":name,"status":status.as_u16(),"completed":success}));
            save(&results);
            assert!(
                status.is_success() && success,
                "{endpoint} compact {name}: expected compaction output, HTTP {status}"
            );
        }
    }
}

#[tokio::test]
#[ignore = "requires codex login and uv sync --locked --group live; consumes subscription usage"]
async fn real_subscription_openai_sdk() {
    let prerequisites = python::command()
        .args(["-c", "import openai, aiortc, websockets"])
        .output()
        .await
        .expect("start Python; set CODEX_TEST_PYTHON if needed");
    assert!(
        prerequisites.status.success(),
        "Run uv sync --locked --group live (see docs/e2e.md): {}",
        String::from_utf8_lossy(&prerequisites.stderr)
    );
    let gateway = live_gateway::LiveGateway::start().await;
    let report = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("artifacts/e2e/sdk/live-results.json");
    let output = tokio::time::timeout(
        Duration::from_secs(1200),
        python::command()
            .env("CODEX_GATEWAY_API_KEY", &gateway.key)
            .args([
                "-c",
                include_str!("support/live_sdk.py"),
                "--url",
                &gateway.url,
                "--report",
            ])
            .arg(&report)
            .output(),
    )
    .await
    .expect("SDK E2E exceeded 20 minutes")
    .expect("start SDK E2E helper");
    assert!(
        output.status.success(),
        "SDK E2E failed; report: {}\n{}\n{}",
        report.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn save(results: &[Value]) {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("artifacts/protocol");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("live-results.json"),serde_json::to_vec_pretty(&json!({"codex_revision":codex_api_gateway::CODEX_REV,"synthetic":false,"results":results})).unwrap()).unwrap();
}
