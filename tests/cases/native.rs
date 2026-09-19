use super::*;

pub(super) async fn upstream_native(
    State(fake): State<Fake>,
    request: axum::extract::Request,
) -> Response {
    let (parts, body) = request.into_parts();
    let body = axum::body::to_bytes(body, 16 * 1024 * 1024).await.unwrap();
    fake.http_bodies.lock().unwrap().push(body.to_vec());
    fake.received.lock().unwrap().push((
        parts.headers,
        json!({"method":parts.method.as_str(),"path":parts.uri.path_and_query().unwrap().as_str()}),
    ));
    if fake.hang {
        std::future::pending::<()>().await;
    }
    Response::builder()
        .status(fake.status)
        .header("content-type", "application/octet-stream")
        .header("location", "/keep-original-location?q=a%2Fb")
        .header("mcp-session-id", "opaque-session")
        .header("x-future-field", "retained")
        .header("connection", "x-hop-header")
        .header("x-hop-header", "remove-me")
        .body(Body::from(body))
        .unwrap()
}

pub(super) async fn upstream_memory(
    State(fake): State<Fake>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    fake.received
        .lock()
        .unwrap()
        .push((headers, serde_json::from_slice(&body).unwrap()));
    Response::builder().header("content-type", "application/json")
        .body(Body::from(r#"{"output":[{"trace_summary":"trace","memory_summary":"memory"}],"future":{"preserved":true}}"#)).unwrap()
}

#[tokio::test]
async fn native_routes_preserve_all_inventoried_backend_contracts() {
    let inventory: Value =
        serde_json::from_str(include_str!("../../baseline/api-inventory.json")).unwrap();
    let h = Harness::with_auth(
        vec![],
        StatusCode::OK,
        false,
        Duration::from_secs(5),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    )
    .await;
    let client = reqwest::Client::new();
    let payload = b"{ \"future\": [true, 1.00, null], \"signature\":\"unchanged\" }";
    let mut checked = 0;
    for entry in inventory["entries"].as_array().unwrap() {
        let path = entry["path"].as_str().unwrap();
        if !path.starts_with("/backend-api/")
            || [
                "/responses",
                "/models",
                "/images/",
                "/alpha/search",
                "/memories/",
                "/realtime/calls",
                "/backend-api/files",
            ]
            .iter()
            .any(|v| path.contains(v))
        {
            continue;
        }
        let path = path
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
        let upstream_path = path
            .strip_prefix("/backend-api/codex")
            .unwrap_or_else(|| path.strip_prefix("/backend-api").unwrap());
        for method in entry["methods"].as_array().unwrap() {
            if method == "GET"
                && (path.ends_with("/guardian")
                    || path.ends_with("/guardian-classifier")
                    || path.ends_with("/remote/control/server"))
            {
                continue;
            }
            let method = method.as_str().unwrap();
            let mut req = client
                .request(
                    method.parse().unwrap(),
                    format!("{}{path}?cursor=a%2Fb&x=1&x=2", h.url),
                )
                .header("content-type", "application/json")
                .header("mcp-session-id", "caller-session")
                .header("x-future-field", "keep")
                .header("user-agent", "original-codex-test")
                .header("originator", "downstream-sdk")
                .header("x-stainless-os", "client-os")
                .header("x-forwarded-for", "192.0.2.10")
                .header("origin", "https://client.example")
                .header("connection", "x-private-hop")
                .header("x-private-hop", "client-only")
                .body(payload.as_slice());
            if entry["auth"] == "remote_control_token" {
                req = req
                    .header("x-codex-gateway-authorization", "Bearer client-key")
                    .bearer_auth("remote-session-token");
            } else {
                req = req.bearer_auth("client-key");
            }
            let response = req.send().await.unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{method} {path}");
            assert_eq!(
                response.headers()["location"],
                "/keep-original-location?q=a%2Fb"
            );
            assert_eq!(response.headers()["mcp-session-id"], "opaque-session");
            assert_eq!(response.headers()["x-future-field"], "retained");
            assert!(!response.headers().contains_key("x-hop-header"));
            assert_eq!(response.bytes().await.unwrap(), payload.as_slice());
            let recorded = h.fake.received.lock().unwrap();
            let (headers, value) = recorded.last().unwrap();
            assert_eq!(value["method"], method);
            assert_eq!(
                value["path"],
                format!("{upstream_path}?cursor=a%2Fb&x=1&x=2")
            );
            assert_eq!(headers["mcp-session-id"], "caller-session");
            assert_eq!(headers["x-future-field"], "keep");
            assert_ne!(
                headers.get("user-agent").and_then(|v| v.to_str().ok()),
                Some("original-codex-test")
            );
            assert_ne!(
                headers.get("originator").and_then(|v| v.to_str().ok()),
                Some("downstream-sdk")
            );
            for name in [
                "x-stainless-os",
                "x-forwarded-for",
                "origin",
                "x-private-hop",
            ] {
                assert!(!headers.contains_key(name), "leaked {name} on {path}");
            }
            assert!(!headers.contains_key("x-codex-gateway-authorization"));
            match entry["auth"].as_str().unwrap() {
                "none" => assert!(!headers.contains_key("authorization")),
                "remote_control_token" => {
                    assert_eq!(headers["authorization"], "Bearer remote-session-token")
                }
                _ => assert_ne!(headers["authorization"], "Bearer client-key"),
            }
            checked += 1;
        }
    }
    assert!(checked >= 55, "only {checked} source contracts exercised");
}

#[tokio::test]
async fn native_errors_redirects_and_auth_modes_remain_transparent() {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    for status in [
        StatusCode::BAD_REQUEST,
        StatusCode::TEMPORARY_REDIRECT,
        StatusCode::SERVICE_UNAVAILABLE,
    ] {
        let h = Harness::new(vec![], status, false, Duration::from_secs(5)).await;
        let payload = [0xff, 0xfe, 0, 123];
        let response = client
            .post(format!("{}/backend-api/ps/mcp", h.url))
            .bearer_auth("client-key")
            .body(payload.to_vec())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(
            response.headers()["location"],
            "/keep-original-location?q=a%2Fb"
        );
        assert_eq!(response.bytes().await.unwrap(), payload.as_slice());
        assert_eq!(
            h.fake.received.lock().unwrap().len(),
            1,
            "backend mutations are not retried"
        );
    }
    let h = Harness::new(vec![], StatusCode::OK, false, Duration::from_secs(5)).await;
    for (path, credential) in [
        ("/auth/oauth/token", None),
        (
            "/auth/api/accounts/v1/user-auth-credential/whoami",
            Some("personal-token"),
        ),
        ("/platform/test", Some("explicit-platform-key")),
        (
            "/backend-api/wham/remote/control/server/pair",
            Some("remote-token"),
        ),
    ] {
        let mut request = client
            .post(format!("{}{path}", h.url))
            .body("unparsed=token%2Bvalue");
        if let Some(token) = credential {
            request = request
                .header("x-codex-gateway-authorization", "Bearer client-key")
                .bearer_auth(token);
        } else {
            request = request.bearer_auth("client-key");
        }
        assert_eq!(request.send().await.unwrap().status(), StatusCode::OK);
        let requests = h.fake.received.lock().unwrap();
        let headers = &requests.last().unwrap().0;
        assert_eq!(
            headers
                .get("authorization")
                .map(|v| v.to_str().unwrap().to_owned()),
            credential.map(|v| format!("Bearer {v}"))
        );
    }
    let before = h.fake.received.lock().unwrap().len();
    for path in ["/backend-api/wham/tasks", "/auth/oauth/token"] {
        assert_eq!(
            client
                .post(format!("{}{path}", h.url))
                .body("x")
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        client
            .post(format!("{}/platform/files", h.url))
            .bearer_auth("client-key")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(h.fake.received.lock().unwrap().len(), before);
}

#[tokio::test]
async fn native_inference_retains_original_retry_policy_and_final_binary_error() {
    let h = Harness::new(
        vec![],
        StatusCode::SERVICE_UNAVAILABLE,
        false,
        Duration::from_secs(10),
    )
    .await;
    let body = vec![0xff, 0, 12];
    let response = reqwest::Client::new()
        .post(format!("{}/codex/alpha/notes/v2/read_file", h.url))
        .bearer_auth("client-key")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.bytes().await.unwrap(), body);
    let original = ModelProviderInfo::create_openai_provider(None);
    assert_eq!(
        h.fake.received.lock().unwrap().len() as u64,
        original.request_max_retries() + 1
    );
}

#[tokio::test]
async fn native_memory_uses_original_parser_and_preserves_future_response_fields() {
    let h = Harness::new(vec![], StatusCode::OK, false, Duration::from_secs(5)).await;
    let body = json!({"model":h.model,"traces":[{"id":"trace1","metadata":{"source_path":"rollout.jsonl"},"items":[{"type":"message","content":[]}]}],"future":true});
    let response = reqwest::Client::new()
        .post(format!("{}/codex/memories/trace_summarize", h.url))
        .bearer_auth("client-key")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({"output":[{"trace_summary":"trace","memory_summary":"memory"}],"future":{"preserved":true}})
    );
    assert_eq!(h.fake.received.lock().unwrap()[0].1, body);
}

#[tokio::test]
async fn native_guardian_and_remote_control_websockets_relay_opaque_frames() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
    for (path, delegated) in [
        ("/codex/guardian", false),
        ("/codex/guardian-classifier", false),
        ("/backend-api/wham/remote/control/server", true),
    ] {
        let event = json!({"unknown-event":true});
        let h = Harness::new(
            vec![event.clone()],
            StatusCode::OK,
            false,
            Duration::from_secs(5),
        )
        .await;
        let mut request = format!("{}{path}", h.url.replace("http:", "ws:"))
            .into_client_request()
            .unwrap();
        request.headers_mut().insert(
            "authorization",
            if delegated {
                "Bearer remote-token"
            } else {
                "Bearer client-key"
            }
            .parse()
            .unwrap(),
        );
        if delegated {
            request.headers_mut().insert(
                "x-codex-gateway-authorization",
                "Bearer client-key".parse().unwrap(),
            );
        }
        let (mut ws, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        ws.send(Message::Text("{\"native\":true}".into()))
            .await
            .unwrap();
        let received = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(received.to_text().unwrap()).unwrap(),
            event
        );
        let headers = h.fake.received.lock().unwrap()[0].0.clone();
        assert_eq!(
            headers["authorization"],
            if delegated {
                "Bearer remote-token"
            } else {
                "Bearer upstream-secret"
            }
        );
        assert!(!headers.contains_key("x-codex-gateway-authorization"));
        ws.close(None).await.unwrap();
    }
}
