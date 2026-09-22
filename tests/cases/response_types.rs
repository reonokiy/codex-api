use super::*;

#[tokio::test]
#[ignore = "requires uv sync --locked; uses a local fake upstream"]
async fn actual_openai_sdk_response_types() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let catalog = codex_models_manager::bundled_models_response().unwrap();
    let lite = catalog
        .models
        .iter()
        .find(|m| m.use_responses_lite)
        .unwrap();
    let output = tokio::time::timeout(
        Duration::from_secs(40),
        support::python::command()
            .args([
                "-c",
                include_str!("../support/response_types_sdk.py"),
                &h.url,
                &h.model,
                &lite.slug,
            ])
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
    let received = h.fake.received.lock().unwrap();
    assert_eq!(received.len(), 22, "invalid fields must not reach upstream");
    for (group, is_lite) in received.chunks_exact(11).zip([false, true]) {
        for (i, context) in ["auto", "current_turn", "all_turns"].iter().enumerate() {
            for row in &group[i * 2..i * 2 + 2] {
                assert_eq!(row.1["reasoning"]["context"], *context);
                let expected_lite = is_lite && *context == "all_turns";
                assert_eq!(
                    row.1["input"][0]["type"] == "additional_tools",
                    expected_lite
                );
                assert_eq!(
                    row.0.contains_key("x-openai-internal-codex-responses-lite"),
                    expected_lite
                );
            }
        }
        for row in &group[6..8] {
            assert_eq!(
                row.1["reasoning"]["context"],
                if is_lite {
                    json!("all_turns")
                } else {
                    Value::Null
                }
            );
        }
        assert_eq!(group[8].1["reasoning"]["summary"], "auto");
        assert!(group[9].1["text"].get("format").is_none());
        assert_eq!(group[10].1["text"]["format"]["type"], "json_schema");
        assert_eq!(group[10].1["text"]["format"]["name"], "result");
    }
}

#[tokio::test]
async fn public_websocket_preserves_reasoning_context() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let catalog = codex_models_manager::bundled_models_response().unwrap();
    for model in [
        catalog
            .models
            .iter()
            .find(|m| !m.use_responses_lite)
            .unwrap(),
        catalog
            .models
            .iter()
            .find(|m| m.use_responses_lite)
            .unwrap(),
    ] {
        for context in ["auto", "current_turn", "all_turns"] {
            let mut request = format!("{}/v1/responses", h.url)
                .replacen("http:", "ws:", 1)
                .into_client_request()
                .unwrap();
            request
                .headers_mut()
                .insert("authorization", "Bearer client-key".parse().unwrap());
            let (mut ws, _) = tokio_tungstenite::connect_async(request).await.unwrap();
            ws.send(Message::Text(json!({"type":"response.create", "model":model.slug,"input":"hello","reasoning":{"context":context}}).to_string().into())).await.unwrap();
            loop {
                let message = ws.next().await.unwrap().unwrap();
                let value: Value = serde_json::from_str(&message.into_text().unwrap()).unwrap();
                assert_ne!(value["type"], "error", "{value}");
                if value["type"] == "response.completed" {
                    break;
                }
            }
            if model.use_responses_lite {
                assert_eq!(
                    h.fake.received.lock().unwrap().last().unwrap().1["reasoning"]["context"],
                    context
                );
            } else {
                let frames = h.fake.ws_frames.lock().unwrap();
                let frame: Value = serde_json::from_str(frames.last().unwrap()).unwrap();
                assert_eq!(frame["reasoning"]["context"], context);
            }
            ws.close(None).await.unwrap();
        }
    }
}

