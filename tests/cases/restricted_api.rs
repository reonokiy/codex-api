use super::*;

async fn restricted_harness() -> Harness {
    restricted_harness_with_events(events()).await
}

async fn restricted_harness_with_events(events: Vec<Value>) -> Harness {
    Harness::with_api_policy(
        events,
        StatusCode::OK,
        false,
        Duration::from_secs(5),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        true,
        16 * 1024 * 1024,
        false,
    )
    .await
}

#[tokio::test]
async fn full_api_explicitly_enables_account_and_stored_data_routes() {
    let h = Harness::with_api_policy(
        vec![],
        StatusCode::OK,
        false,
        Duration::from_secs(5),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        false,
        16 * 1024 * 1024,
        true,
    )
    .await;
    let client = reqwest::Client::new();
    for path in [
        "/backend-api/wham/profiles/me",
        "/api/codex/accounts/check",
        "/codex/alpha/history/v2/read_item",
        "/backend-api/codex/alpha/notes/v2/read_file",
    ] {
        let response = client
            .get(format!("{}{path}", h.url))
            .bearer_auth("client-key")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }
    assert_eq!(h.fake.received.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn restricted_api_blocks_inventoried_routes_and_aliases_before_upstream() {
    let h = restricted_harness().await;
    let inventory: Value =
        serde_json::from_str(include_str!("../../baseline/api-inventory.json")).unwrap();
    let client = reqwest::Client::new();
    let mut checked = 0;
    for entry in inventory["entries"].as_array().unwrap() {
        if matches!(
            entry["id"].as_str().unwrap(),
            "responses"
                | "models"
                | "image-generations"
                | "image-edits"
                | "search"
                | "historical-audio-transcription"
        ) {
            continue;
        }
        for path in entry["gateway_paths"].as_array().unwrap() {
            let path = path
                .as_str()
                .unwrap()
                .split('/')
                .map(|part| {
                    if part.starts_with('{') {
                        "opaque-id"
                    } else {
                        part
                    }
                })
                .collect::<Vec<_>>()
                .join("/");
            for method in entry["methods"].as_array().unwrap() {
                let response = client
                    .request(
                        reqwest::Method::from_bytes(method.as_str().unwrap().as_bytes()).unwrap(),
                        format!("{}{path}?future=true", h.url),
                    )
                    .bearer_auth("client-key")
                    .body("must not reach upstream")
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::FORBIDDEN, "{method} {path}");
                assert_eq!(
                    response.json::<Value>().await.unwrap()["error"]["code"],
                    "endpoint_disabled"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "checked {checked} inventory routes");
    assert!(h.fake.http_bodies.lock().unwrap().is_empty());
    assert_eq!(
        h.fake
            .ws_connections
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn restricted_api_denies_unlisted_paths_methods_and_proxy_escape_routes() {
    let h = restricted_harness().await;
    let client = reqwest::Client::new();
    for path in [
        "/v1/files",
        "/v1/realtime/calls",
        "/codex/live/sessions",
        "/backend-api/codex/live/sessions/session/attach",
        "/transfers/opaque-handle",
        "/platform/responses",
        "/platform/organizations",
        "/backend-api/future-account-endpoint",
        "/codex/future-account-endpoint",
        "/api/codex/future-account-endpoint",
        "/auth/future-account-endpoint",
        "/v1/responses/resp_saved",
        "/v1/responses/",
        "/codex/responses/extra",
        "/codex/%72esponses",
        "/codex/responses%2f..%2fwham/usage",
        "/codex//responses",
        "/v1/audio/sessions",
        "/unknown",
    ] {
        let response = client
            .post(format!("{}{path}", h.url))
            .bearer_auth("client-key")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
    }
    for method in [
        reqwest::Method::DELETE,
        reqwest::Method::PUT,
        reqwest::Method::PATCH,
    ] {
        let response = client
            .request(method, format!("{}/codex/responses", h.url))
            .bearer_auth("client-key")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let response = client
        .get(format!("{}/api/codex/usage", h.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(h.fake.http_bodies.lock().unwrap().is_empty());
}

#[tokio::test]
async fn restricted_api_keeps_models_responses_and_websockets_working() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let h = restricted_harness().await;
    let client = reqwest::Client::new();
    assert_eq!(
        client
            .get(format!("{}/healthz", h.url))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    for prefix in ["v1", "codex", "backend-api/codex"] {
        let response = client
            .get(format!("{}/{prefix}/models?client_version=0.163.0", h.url))
            .bearer_auth("client-key")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{prefix} models");
        let body = response.json::<Value>().await.unwrap();
        assert!(body["models"].is_array());

        let response = client
            .post(format!("{}/{prefix}/responses", h.url))
            .bearer_auth("client-key")
            .json(&json!({"model":h.model,"input":"hello","stream":true}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{prefix} responses");
        assert!(
            response
                .text()
                .await
                .unwrap()
                .contains("response.completed")
        );

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
        assert!(socket.next().await.unwrap().unwrap().is_text());
        socket.close(None).await.unwrap();
    }
}

#[tokio::test]
async fn restricted_api_keeps_standalone_inference_aliases() {
    let h = restricted_harness().await;
    let client = reqwest::Client::new();
    // Invalid inputs must reach each specialized handler's validation, rather
    // than being rejected by the access policy or sent to a generic proxy.
    for suffix in ["images/generations", "images/edits", "alpha/search"] {
        for prefix in ["v1", "codex", "backend-api/codex"] {
            let response = client
                .post(format!("{}/{prefix}/{suffix}", h.url))
                .bearer_auth("client-key")
                .json(&json!({}))
                .send()
                .await
                .unwrap();
            assert!(
                matches!(
                    response.status(),
                    StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
                ),
                "{prefix}/{suffix}: {}",
                response.status()
            );
        }
    }
    for path in [
        "/transcribe",
        "/v1/audio/transcriptions",
        "/backend-api/transcribe",
    ] {
        let response = client
            .post(format!("{}{path}", h.url))
            .bearer_auth("client-key")
            .header("content-type", "multipart/form-data; boundary=test")
            .body("--test--\r\n")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }
    assert_eq!(h.fake.http_bodies.lock().unwrap().len(), 3);
}

#[tokio::test]
async fn restricted_api_keeps_compaction_and_image_and_search_generation() {
    let h = restricted_harness_with_events(vec![completed(vec![json!({
        "type":"compaction", "id":"cmp_1", "encrypted_content":"encrypted-compaction"
    })])])
    .await;
    let client = reqwest::Client::new();
    for prefix in ["v1", "codex", "backend-api/codex"] {
        for (suffix, body) in [
            (
                "images/generations",
                json!({"model":"gpt-image-2","prompt":"a square"}),
            ),
            (
                "images/edits",
                json!({"model":"gpt-image-2","prompt":"a blue square","images":[{"image_url":format!("data:image/png;base64,{}", tool_cases::IMAGE_PNG)}]}),
            ),
            (
                "alpha/search",
                json!({"id":"search-session","model":h.model,"input":"Find Codex"}),
            ),
            (
                "responses/compact",
                json!({"model":h.model,"input":[{"role":"user","content":"compress"}]}),
            ),
        ] {
            let response = client
                .post(format!("{}/{prefix}/{suffix}", h.url))
                .bearer_auth("client-key")
                .json(&body)
                .send()
                .await
                .unwrap();
            let status = response.status();
            let output = response.text().await.unwrap();
            assert_eq!(status, StatusCode::OK, "{prefix}/{suffix}: {output}");
            assert!(!output.is_empty());
        }
    }
}
