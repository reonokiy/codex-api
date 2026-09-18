use super::*;

const IMAGE_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg==";
fn image_response() -> Value {
    json!({"created":1778832973u64,"data":[{"b64_json":IMAGE_PNG,"generation_id":"gen-test","future_item":42}],"output_format":"png","usage":{"total_tokens":12},"future_response":{"preserved":true}})
}
pub(super) async fn upstream_image(
    State(fake): State<Fake>,
    uri: axum::extract::OriginalUri,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    fake.image_paths
        .lock()
        .unwrap()
        .push(uri.path().to_string());
    fake.http_bodies.lock().unwrap().push(bytes.to_vec());
    fake.received
        .lock()
        .unwrap()
        .push((headers, serde_json::from_slice(&bytes).unwrap()));
    if fake.hang {
        std::future::pending::<()>().await;
    }
    if fake.status != StatusCode::OK {
        return Response::builder()
            .status(fake.status)
            .header("content-type", "application/json")
            .header("retry-after", "7")
            .body(Body::from(
                r#"{"error":{"message":"image quota exceeded","code":"quota"}}"#,
            ))
            .unwrap();
    }
    Response::builder()
        .header("content-type", "application/json")
        .header("x-codex-imagegen-request-id", "image-request-123")
        .body(Body::from(image_response().to_string()))
        .unwrap()
}

#[tokio::test]
async fn images_match_original_client_and_preserve_complete_responses() {
    let h = Harness::with_auth(
        events(),
        StatusCode::OK,
        false,
        Duration::from_secs(5),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    )
    .await;
    let provider = ws_provider(h.upstream_url.clone());
    let http = codex_login::default_client::create_client_for_route(
        &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        &provider.url_for_path("images/generations"),
        codex_http_client::ClientRouteClass::Api,
    )
    .unwrap();
    let original = codex_api::ImagesClient::new(
        codex_api::ReqwestTransport::from_http_client(http),
        provider,
        Arc::new(TestKey("Access Token")),
    );
    let generation = codex_api::ImageGenerationRequest {
        prompt: "Generate a tiny square".into(),
        model: "gpt-image-2".into(),
        background: Some(codex_api::ImageBackground::Auto),
        quality: Some(codex_api::ImageQuality::Auto),
        size: Some("auto".into()),
        n: None,
    };
    let edit = codex_api::ImageEditRequest {
        images: vec![codex_api::ImageUrl {
            image_url: format!("data:image/png;base64,{IMAGE_PNG}"),
        }],
        prompt: generation.prompt.clone(),
        model: generation.model.clone(),
        background: generation.background,
        quality: generation.quality,
        size: generation.size.clone(),
        n: None,
    };
    let mut direct_headers = HeaderMap::new();
    direct_headers.insert("chatgpt-account-id", "account_id".parse().unwrap());
    direct_headers.insert("x-codex-image-turn-id", "fixed-image-turn".parse().unwrap());
    for editing in [false, true] {
        h.fake.http_bodies.lock().unwrap().clear();
        h.fake.received.lock().unwrap().clear();
        let (body, path) = if editing {
            original.edit(&edit, direct_headers.clone()).await.unwrap();
            (serde_json::to_value(&edit).unwrap(), "edits")
        } else {
            original
                .generate(&generation, direct_headers.clone())
                .await
                .unwrap();
            (serde_json::to_value(&generation).unwrap(), "generations")
        };
        for prefix in ["v1", "codex", "backend-api/codex"] {
            let request = reqwest::Client::new()
                .post(format!("{}/{prefix}/images/{path}", h.url))
                .bearer_auth("client-key")
                .header("x-codex-image-turn-id", "fixed-image-turn")
                .header("x-openai-actor-authorization", "gateway")
                .header("chatgpt-account-id", "client-account-must-not-leak")
                .json(&body);
            let request = if prefix == "v1" {
                request
                    .header("user-agent", "OpenAI/Python SDK-test")
                    .header("originator", "sdk-must-not-leak")
            } else {
                request
            };
            let response = request.send().await.unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(
                response.headers()["x-codex-imagegen-request-id"],
                "image-request-123"
            );
            assert_eq!(response.json::<Value>().await.unwrap(), image_response());
        }
        let bodies = h.fake.http_bodies.lock().unwrap();
        assert_eq!(bodies.len(), 4);
        assert!(bodies.iter().all(|body| body == &bodies[0]));
        let requests = h.fake.received.lock().unwrap();
        assert!(requests.iter().all(|r| r.0 == requests[0].0));
    }
    let directory = h.capture.save("images-parity");
    std::fs::write(directory.join("assertions.json"),serde_json::to_vec_pretty(&json!({"codex_revision":codex_api_gateway::CODEX_REV,"generation_and_edit":true,"request_headers_identical":true,"request_bodies_byte_identical":true,"extended_response_preserved":true})).unwrap()).unwrap();
}