#[tokio::test]
async fn public_compaction_preserves_explicit_reasoning_context() {
    let compaction = json!({"type":"compaction","id":"cmp_1","encrypted_content":"encrypted"});
    let h = Harness::new(
        vec![completed(vec![compaction.clone()])],
        StatusCode::OK,
        false,
        Duration::from_secs(5),
    )
    .await;
    for context in ["auto", "current_turn", "all_turns"] {
        let response = reqwest::Client::new()
            .post(format!("{}/v1/responses/compact", h.url))
            .bearer_auth("client-key")
            .json(&json!({"model":h.model,"input":"hello","reasoning":{"context":context}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.json::<Value>().await.unwrap()["output"][0],
            compaction
        );
        assert_eq!(
            h.fake.received.lock().unwrap().last().unwrap().1["reasoning"]["context"],
            context
        );
    }
}

#[tokio::test]
#[ignore = "requires uv sync --locked; uses local fake upstreams"]
async fn actual_openai_sdk_compatibility_suite() {
    fn sdk_events() -> Vec<Value> {
        let mut events = events();
        events[0]["response"]["output"] = json!([]);
        events[1]["item"]["content"] = json!([]);
        events
    }
    let text = Harness::new(sdk_events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    fn replace_text(value: &mut Value) {
        match value {
            Value::String(text) if text == "你好" => *text = r#"{"ok":true}"#.into(),
            Value::Array(items) => items.iter_mut().for_each(replace_text),
            Value::Object(items) => items.values_mut().for_each(replace_text),
            _ => {}
        }
    }
    let mut structured_events = sdk_events();
    for event in &mut structured_events {
        replace_text(event);
    }
    let structured = Harness::new(
        structured_events,
        StatusCode::OK,
        false,
        Duration::from_secs(5),
    )
    .await;
    let compact = Harness::new(
        vec![completed(vec![
            json!({"type":"compaction","id":"cmp_fixture","encrypted_content":"opaque-fixture"}),
        ])],
        StatusCode::OK,
        false,
        Duration::from_secs(5),
    )
    .await;
    let files = Harness::with_auth(
        events(),
        StatusCode::OK,
        false,
        Duration::from_secs(5),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    )
    .await;
    let limited = Harness::new(
        events(),
        StatusCode::TOO_MANY_REQUESTS,
        false,
        Duration::from_secs(5),
    )
    .await;
    let failed = Harness::new(vec![json!({"type":"response.failed","response":{"id":"resp_failed","object":"response","status":"failed","output":[],"error":{"code":"server_error","message":"fixture failure"}}})], StatusCode::OK, false, Duration::from_secs(5)).await;
    let incomplete = Harness::new(vec![json!({"type":"response.incomplete","response":{"id":"resp_incomplete","object":"response","status":"incomplete","output":[],"incomplete_details":{"reason":"max_output_tokens"}}})], StatusCode::OK, false, Duration::from_secs(5)).await;
    let catalog = codex_models_manager::bundled_models_response().unwrap();
    let lite = catalog
        .models
        .iter()
        .find(|m| m.use_responses_lite)
        .unwrap();
    async fn tool_harness(item: Value) -> Harness {
        let h = Harness::new(
            vec![
                json!({"type":"response.output_item.done","output_index":0,"item":item}),
                completed(vec![item]),
            ],
            StatusCode::OK,
            false,
            Duration::from_secs(5),
        )
        .await;
        h.fake
            .sdk_tool_roundtrip
            .store(true, std::sync::atomic::Ordering::SeqCst);
        h
    }
    let function = tool_harness(json!({"type":"function_call","id":"fc_1","call_id":"call_fixture","name":"lookup","arguments":"{}","status":"completed"})).await;
    let custom = tool_harness(json!({"type":"custom_tool_call","id":"ct_1","call_id":"call_custom","name":"patch","input":"abc","status":"completed"})).await;
    let config = json!({"text":text.url,"function":function.url,"custom":custom.url,"structured":structured.url,"compact":compact.url,"files":files.url,"limited":limited.url,"failed":failed.url,"incomplete":incomplete.url,"models":[text.model,lite.slug],"png":tool_cases::IMAGE_PNG});
    let output = tokio::time::timeout(
        Duration::from_secs(120),
        support::python::command()
            .args([
                "-c",
                include_str!("../support/sdk_compat.py"),
                &config.to_string(),
            ])
            .output(),
    )
    .await
    .expect("SDK suite timeout")
    .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "SDK failed before report: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("artifacts/sdk");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("compatibility.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    let failures: Vec<_> = report["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["status"] != "passed")
        .collect();
    assert!(
        output.status.success(),
        "SDK failures: {}\n{}",
        serde_json::to_string_pretty(&failures).unwrap(),
        String::from_utf8_lossy(&output.stderr)
    );
    let captured = text.fake.received.lock().unwrap();
    for model in [&text.model, &lite.slug] {
        assert!(
            captured
                .iter()
                .any(|(_, v)| v["model"] == *model && v["reasoning"]["context"] == "current_turn")
        );
        assert!(captured.iter().any(|(_, v)| {
            v["model"] == *model
                && v["input"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|i| i["type"] == "function_call_output" && i["output"] == "42")
        }));
        assert!(captured.iter().any(|(_, v)| {
            v["model"] == *model
                && v["input"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|i| i["type"] == "reasoning" && i["encrypted_content"] == "opaque-fixture")
        }));
    }
    let schema_requests = structured.fake.received.lock().unwrap();
    assert!(schema_requests.len() >= 3);
    for (_, v) in schema_requests.iter() {
        assert_eq!(
            v["text"]["format"]["schema"]["properties"]["ok"]["type"],
            "boolean"
        );
    }
}
