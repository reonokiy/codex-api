mod support;
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
};
use codex_api_gateway::{
    backend::Backend,
    server::{Gateway, router},
};
use codex_http_client::{HttpClientFactory, OutboundProxyPolicy};
use codex_login::{AuthManager, CodexAuth};
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;

#[derive(Clone)]
struct Fake {
    received: Arc<Mutex<Vec<(HeaderMap, Value)>>>,
    events: Vec<Value>,
    status: StatusCode,
    hang: bool,
    ws_frames: Arc<Mutex<Vec<String>>>,
    http_bodies: Arc<Mutex<Vec<Vec<u8>>>>,
    ws_connections: Arc<std::sync::atomic::AtomicUsize>,
    image_paths: Arc<Mutex<Vec<String>>>,
    cli_image_path: Arc<Mutex<Option<String>>>,
    cli_turn: Arc<std::sync::atomic::AtomicUsize>,
}
async fn upstream(State(fake): State<Fake>, headers: HeaderMap, bytes: Bytes) -> Response {
    fake.http_bodies.lock().unwrap().push(bytes.to_vec());
    let data = if headers.get("content-encoding").is_some_and(|v| v == "zstd") {
        zstd::decode_all(bytes.as_ref()).unwrap()
    } else {
        bytes.to_vec()
    };
    let value: Value = serde_json::from_slice(&data).unwrap();
    fake.received.lock().unwrap().push((headers, value));
    if fake.status != StatusCode::OK {
        return Response::builder()
            .status(fake.status)
            .body(Body::from("upstream rejected request"))
            .unwrap();
    }
    let mut wire = String::new();
    let cli_path = fake.cli_image_path.lock().unwrap().clone();
    let cli_events = cli_path.map(|path| {
        let turn = fake.cli_turn.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if turn > 2 { return events(); }
        if turn == 2 {
            let call = json!({"type":"function_call","id":"fc_web_2","call_id":"call_web_2","namespace":"web","name":"run","arguments":json!({"search_query":[{"q":"Codex"}]}).to_string()});
            return vec![json!({"type":"response.output_item.done","output_index":0,"item":call}),completed(vec![call])];
        }
        let args = if turn == 0 { json!({"prompt":"Generate a tiny white square"}) }
            else { json!({"prompt":"Make the square blue","referenced_image_paths":[path]}) };
        let call = json!({"type":"function_call","id":format!("fc_image_{turn}"),"call_id":format!("call_image_{turn}"),"namespace":"image_gen","name":"imagegen","arguments":args.to_string(),"status":"completed"});
        vec![json!({"type":"response.output_item.done","output_index":0,"item":call}), completed(vec![call])]
    });
    for event in cli_events.as_ref().unwrap_or(&fake.events) {
        wire.push_str(&format!(
            "event: {}\ndata: {event}\n\n",
            event["type"].as_str().unwrap()
        ));
    }
    let chunks: Vec<_> = wire
        .as_bytes()
        .chunks(7)
        .map(Bytes::copy_from_slice)
        .collect();
    let stream = async_stream::stream! {
        for chunk in chunks { yield Ok::<_, std::io::Error>(chunk); }
        if fake.hang { std::future::pending::<()>().await; }
    };
    Response::builder()
        .header("content-type", "text/event-stream")
        .header("x-codex-turn-state", "upstream-state")
        .header("x-models-etag", "catalog-1")
        .body(Body::from_stream(stream))
        .unwrap()
}
async fn upstream_ws(
    State(fake): State<Fake>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    use axum::extract::ws::Message;
    use futures::StreamExt;
    if fake.status != StatusCode::OK {
        return Response::builder()
            .status(fake.status)
            .body(Body::from("handshake rejected"))
            .unwrap();
    }
    fake.ws_connections
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let mut response = ws.on_upgrade(move |mut socket| async move {
        while let Some(Ok(message)) = socket.next().await {
            match message {
                Message::Text(text) => {
                    fake.ws_frames.lock().unwrap().push(text.to_string());
                    let body: Value = serde_json::from_str(&text).unwrap();
                    fake.received.lock().unwrap().push((headers.clone(), body));
                    for event in &fake.events {
                        if socket
                            .send(Message::Text(event.to_string().into()))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                }
                Message::Binary(data) => {
                    let sent = socket.send(Message::Binary(data)).await;
                    if sent.is_err() {
                        return;
                    }
                }
                Message::Close(_) => return,
                _ => {}
            }
        }
    });
    response
        .headers_mut()
        .insert("x-codex-turn-state", "ws-state".parse().unwrap());
    response
        .headers_mut()
        .insert("x-models-etag", "models-ws".parse().unwrap());
    response
}

struct Harness {
    url: String,
    upstream_url: String,
    capture: support::wire::Capture,
    model: String,
    fake: Fake,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
impl Drop for Harness {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
impl Harness {
    async fn new(events: Vec<Value>, status: StatusCode, hang: bool, timeout: Duration) -> Self {
        Self::with_auth(
            events,
            status,
            hang,
            timeout,
            CodexAuth::from_api_key("upstream-secret"),
        )
        .await
    }
    async fn with_auth(
        events: Vec<Value>,
        status: StatusCode,
        hang: bool,
        timeout: Duration,
        login: CodexAuth,
    ) -> Self {
        let subscription = login.is_chatgpt_auth();
        let fake = Fake {
            received: Default::default(),
            events,
            status,
            hang,
            ws_frames: Default::default(),
            http_bodies: Default::default(),
            ws_connections: Default::default(),
            image_paths: Default::default(),
            cli_image_path: Default::default(),
            cli_turn: Default::default(),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_url = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/responses", post(upstream).get(upstream_ws))
            .route("/alpha/search", post(upstream_search))
            .route("/images/generations", post(upstream_image))
            .route("/images/edits", post(upstream_image))
            .route(
                "/models",
                axum::routing::get(|| async {
                    axum::Json(codex_models_manager::bundled_models_response().unwrap())
                }),
            )
            .with_state(fake.clone());
        let upstream_task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let capture = support::wire::Capture::start(upstream_url).await;
        let upstream_url = capture.url.clone();
        let auth = AuthManager::from_auth_for_testing(login);
        let backend = Backend {
            provider: create_model_provider(
                ModelProviderInfo::create_openai_provider(Some(upstream_url.clone())),
                Some(auth.clone()),
            ),
            auth,
            factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            chatgpt_base_url: "https://chatgpt.com/backend-api".into(),
            subscription_only: subscription,
            compression: subscription,
            agent_identity_policy: codex_login::AgentIdentityAuthPolicy::JwtOnly,
        };
        let models = codex_models_manager::bundled_models_response()
            .unwrap()
            .models;
        let model = models
            .iter()
            .find(|m| !m.use_responses_lite)
            .unwrap()
            .slug
            .clone();
        let app = router(Gateway {
            backend,
            models,
            key: Some("client-key".into()),
            concurrency: Arc::new(Semaphore::new(1)),
            timeout,
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let gateway_task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url,
            upstream_url,
            capture,
            model,
            fake,
            tasks: vec![upstream_task, gateway_task],
        }
    }
    fn request(&self, mut body: Value) -> reqwest::RequestBuilder {
        body["model"] = json!(self.model);
        reqwest::Client::new()
            .post(format!("{}/v1/responses", self.url))
            .bearer_auth("client-key")
            .json(&body)
    }
}
fn message() -> Value {
    json!({"type":"message","id":"msg_1","status":"completed","role":"assistant","content":[{"type":"output_text","text":"你好","annotations":[]}]})
}
fn completed(output: Vec<Value>) -> Value {
    json!({"type":"response.completed","sequence_number":9,"response":{"id":"resp_1","object":"response","created_at":1,"status":"completed","output":output,"usage":{"input_tokens":12,"output_tokens":3,"total_tokens":15,"input_tokens_details":{"cached_tokens":2},"output_tokens_details":{"reasoning_tokens":1}},"future_field":{"kept":true}}})
}
fn events() -> Vec<Value> {
    vec![
        json!({"type":"response.created","sequence_number":0,"response":{"id":"resp_1","object":"response","status":"in_progress"}}),
        json!({"type":"response.output_item.added","sequence_number":1,"output_index":0,"item":message()}),
        json!({"type":"response.content_part.added","sequence_number":2,"item_id":"msg_1","output_index":0,"content_index":0,"part":{"type":"output_text","text":"","annotations":[]}}),
        json!({"type":"response.output_text.delta","sequence_number":3,"item_id":"msg_1","output_index":0,"content_index":0,"delta":"你好","future_field":"unchanged"}),
        json!({"type":"response.output_item.done","sequence_number":8,"output_index":0,"item":message()}),
        completed(vec![message()]),
    ]
}
fn parse_sse(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str(data).unwrap())
        .collect()
}

#[tokio::test]
async fn stream_preserves_full_events_and_uses_codex_request_and_headers() {
    let expected = events();
    let h = Harness::new(
        expected.clone(),
        StatusCode::OK,
        false,
        Duration::from_secs(10),
    )
    .await;
    let response = h
        .request(json!({"input":"hello","stream":true}))
        .header("ChatGPT-Account-ID", "client-spoof")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(parse_sse(&response.text().await.unwrap()), expected);
    let received = h.fake.received.lock().unwrap();
    assert_eq!(received.len(), 1);
    let (headers, body) = &received[0];
    assert_eq!(headers["authorization"], "Bearer upstream-secret");
    assert!(headers.get("chatgpt-account-id").is_none());
    assert!(headers.contains_key("originator"));
    assert!(headers.contains_key("user-agent"));
    assert_eq!(headers["session-id"], headers["thread-id"]);
    assert_eq!(body["model"], h.model);
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
    assert_eq!(body["tool_choice"], "auto");
    assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
    assert_eq!(
        body["input"],
        json!([{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}])
    );
    assert!(!body["instructions"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn nonstream_returns_the_entire_upstream_response() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(10)).await;
    let response = h.request(json!({"input":"hi"})).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        completed(vec![message()])["response"]
    );
}

#[tokio::test]
async fn function_arguments_stream_and_tool_results_are_not_executed_or_lost() {
    let call = json!({"type":"function_call","id":"fc_1","call_id":"call_1","name":"weather","arguments":"{\"city\":\"Berlin\"}","status":"completed"});
    let expected = vec![
        events()[0].clone(),
        json!({"type":"response.function_call_arguments.delta","sequence_number":1,"item_id":"fc_1","output_index":0,"delta":"{\"city\":"}),
        json!({"type":"response.function_call_arguments.done","sequence_number":2,"item_id":"fc_1","output_index":0,"arguments":"{\"city\":\"Berlin\"}"}),
        json!({"type":"response.output_item.done","sequence_number":3,"output_index":0,"item":call}),
        completed(vec![call.clone()]),
    ];
    let h = Harness::new(
        expected.clone(),
        StatusCode::OK,
        false,
        Duration::from_secs(10),
    )
    .await;
    let response = h.request(json!({"input":[{"role":"user","content":"weather?"},call,{"type":"function_call_output","call_id":"call_1","output":"sunny"}],"stream":true,"tools":[{"type":"function","name":"weather","parameters":{"type":"object","properties":{"city":{"type":"string"}}}}]})).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(parse_sse(&response.text().await.unwrap()), expected);
    let received = h.fake.received.lock().unwrap();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].1["input"][2]["call_id"], "call_1");
    assert_eq!(received[0].1["input"][2]["output"], "sunny");
}

#[tokio::test]
async fn rejects_unsupported_features_before_contacting_upstream() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(10)).await;
    for body in [
        json!({"input":"hi","store":true}),
        json!({"input":"hi","previous_response_id":"resp_1"}),
        json!({"input":"hi","temperature":0.5}),
        json!({"input":"hi","tool_choice":"none"}),
        json!({"input":[{"type":"unknown"}]}),
    ] {
        let response = h.request(body).send().await.unwrap();
        assert!(response.status().is_client_error(), "{}", response.status());
        assert!(response.json::<Value>().await.unwrap()["error"].is_object());
    }
    assert!(h.fake.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn gateway_auth_failure_does_not_contact_upstream() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(10)).await;
    let response = reqwest::Client::new()
        .post(format!("{}/v1/responses", h.url))
        .json(&json!({"model":h.model,"input":"hi"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(h.fake.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn upstream_http_failure_is_not_reported_as_success_or_retried_indefinitely() {
    let h = Harness::new(
        vec![],
        StatusCode::UNAUTHORIZED,
        false,
        Duration::from_secs(10),
    )
    .await;
    let response = h.request(json!({"input":"hi"})).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(h.fake.received.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn failed_and_incomplete_terminal_events_keep_original_details() {
    for (kind, status) in [
        ("response.failed", "failed"),
        ("response.incomplete", "incomplete"),
    ] {
        let event = json!({"type":kind,"sequence_number":1,"response":{"id":"resp_1","object":"response","status":status,"error":{"code":"invalid_prompt","message":"bad prompt"},"incomplete_details":{"reason":"max_output_tokens"},"output":[]}});
        let h = Harness::new(
            vec![events()[0].clone(), event.clone()],
            StatusCode::OK,
            false,
            Duration::from_secs(10),
        )
        .await;
        let response = h
            .request(json!({"input":"hi","stream":true}))
            .send()
            .await
            .unwrap();
        assert_eq!(
            parse_sse(&response.text().await.unwrap()).last(),
            Some(&event)
        );
        let response = h.request(json!({"input":"hi"})).send().await.unwrap();
        assert_eq!(response.json::<Value>().await.unwrap(), event["response"]);
    }
}

#[tokio::test]
async fn truncated_stream_never_invents_a_successful_completion() {
    let h = Harness::new(
        vec![events()[0].clone()],
        StatusCode::OK,
        false,
        Duration::from_secs(10),
    )
    .await;
    let response = h
        .request(json!({"input":"hi","stream":true}))
        .send()
        .await
        .unwrap();
    let actual = parse_sse(&response.text().await.unwrap());
    assert_eq!(actual.last().unwrap()["type"], "error");
    let response = h.request(json!({"input":"hi"})).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn deadline_releases_concurrency_slot() {
    let h = Harness::new(
        vec![events()[0].clone()],
        StatusCode::OK,
        true,
        Duration::from_millis(250),
    )
    .await;
    let first = h
        .request(json!({"input":"hi","stream":true}))
        .send()
        .await
        .unwrap();
    let second = h.request(json!({"input":"hi"})).send().await.unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    let events = parse_sse(&first.text().await.unwrap());
    assert_eq!(events.last().unwrap()["type"], "error");
    let third = h.request(json!({"input":"hi"})).send().await.unwrap();
    assert_eq!(third.status(), StatusCode::GATEWAY_TIMEOUT);
}

#[tokio::test]
async fn client_disconnect_cancels_the_response_and_releases_the_slot() {
    let h = Harness::new(
        vec![events()[0].clone()],
        StatusCode::OK,
        true,
        Duration::from_secs(5),
    )
    .await;
    let mut response = h
        .request(json!({"input":"hi","stream":true}))
        .send()
        .await
        .unwrap();
    assert!(response.chunk().await.unwrap().is_some());
    drop(response);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let response = h
            .request(json!({"input":"hi","stream":true}))
            .send()
            .await
            .unwrap();
        if response.status() == StatusCode::OK {
            break;
        }
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(
            tokio::time::Instant::now() < deadline,
            "disconnect did not release concurrency slot"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
async fn subscription_uses_codex_account_auth_and_zstd_without_reading_real_credentials() {
    let h = Harness::with_auth(
        events(),
        StatusCode::OK,
        false,
        Duration::from_secs(10),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    )
    .await;
    let response = h
        .request(json!({"input":"subscription request"}))
        .header("ChatGPT-Account-ID", "untrusted-client")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<Value>().await.unwrap()["status"],
        "completed"
    );
    let received = h.fake.received.lock().unwrap();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].0["authorization"], "Bearer Access Token");
    assert_eq!(received[0].0["chatgpt-account-id"], "account_id");
    assert_eq!(received[0].0["content-encoding"], "zstd");
    assert_eq!(
        received[0].1["input"][0]["content"][0]["text"],
        "subscription request"
    );
}

#[tokio::test]
async fn native_preserves_full_body_headers_and_compressed_input() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let body = json!({"model":"future-codex-model", "stream":true, "store":false,
        "instructions":"original instructions", "input":[{"type":"custom_tool_call", "call_id":"call_1", "name":"apply_patch", "input":"patch"}],
        "tools":[{"type":"custom", "name":"apply_patch", "format":{"type":"text"}}],
        "client_metadata":{"trace":"keep"}, "future_field":{"nested":[1,2,3]}});
    let response = reqwest::Client::new()
        .post(format!("{}/codex/responses", h.url))
        .bearer_auth("client-key")
        .header("content-encoding", "zstd")
        .header("session-id", "original-session")
        .header("thread-id", "original-thread")
        .header("x-codex-turn-state", "previous-state")
        .header("chatgpt-account-id", "client-must-not-win")
        .body(zstd::encode_all(serde_json::to_vec(&body).unwrap().as_slice(), 3).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-codex-turn-state"], "upstream-state");
    assert_eq!(response.headers()["x-models-etag"], "catalog-1");
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("response.completed")
    );
    let received = h.fake.received.lock().unwrap();
    assert_eq!(received[0].1, body);
    let headers = &received[0].0;
    assert_eq!(headers["session-id"], "original-session");
    assert_eq!(headers["thread-id"], "original-thread");
    assert_eq!(headers["x-codex-turn-state"], "previous-state");
    assert_eq!(headers["authorization"], "Bearer upstream-secret");
    assert!(!headers.contains_key("chatgpt-account-id"));
}

#[tokio::test]
async fn native_rejects_unauthorized_and_nonstream_and_decompression_bombs() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let client = reqwest::Client::new();
    let url = format!("{}/codex/responses", h.url);
    assert_eq!(
        client
            .post(&url)
            .json(&json!({"stream":true}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .post(&url)
            .bearer_auth("client-key")
            .json(&json!({"model":"x","stream":false}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let bomb = zstd::encode_all(vec![b' '; 16 * 1024 * 1024 + 1].as_slice(), 1).unwrap();
    assert_eq!(
        client
            .post(&url)
            .bearer_auth("client-key")
            .header("content-encoding", "zstd")
            .body(bomb)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert!(h.fake.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn original_codex_client_can_consume_gateway_native_endpoint() {
    use futures::StreamExt;
    struct Key;
    impl codex_api::AuthProvider for Key {
        fn add_auth_headers(&self, headers: &mut HeaderMap) {
            headers.insert("authorization", "Bearer client-key".parse().unwrap());
        }
    }
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let provider = codex_api::Provider {
        name: "gateway".into(),
        base_url: format!("{}/codex", h.url),
        query_params: None,
        headers: HeaderMap::new(),
        retry: codex_api::RetryConfig {
            max_attempts: 1,
            base_delay: Duration::from_millis(1),
            retry_429: false,
            retry_5xx: false,
            retry_transport: false,
        },
        stream_idle_timeout: Duration::from_secs(5),
    };
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    let http = codex_login::default_client::create_client_for_route(
        &factory,
        &provider.url_for_path("/responses"),
        codex_http_client::ClientRouteClass::Api,
    )
    .unwrap();
    let client = codex_api::ResponsesClient::new(
        codex_api::ReqwestTransport::from_http_client(http),
        provider,
        Arc::new(Key),
    );
    let turn_state = Arc::new(std::sync::OnceLock::new());
    let mut stream = client
        .stream(
            json!({"model":h.model,"stream":true,"input":[],"tools":[],"instructions":"native"}),
            HeaderMap::new(),
            codex_api::Compression::None,
            Some(turn_state.clone()),
        )
        .await
        .unwrap();
    let mut completed = false;
    while let Some(event) = stream.next().await {
        if matches!(event.unwrap(), codex_api::ResponseEvent::Completed { .. }) {
            completed = true;
        }
    }
    assert!(completed);
    assert_eq!(turn_state.get().unwrap(), "upstream-state");
}

fn ws_provider(base_url: String) -> codex_api::Provider {
    let mut provider = ModelProviderInfo::create_openai_provider(Some(base_url))
        .to_api_provider(None)
        .unwrap();
    provider.retry.max_attempts = 1;
    provider.retry.retry_429 = false;
    provider.retry.retry_5xx = false;
    provider.retry.retry_transport = false;
    provider.stream_idle_timeout = Duration::from_secs(5);
    provider
}
struct TestKey(&'static str);
impl codex_api::AuthProvider for TestKey {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        headers.insert(
            "authorization",
            format!("Bearer {}", self.0).parse().unwrap(),
        );
    }
}

#[tokio::test]
async fn original_ws_client_direct_and_gateway_match_across_turns_and_compaction() {
    use codex_api::{
        ResponseCreateWsRequest, ResponseEvent, ResponsesWebsocketClient, ResponsesWsRequest,
    };
    use futures::StreamExt;
    let compaction =
        json!({"type":"compaction","id":"cmp_1","encrypted_content":"encrypted-compaction"});
    let h = Harness::new(
        vec![
            json!({"type":"response.output_item.done","output_index":0,"item":compaction}),
            completed(vec![compaction]),
        ],
        StatusCode::OK,
        false,
        Duration::from_secs(5),
    )
    .await;
    let models = codex_models_manager::bundled_models_response()
        .unwrap()
        .models;
    let model = models.iter().find(|m| m.slug == h.model).unwrap();
    let input: codex_api_gateway::request::CreateResponse = serde_json::from_value(json!({"model":h.model,"input":[{"type":"compaction_trigger"}],"instructions":"unchanged","stream":true})).unwrap();
    let body = input.into_codex(model, "same-thread").unwrap();
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    for endpoint in [
        h.upstream_url.clone(),
        format!("{}/codex", h.url),
        format!("{}/v1", h.url),
    ] {
        let direct = endpoint == h.upstream_url;
        let client = ResponsesWebsocketClient::new(
            ws_provider(endpoint),
            Arc::new(TestKey(if direct {
                "upstream-secret"
            } else {
                "client-key"
            })),
        );
        let mut headers = codex_api::build_session_headers(
            Some("same-session".into()),
            Some("same-thread".into()),
        );
        headers.insert(
            "openai-beta",
            "responses_websockets=2026-02-06".parse().unwrap(),
        );
        let state = Arc::new(std::sync::OnceLock::new());
        let connection = client
            .connect(
                &factory,
                headers,
                codex_login::default_client::default_headers(),
                Some(state.clone()),
                None,
            )
            .await
            .unwrap();
        assert_eq!(state.get().unwrap(), "ws-state");
        for turn in 0..3 {
            let mut request = ResponseCreateWsRequest::from(&body);
            request.generate = Some(turn != 0);
            request.previous_response_id = (turn > 0).then(|| "resp_1".to_owned());
            request.client_metadata = Some(std::collections::HashMap::from([(
                "ws_request_header_x_openai_internal_codex_responses_lite".into(),
                "true".into(),
            )]));
            let mut stream = connection
                .stream_request(
                    ResponsesWsRequest::ResponseCreate(request),
                    turn > 0,
                    Some(state.clone()),
                )
                .await
                .unwrap();
            let mut done = false;
            let mut compact = false;
            while let Some(event) = stream.next().await {
                match event.unwrap() {
                    ResponseEvent::Completed { .. } => done = true,
                    ResponseEvent::OutputItemDone(
                        codex_protocol::models::ResponseItem::Compaction { .. },
                    ) => compact = true,
                    _ => {}
                }
            }
            assert!(done && compact);
        }
        drop(connection);
        // The permit is held until the connection actually closes.
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let frames = h.fake.ws_frames.lock().unwrap();
    assert_eq!(frames.len(), 9);
    for i in 0..3 {
        assert_eq!(
            frames[i],
            frames[i + 3],
            "native request frame must be byte-identical"
        );
        assert_eq!(
            frames[i],
            frames[i + 6],
            "public WS request frame must be byte-identical"
        );
    }
    assert_eq!(
        h.fake
            .ws_connections
            .load(std::sync::atomic::Ordering::SeqCst),
        3
    );
    let directory = h.capture.save("websocket-parity");
    std::fs::write(directory.join("assertions.json"),serde_json::to_vec_pretty(&json!({"codex_revision":codex_api_gateway::CODEX_REV,"comparison":"direct vs /codex vs /v1","request_frames_byte_identical":true,"turns_per_connection":3,"connections":3,"features":["warmup","previous_response_id","lite metadata","remote compaction v2"],"frames":&*frames})).unwrap()).unwrap();
    let received = h.fake.received.lock().unwrap();
    for (headers, _) in received.iter() {
        assert_eq!(headers["authorization"], "Bearer upstream-secret");
        assert_eq!(headers["session-id"], "same-session");
    }
}

#[tokio::test]
async fn websocket_preserves_unknown_events_binary_errors_and_rejects_bad_auth() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
    let events = vec![
        json!({"type":"future.event","payload":{"unchanged":true}}),
        json!({"type":"error","error":{"code":"previous_response_not_found","message":"unchanged"}}),
    ];
    let h = Harness::new(
        events.clone(),
        StatusCode::OK,
        false,
        Duration::from_secs(5),
    )
    .await;
    for path in ["codex", "v1"] {
        let url = format!("{}/{path}/responses", h.url).replacen("http:", "ws:", 1);
        let mut request = url.into_client_request().unwrap();
        request
            .headers_mut()
            .insert("authorization", "Bearer bad-key".parse().unwrap());
        let err = tokio_tungstenite::connect_async(request.clone())
            .await
            .unwrap_err();
        assert!(
            matches!(err,tokio_tungstenite::tungstenite::Error::Http(ref r) if r.status()==StatusCode::UNAUTHORIZED)
        );
        request
            .headers_mut()
            .insert("authorization", "Bearer client-key".parse().unwrap());
        let (mut ws, headers) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert_eq!(headers.headers()["x-codex-turn-state"], "ws-state");
        ws.send(Message::Text(
            "{ \"type\": \"response.create\", \"model\": \"future\", \"stream\": true }".into(),
        ))
        .await
        .unwrap();
        for event in &events {
            assert_eq!(
                ws.next()
                    .await
                    .unwrap()
                    .unwrap()
                    .into_text()
                    .unwrap()
                    .as_str(),
                event.to_string()
            );
        }
        ws.send(Message::Binary(vec![0, 1, 255, 128].into()))
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap(),
            Message::Binary(vec![0, 1, 255, 128].into())
        );
        ws.close(None).await.unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}

#[tokio::test]
async fn lite_http_and_both_compact_endpoints_return_original_output() {
    let compaction =
        json!({"type":"compaction","id":"cmp_1","encrypted_content":"encrypted-compaction"});
    let h = Harness::new(
        vec![completed(vec![compaction.clone()])],
        StatusCode::OK,
        false,
        Duration::from_secs(5),
    )
    .await;
    let models = codex_models_manager::bundled_models_response()
        .unwrap()
        .models;
    let lite = models
        .iter()
        .find(|m| m.use_responses_lite)
        .expect("pinned catalog must have a Lite model");
    let client = reqwest::Client::new();
    let response=client.post(format!("{}/v1/responses",h.url)).bearer_auth("client-key").json(&json!({"model":lite.slug,"input":"hello","instructions":"original instruction","stream":false,"tools":[{"type":"function","name":"test","parameters":{"type":"object","properties":{}}}]})).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<Value>().await.unwrap()["output"][0],
        compaction
    );
    {
        let received = h.fake.received.lock().unwrap();
        assert_eq!(
            received[0].0["x-openai-internal-codex-responses-lite"],
            "true"
        );
        assert!(
            received[0].1.get("instructions").is_none(),
            "original Codex serialization omits empty instructions"
        );
        assert!(received[0].1.get("tools").is_none());
        assert_eq!(received[0].1["input"][0]["type"], "additional_tools");
        assert_eq!(received[0].1["input"][1]["role"], "developer");
        assert_eq!(received[0].1["parallel_tool_calls"], false);
        assert_eq!(received[0].1["reasoning"]["context"], "all_turns");
    }
    for path in ["v1", "codex"] {
        let response = client
            .post(format!("{}/{path}/responses/compact", h.url))
            .bearer_auth("client-key")
            .json(&json!({"model":h.model,"input":[{"role":"user","content":"compress"}]}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["object"], "response.compaction");
        assert_eq!(body["output"][0], compaction);
        let received = h.fake.received.lock().unwrap();
        assert_eq!(
            received.last().unwrap().1["input"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["type"],
            "compaction_trigger"
        );
    }
}

#[tokio::test]
async fn captured_http_bodies_match_original_codex_including_zstd() {
    use codex_api::{Compression, ReqwestTransport, ResponsesClient, ResponsesOptions};
    use futures::StreamExt;
    let h = Harness::with_auth(
        events(),
        StatusCode::OK,
        false,
        Duration::from_secs(5),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    )
    .await;
    let catalog = codex_models_manager::bundled_models_response().unwrap();
    let model = catalog.models.iter().find(|m| m.slug == h.model).unwrap();
    let public =
        json!({"model":model.slug,"input":"你好","instructions":"Exact input","stream":true});
    let input: codex_api_gateway::request::CreateResponse =
        serde_json::from_value(public.clone()).unwrap();
    let request = input.into_codex(model, "fixed-thread").unwrap();
    let provider = ws_provider(h.upstream_url.clone());
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    let http = codex_login::default_client::create_client_for_route(
        &factory,
        &provider.url_for_path("/responses"),
        codex_http_client::ClientRouteClass::Api,
    )
    .unwrap();
    let direct = ResponsesClient::new(
        ReqwestTransport::from_http_client(http),
        provider,
        Arc::new(TestKey("Access Token")),
    );
    let mut extra = HeaderMap::new();
    extra.insert(
        "x-codex-routing-hint",
        format!("model={}", model.slug).parse().unwrap(),
    );
    extra.insert("chatgpt-account-id", "account_id".parse().unwrap());
    let options = ResponsesOptions {
        session_id: Some("fixed-thread".into()),
        thread_id: Some("fixed-thread".into()),
        extra_headers: extra,
        compression: Compression::Zstd,
        ..Default::default()
    };
    let mut stream = direct
        .stream_request(request.clone(), options)
        .await
        .unwrap();
    while let Some(event) = stream.next().await {
        event.unwrap();
    }
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/codex/responses", h.url))
        .bearer_auth("client-key")
        .header("thread-id", "fixed-thread")
        .header("session-id", "fixed-thread")
        .header("x-client-request-id", "fixed-thread")
        .header("x-codex-routing-hint", format!("model={}", model.slug))
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("response.completed")
    );
    let response = client
        .post(format!("{}/v1/responses", h.url))
        .bearer_auth("client-key")
        .header("thread-id", "fixed-thread")
        .json(&public)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("response.completed")
    );
    let bodies = h.fake.http_bodies.lock().unwrap();
    assert_eq!(bodies.len(), 3);
    assert_eq!(
        bodies[0], bodies[1],
        "native Zstd wire body must match original Codex bytes"
    );
    assert_eq!(
        bodies[0], bodies[2],
        "public Zstd wire body must match original Codex bytes"
    );
    let received = h.fake.received.lock().unwrap();
    for index in [1, 2] {
        assert_eq!(
            received[0].0, received[index].0,
            "all HTTP request headers must match the original provider"
        );
    }
    let directory = h.capture.save("http-parity");
    std::fs::write(directory.join("assertions.json"),serde_json::to_vec_pretty(&json!({"codex_revision":codex_api_gateway::CODEX_REV,"request_json_byte_identical":true,"compressed_request_byte_identical":true,"request_headers_identical":true,"compared_paths":["direct","codex","v1"],"body":serde_json::from_slice::<Value>(&zstd::decode_all(bodies[0].as_slice()).unwrap()).unwrap()})).unwrap()).unwrap();
}

#[tokio::test]
async fn websocket_handshake_failure_preserves_http_status_for_fallback() {
    use tokio_tungstenite::tungstenite::{Error, client::IntoClientRequest};
    for status in [
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::UPGRADE_REQUIRED,
    ] {
        let h = Harness::new(vec![], status, false, Duration::from_secs(5)).await;
        let mut request = format!("{}/codex/responses", h.url)
            .replacen("http:", "ws:", 1)
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("authorization", "Bearer client-key".parse().unwrap());
        let error = tokio_tungstenite::connect_async(request).await.unwrap_err();
        assert!(matches!(error,Error::Http(ref response) if response.status()==status));
    }
}

#[tokio::test]
async fn public_http_and_ws_assemble_items_missing_from_terminal_output() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
    let mut fixture = events();
    *fixture.last_mut().unwrap() = completed(vec![]);
    let h = Harness::new(fixture, StatusCode::OK, false, Duration::from_secs(5)).await;
    let response = h
        .request(json!({"input":"hello","stream":false}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.json::<Value>().await.unwrap()["output"],
        json!([message()])
    );
    let response = h
        .request(json!({"input":"hello","stream":true}))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let terminal = response
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|v| v["type"] == "response.completed")
        .unwrap();
    assert_eq!(terminal["response"]["output"], json!([message()]));
    for path in ["v1", "codex"] {
        let mut request = format!("{}/{path}/responses", h.url)
            .replacen("http:", "ws:", 1)
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("authorization", "Bearer client-key".parse().unwrap());
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        socket
            .send(Message::Text(
                json!({"type":"response.create","model":h.model,"stream":true})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        loop {
            let event: Value =
                serde_json::from_str(&socket.next().await.unwrap().unwrap().into_text().unwrap())
                    .unwrap();
            if event["type"] == "response.completed" {
                assert_eq!(
                    event["response"]["output"],
                    if path == "v1" {
                        json!([message()])
                    } else {
                        json!([])
                    }
                );
                break;
            }
        }
        socket.close(None).await.unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}

#[tokio::test]
#[ignore = "requires the pinned release binary; run scripts/fetch-codex-baseline.py"]
async fn actual_codex_cli_can_use_gateway_http_and_websocket() {
    let binary = std::env::var("CODEX_CLI_BIN").unwrap_or_else(|_| {
        format!(
            "{}/.cache/codex-baseline/rust-v{}/bin/codex-x86_64-unknown-linux-musl",
            env!("CARGO_MANIFEST_DIR"),
            codex_api_gateway::CODEX_RELEASE
        )
    });
    let version = tokio::process::Command::new(&binary)
        .arg("--version")
        .output()
        .await
        .unwrap();
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        format!("codex-cli {}", codex_api_gateway::CODEX_RELEASE),
        "CLI must match the pinned implementation"
    );
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(20)).await;
    let directory = tempfile::tempdir().unwrap();
    for ws in [false, true] {
        let home = tempfile::tempdir().unwrap();
        let config = format!(
            r#"model = "gpt-5.5"
model_provider = "gateway"
model_reasoning_effort = "low"
[model_providers.gateway]
name = "OpenAI"
base_url = "{}/backend-api/codex"
wire_api = "responses"
env_key = "CODEX_GATEWAY_API_KEY"
requires_openai_auth = false
supports_websockets = {}
"#,
            h.url, ws
        );
        std::fs::write(home.path().join("config.toml"), config).unwrap();
        let output = tokio::time::timeout(
            Duration::from_secs(60),
            tokio::process::Command::new(&binary)
                .env("CODEX_HOME", home.path())
                .env("CODEX_GATEWAY_API_KEY", "client-key")
                .env_remove("OPENAI_API_KEY")
                .env_remove("OPENAI_BASE_URL")
                .current_dir(directory.path())
                .args([
                    "exec",
                    "--skip-git-repo-check",
                    "--sandbox",
                    "read-only",
                    "--json",
                    "Reply briefly. Do not use tools.",
                ])
                .kill_on_drop(true)
                .output(),
        )
        .await
        .expect("CLI timeout")
        .unwrap();
        assert!(
            output.status.success(),
            "CLI failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("你好"),
            "CLI did not receive model text: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    assert!(!h.fake.http_bodies.lock().unwrap().is_empty());
    assert!(
        h.fake
            .ws_connections
            .load(std::sync::atomic::Ordering::SeqCst)
            > 0,
        "CLI did not exercise WebSocket"
    );
    let directory = h.capture.save("codex-cli-e2e");
    std::fs::write(directory.join("assertions.json"),serde_json::to_vec_pretty(&json!({"codex_revision":codex_api_gateway::CODEX_REV,"cli_version":String::from_utf8_lossy(&version.stdout).trim(),"http":true,"websocket":true,"received_text":true})).unwrap()).unwrap();
}

#[tokio::test]
async fn public_ws_accepts_sdk_input_and_builds_lite_request() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let catalog = codex_models_manager::bundled_models_response().unwrap();
    let lite = catalog
        .models
        .iter()
        .find(|m| m.use_responses_lite)
        .unwrap();
    let mut request = format!("{}/v1/responses", h.url)
        .replacen("http:", "ws:", 1)
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("authorization", "Bearer client-key".parse().unwrap());
    let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    socket.send(Message::Text(json!({"type":"response.create","model":lite.slug,"input":"hello","instructions":"brief","generate":false,"stream_id":"lane-1"}).to_string().into())).await.unwrap();
    loop {
        let message = socket.next().await.unwrap().unwrap();
        let value: Value = serde_json::from_str(&message.into_text().unwrap()).unwrap();
        if value["type"] == "response.completed" {
            break;
        }
    }
    let received = h.fake.received.lock().unwrap();
    let body = &received[0].1;
    assert_eq!(body["input"][0]["type"], "additional_tools");
    assert_eq!(body["reasoning"]["context"], "all_turns");
    assert_eq!(body["generate"], false);
    assert_eq!(body["stream_id"], "lane-1");
    assert_eq!(
        body["client_metadata"]["ws_request_header_x_openai_internal_codex_responses_lite"],
        "true"
    );
}

#[tokio::test]
async fn native_model_catalog_is_fetched_through_original_models_client() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let client = reqwest::Client::new();
    for path in ["codex", "backend-api/codex"] {
        let response = client
            .get(format!("{}/{path}/models?client_version=0.155.0", h.url))
            .bearer_auth("client-key")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = response.json().await.unwrap();
        assert_eq!(
            body["models"][0]["slug"],
            codex_models_manager::bundled_models_response()
                .unwrap()
                .models[0]
                .slug
        );
    }
    let directory = h.capture.save("model-catalog");
    let first = std::fs::read(directory.join("0-request.tcp")).unwrap();
    assert!(first.starts_with(b"GET /models?client_version=0.155.0 HTTP/1.1\r\n"));
}

#[path = "cases/tools.rs"]
mod tool_cases;
use tool_cases::{upstream_image, upstream_search};