#[tokio::test]
async fn images_accept_sdk_multipart_without_touching_the_filesystem() {
    use base64::Engine;
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let png = base64::engine::general_purpose::STANDARD
        .decode(IMAGE_PNG)
        .unwrap();
    let form = reqwest::multipart::Form::new()
        .text("model", "gpt-image-2")
        .text("prompt", "Make it blue")
        .text("n", "1")
        .text("response_format", "b64_json")
        .part(
            "image",
            reqwest::multipart::Part::bytes(png)
                .file_name("../never-write.png")
                .mime_str("image/png")
                .unwrap(),
        );
    let response = reqwest::Client::new()
        .post(format!("{}/v1/images/edits", h.url))
        .bearer_auth("client-key")
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.json::<Value>().await.unwrap(), image_response());
    let requests = h.fake.received.lock().unwrap();
    let body = &requests[0].1;
    assert_eq!(
        body["images"][0]["image_url"],
        format!("data:image/png;base64,{IMAGE_PNG}")
    );
    assert_eq!(body["n"], 1);
    assert!(body.get("response_format").is_none());
    assert_eq!(h.fake.image_paths.lock().unwrap()[0], "/images/edits");
}

#[tokio::test]
async fn images_reject_invalid_requests_and_unauthorized_clients_before_upstream() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let client = reqwest::Client::new();
    for (path, body) in [
        ("generations", json!({"prompt":"x","response_format":"url"})),
        ("generations", json!({"prompt":"x","output_format":"jpeg"})),
        ("generations", json!({"prompt":"x","stream":true})),
        ("generations", json!({"prompt":"x","n":0})),
        ("edits", json!({"prompt":"x","images":[]})),
        (
            "edits",
            json!({"prompt":"x","images":[{"image_url":"file:///etc/passwd"}]}),
        ),
        ("edits", json!({"prompt":"x","mask":"unsupported"})),
    ] {
        let r = client
            .post(format!("{}/v1/images/{path}", h.url))
            .bearer_auth("client-key")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 400, "{body}");
    }
    let r = client
        .post(format!("{}/v1/images/generations", h.url))
        .json(&json!({"prompt":"x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    assert!(h.fake.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn images_preserve_upstream_failures_and_release_slots_after_timeout() {
    let h = Harness::new(
        events(),
        StatusCode::BAD_REQUEST,
        false,
        Duration::from_secs(5),
    )
    .await;
    let r = reqwest::Client::new()
        .post(format!("{}/v1/images/generations", h.url))
        .bearer_auth("client-key")
        .json(&json!({"prompt":"x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    assert_eq!(r.headers()["retry-after"], "7");
    assert_eq!(r.json::<Value>().await.unwrap()["error"]["code"], "quota");
    let h = Harness::new(events(), StatusCode::OK, true, Duration::from_millis(80)).await;
    for _ in 0..2 {
        let r = reqwest::Client::new()
            .post(format!("{}/v1/images/generations", h.url))
            .bearer_auth("client-key")
            .json(&json!({"prompt":"x"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 504);
    }
}

#[tokio::test]
#[ignore = "requires pinned official CLI; uses a local fake image upstream"]
async fn actual_codex_cli_images_and_standalone_web_search_through_gateway() {
    use base64::Engine;
    let binary = format!(
        "{}/.cache/codex-baseline/rust-v{}/bin/codex-x86_64-unknown-linux-musl",
        env!("CARGO_MANIFEST_DIR"),
        codex_api_gateway::CODEX_RELEASE
    );
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(20)).await;
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let input = work.path().join("input.png");
    std::fs::write(
        &input,
        base64::engine::general_purpose::STANDARD
            .decode(IMAGE_PNG)
            .unwrap(),
    )
    .unwrap();
    *h.fake.cli_image_path.lock().unwrap() = Some(input.to_string_lossy().into_owned());
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            r#"model = "gpt-5.5"
model_provider = "gateway"
model_reasoning_effort = "low"
[features]
image_generation = true
standalone_web_search = true
[model_providers.gateway]
name = "OpenAI"
base_url = "{}/backend-api/codex"
wire_api = "responses"
env_key = "CODEX_GATEWAY_API_KEY"
requires_openai_auth = false
supports_websockets = false
http_headers = {{ "x-openai-actor-authorization" = "gateway" }}
"#,
            h.url
        ),
    )
    .unwrap();
    let output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new(binary)
            .env("CODEX_HOME", home.path())
            .env("CODEX_GATEWAY_API_KEY", "client-key")
            .env_remove("OPENAI_API_KEY")
            .env_remove("OPENAI_BASE_URL")
            .current_dir(work.path())
            .args([
                "exec",
                "--skip-git-repo-check",
                "--sandbox",
                "danger-full-access",
                "--json",
                "Generate an image, then edit the provided input image. Use image_gen.imagegen.",
            ])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("CLI timeout")
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        *h.fake.image_paths.lock().unwrap(),
        vec!["/images/generations", "/images/edits"],
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let received = h.fake.received.lock().unwrap();
    assert!(received[0].1["tools"].to_string().contains("image_gen"));
    assert!(
        received
            .iter()
            .any(|(_, v)| v["commands"]["search_query"].is_array()),
        "CLI did not call standalone search"
    );
    let directory = h.capture.save("images-cli-e2e");
    std::fs::write(directory.join("assertions.json"),serde_json::to_vec_pretty(&json!({"codex_revision":codex_api_gateway::CODEX_REV,"cli_version":codex_api_gateway::CODEX_RELEASE,"generation":true,"edit":true,"caller_subscription_credentials":false})).unwrap()).unwrap();
}

#[tokio::test]
async fn web_search_preserves_sources_citations_and_stream_events() {
    let search = json!({"type":"web_search_call","id":"ws_1","status":"completed","action":{"type":"search","query":"Codex","sources":[{"type":"url","url":"https://openai.com/"}]},"results":[{"type":"image_result","image_url":"https://openai.com/image.png"}]});
    let mut answer = message();
    answer["content"][0]["annotations"] = json!([{"type":"url_citation","url":"https://openai.com/","title":"OpenAI","start_index":0,"end_index":2}]);
    let upstream = vec![
        json!({"type":"response.web_search_call.searching","item_id":"ws_1","output_index":0}),
        json!({"type":"response.output_item.done","output_index":0,"item":search}),
        json!({"type":"response.output_item.done","output_index":1,"item":answer}),
        completed(vec![]),
    ];
    let h = Harness::new(upstream, StatusCode::OK, false, Duration::from_secs(5)).await;
    let tool = json!({"type":"web_search","external_web_access":false,"indexed_web_access":true,"filters":{"allowed_domains":["openai.com"]},"user_location":{"type":"approximate","country":"US"},"search_context_size":"high","search_content_types":["text","image"]});
    for stream in [false, true] {
        let response = h.request(json!({"input":"Search Codex","tools":[tool],"include":["web_search_call.action.sources","web_search_call.results"],"stream":stream})).send().await.unwrap();
        assert_eq!(response.status(), 200);
        if stream {
            let body = response.text().await.unwrap();
            assert!(body.contains("response.web_search_call.searching"));
            assert!(body.contains("url_citation"));
            assert!(body.contains("image_result"));
        } else {
            let body = response.json::<Value>().await.unwrap();
            assert_eq!(body["output"], json!([search, answer]));
        }
        let received = h.fake.received.lock().unwrap();
        let request = &received.last().unwrap().1;
        assert_eq!(request["tools"], json!([tool]));
        assert!(
            request["include"]
                .as_array()
                .unwrap()
                .contains(&json!("web_search_call.action.sources"))
        );
    }
    // A subsequent turn can carry a native web search item in its history.
    let response = h.request(json!({"input":[search,answer,{"role":"user","content":"Continue"}],"tools":[{"type":"web_search"}]})).send().await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn tools_use_original_lite_serializer_and_reject_unavailable_hosted_services() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let tools = json!([
        {"type":"web_search","external_web_access":true},
        {"type":"namespace","name":"local","tools":[{"type":"function","name":"read","parameters":{"type":"object","properties":{}}}]},
        {"type":"custom","name":"patch","format":{"type":"grammar","syntax":"regex","definition":".+"}}
    ]);
    let lite = codex_models_manager::bundled_models_response()
        .unwrap()
        .models
        .into_iter()
        .find(|m| m.use_responses_lite)
        .unwrap();
    let response = reqwest::Client::new()
        .post(format!("{}/v1/responses", h.url))
        .bearer_auth("client-key")
        .json(&json!({"model":lite.slug,"input":"Search","tools":tools}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    {
        let received = h.fake.received.lock().unwrap();
        let encoded = &received.last().unwrap().1["input"][0]["tools"];
        assert_eq!(encoded[0], tools[0]);
        assert_eq!(encoded[1]["type"], "namespace");
        assert!(encoded.to_string().contains("regex"));
    }
    for tool in [
        json!({"type":"file_search","vector_store_ids":["vs_1"]}),
        json!({"type":"code_interpreter","container":{"type":"auto"}}),
        json!({"type":"image_generation"}),
        json!({"type":"mcp","server_url":"https://example.com"}),
        json!({"type":"web_search","return_token_budget":"unlimited"}),
        json!({"type":"web_search","filters":{"blocked_domains":["example.com"]}}),
        json!({"type":"web_search","search_content_types":["video"]}),
        json!({"type":"namespace","name":"bad","tools":[{"type":"web_search"}]}),
    ] {
        let response = h
            .request(json!({"input":"x","tools":[tool]}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
    }
    assert_eq!(h.fake.received.lock().unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "requires the openai Python SDK in .cache/python-sdk; uses a local fake upstream"]
async fn actual_openai_python_sdk_images_and_web_search() {
    let search = json!({"type":"web_search_call","id":"ws_1","status":"completed","action":{"type":"search","query":"Codex"}});
    let h = Harness::new(vec![
        json!({"type":"response.web_search_call.searching","item_id":"ws_1","output_index":0,"sequence_number":0}),
        json!({"type":"response.output_item.done","output_index":0,"item":search,"sequence_number":1}),
        json!({"type":"response.output_item.done","output_index":1,"item":message(),"sequence_number":2}),
        completed(vec![]),
    ], StatusCode::OK, false, Duration::from_secs(10)).await;
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        tokio::process::Command::new("/usr/bin/python3")
            .env(
                "PYTHONPATH",
                format!("{}/.cache/python-sdk", env!("CARGO_MANIFEST_DIR")),
            )
            .args([
                "-c",
                include_str!("../support/openai_sdk.py"),
                &h.url,
                &h.model,
            ])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        *h.fake.image_paths.lock().unwrap(),
        vec!["/images/generations", "/images/edits"]
    );
    assert!(
        h.fake.received.lock().unwrap().last().unwrap().1["tools"]
            .to_string()
            .contains("web_search")
    );
    let received = h.fake.received.lock().unwrap();
    for (headers, body) in received.iter().take(2) {
        assert!(uuid::Uuid::parse_str(headers["x-codex-image-turn-id"].to_str().unwrap()).is_ok());
        assert_eq!(body["background"], "auto");
        assert_eq!(body["quality"], "auto");
        assert_eq!(body["size"], "auto");
        assert!(
            !headers["user-agent"]
                .to_str()
                .unwrap()
                .contains("OpenAI/Python")
        );
    }
}

fn search_response() -> Value {
    json!({"encrypted_output":"encrypted-search","output":"Codex is an OpenAI tool.","results":[{"type":"text_result","ref_id":"turn0search0","url":"https://openai.com/codex/","future_result":42}],"future_response":true})
}
pub(super) async fn upstream_search(
    State(fake): State<Fake>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    fake.http_bodies.lock().unwrap().push(bytes.to_vec());
    fake.received
        .lock()
        .unwrap()
        .push((headers, serde_json::from_slice(&bytes).unwrap()));
    if fake.hang {
        std::future::pending::<()>().await;
    }
    if fake.status != StatusCode::OK {
        return Response::builder()
            .status(fake.status)
            .header("retry-after", "7")
            .body(Body::from(
                r#"{"error":{"code":"quota","message":"search quota"}}"#,
            ))
            .unwrap();
    }
    Response::builder()
        .header("content-type", "application/json")
        .body(Body::from(search_response().to_string()))
        .unwrap()
}

#[tokio::test]
async fn standalone_search_matches_original_client_and_preserves_complete_output() {
    let h = Harness::with_auth(
        events(),
        StatusCode::OK,
        false,
        Duration::from_secs(5),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    )
    .await;
    let provider = ws_provider(h.upstream_url.clone());
    let client = codex_api::SearchClient::new(
        codex_api::ReqwestTransport::from_http_client(codex_login::default_client::create_client()),
        provider,
        Arc::new(TestKey("Access Token")),
    );
    let request = codex_api::SearchRequest {
        id:"search-session".into(), model:h.model.clone(), reasoning:None,
        input:Some(codex_api::SearchInput::Text("Find Codex".into())),
        commands:Some(serde_json::from_value(json!({
            "search_query":[{"q":"Codex","domains":["openai.com"]}],
            "image_query":[{"q":"Codex logo"}], "open":[{"ref_id":"https://openai.com/codex/"}],
            "click":[{"ref_id":"turn0search0","id":1}],"find":[{"ref_id":"turn0search0","pattern":"Codex"}],
            "screenshot":[{"ref_id":"turn0view0","pageno":0}],
            "weather":[{"location":"Berlin"}],"finance":[{"ticker":"MSFT","type":"equity","market":"USA"}],
            "sports":[{"fn":"standings","league":"nba"}],"time":[{"utc_offset":"+02:00"}],"response_length":"short"
        })).unwrap()), settings:Some(codex_api::SearchSettings::default()),max_output_tokens:Some(4096),
    };
    let mut headers = HeaderMap::new();
    headers.insert("chatgpt-account-id", "account_id".parse().unwrap());
    headers.insert(
        "x-codex-turn-metadata",
        "fixed-turn-metadata".parse().unwrap(),
    );
    client.search(&request, headers).await.unwrap();
    for prefix in ["codex", "backend-api/codex"] {
        let response = reqwest::Client::new()
            .post(format!("{}/{prefix}/alpha/search", h.url))
            .bearer_auth("client-key")
            .header("x-codex-turn-metadata", "fixed-turn-metadata")
            .json(&request)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.json::<Value>().await.unwrap(), search_response());
    }
    let received = h.fake.received.lock().unwrap();
    assert_eq!(received.len(), 3);
    assert!(received.iter().all(|r| r.0 == received[0].0));
    let bodies = h.fake.http_bodies.lock().unwrap();
    assert!(bodies.iter().all(|r| r == &bodies[0]));
}

#[tokio::test]
async fn standalone_search_rejects_unknown_fields_and_preserves_failures() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let client = reqwest::Client::new();
    for body in [
        json!({"id":"x","model":"x"}),
        json!({"id":"x","model":"x","commands":{"unknown":true}}),
        json!({"id":"x","model":"x","commands":{"search_query":[{"q":"x","unknown":true}]}}),
        json!({"id":"x","model":"x","commands":{},"settings":{"cookies":"must-not-be-forwarded"}}),
    ] {
        let response = client
            .post(format!("{}/codex/alpha/search", h.url))
            .bearer_auth("client-key")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
    }
    let response = client
        .post(format!("{}/codex/alpha/search", h.url))
        .body("invalid JSON")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    assert!(h.fake.received.lock().unwrap().is_empty());
    let h = Harness::new(
        events(),
        StatusCode::TOO_MANY_REQUESTS,
        false,
        Duration::from_secs(5),
    )
    .await;
    let response = client
        .post(format!("{}/codex/alpha/search", h.url))
        .bearer_auth("client-key")
        .json(&json!({"id":"x","model":"x","commands":{"time":[{"utc_offset":"+00:00"}]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429);
    assert_eq!(response.headers()["retry-after"], "7");
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "quota"
    );
}

#[tokio::test]
async fn image_uploads_enforce_body_limit_and_reject_duplicate_fields() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let client = reqwest::Client::new();
    let oversized = reqwest::multipart::Form::new().text("prompt", "x").part(
        "image",
        reqwest::multipart::Part::bytes(vec![0; 16 * 1024 * 1024 + 1]).file_name("large.png"),
    );
    let response = client
        .post(format!("{}/v1/images/edits", h.url))
        .bearer_auth("client-key")
        .multipart(oversized)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
    for form in [
        reqwest::multipart::Form::new()
            .text("prompt", "x")
            .text("prompt", "y"),
        reqwest::multipart::Form::new().text("mask", "unsupported"),
        reqwest::multipart::Form::new().part(
            "image",
            reqwest::multipart::Part::bytes(b"not PNG".to_vec()).file_name("fake.png"),
        ),
    ] {
        let response = client
            .post(format!("{}/v1/images/edits", h.url))
            .bearer_auth("client-key")
            .multipart(form)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
    }
    assert!(h.fake.received.lock().unwrap().is_empty());
}
