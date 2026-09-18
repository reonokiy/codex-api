use super::*;

pub(super) async fn upstream_call(
    State(fake): State<Fake>,
    uri: axum::extract::OriginalUri,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    fake.http_bodies.lock().unwrap().push(bytes.to_vec());
    let value = json!({"path":uri.path_and_query().unwrap().as_str(), "body":serde_json::from_slice::<Value>(&bytes).ok()});
    fake.received.lock().unwrap().push((headers, value));
    if fake.hang {
        std::future::pending::<()>().await;
    }
    if fake.status != StatusCode::OK {
        return Response::builder()
            .status(fake.status)
            .header("content-type", "application/json")
            .header("retry-after", "7")
            .body(Body::from(r#"{"error":{"code":"call_rejected"}}"#))
            .unwrap();
    }
    if uri.path() == "/live/sessions" {
        return Response::builder().status(StatusCode::CREATED)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"session":{"id":"live_test"},"transport":{"type":"webrtc","sdp":"v=answer\r\n"},"future":true}"#)).unwrap();
    }
    Response::builder()
        .status(StatusCode::CREATED)
        .header("content-type", "application/sdp")
        .header("location", "/v1/realtime/calls/rtc_gateway_test")
        .body(Body::from("v=answer\r\n"))
        .unwrap()
}

#[tokio::test]
async fn realtime_calls_preserve_native_json_and_convert_official_multipart() {
    let h = Harness::with_auth(
        vec![],
        StatusCode::OK,
        false,
        Duration::from_secs(5),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    )
    .await;
    let client = reqwest::Client::new();
    let native =
        r#"{ "sdp":"v=offer\r\n", "session":{"type":"quicksilver","future":{"preserved":true}} }"#;
    for prefix in ["codex", "backend-api/codex"] {
        let response = client
            .post(format!(
                "{}/{prefix}/realtime/calls?intent=quicksilver&architecture=avas",
                h.url
            ))
            .bearer_auth("client-key")
            .header("content-type", "application/json")
            .header("openai-alpha", "quicksilver=v1")
            .header("x-session-id", "session-test")
            .body(native)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response.headers()["location"],
            "/v1/realtime/calls/rtc_gateway_test"
        );
        assert_eq!(response.text().await.unwrap(), "v=answer\r\n");
    }
    assert!(
        h.fake
            .http_bodies
            .lock()
            .unwrap()
            .iter()
            .all(|bytes| bytes == native.as_bytes())
    );
    let session = json!({"type":"realtime","model":"gpt-realtime","audio":{"output":{"voice":"marin"}},"future":{"preserved":true}});
    let response = client
        .post(format!("{}/v1/realtime/calls", h.url))
        .bearer_auth("client-key")
        .multipart(
            reqwest::multipart::Form::new()
                .text("sdp", "v=offer\r\n")
                .text("session", session.to_string()),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["content-type"], "application/sdp");
    let received = h.fake.received.lock().unwrap();
    assert_eq!(received[0].0["openai-alpha"], "quicksilver=v1");
    assert_eq!(received[0].0["x-session-id"], "session-test");
    assert_eq!(received[0].0["chatgpt-account-id"], "account_id");
    assert_ne!(received[0].0["authorization"], "Bearer client-key");
    assert_eq!(
        received[2].1["body"],
        json!({"sdp":"v=offer\r\n","session":session})
    );
}

#[tokio::test]
async fn realtime_explicit_api_credentials_preserve_official_raw_request() {
    let h = Harness::new(vec![], StatusCode::OK, false, Duration::from_secs(5)).await;
    let client = reqwest::Client::new();
    let body = "--test-boundary\r\nContent-Disposition: form-data; name=\"sdp\"\r\n\r\nv=offer\r\n--test-boundary--\r\n";
    let response = client
        .post(format!("{}/v1/realtime/calls?model=explicit-model", h.url))
        .header("x-codex-gateway-authorization", "Bearer client-key")
        .bearer_auth("explicit-platform-key")
        .header(
            "content-type",
            "multipart/form-data; boundary=test-boundary",
        )
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let received = h.fake.received.lock().unwrap();
    assert_eq!(
        received[0].0["authorization"],
        "Bearer explicit-platform-key"
    );
    assert!(!received[0].0.contains_key("x-codex-gateway-authorization"));
    assert_eq!(
        received[0].0["content-type"],
        "multipart/form-data; boundary=test-boundary"
    );
    assert_eq!(
        received[0].1["path"],
        "/realtime/calls?model=explicit-model"
    );
    assert_eq!(h.fake.http_bodies.lock().unwrap()[0], body.as_bytes());
}

#[tokio::test]
async fn modern_live_uses_explicit_credentials_and_retains_current_json_contract() {
    let h = Harness::new(vec![], StatusCode::OK, false, Duration::from_secs(5)).await;
    let client = reqwest::Client::new();
    let body = r#"{ "session":{"model":"gpt-live-1","delegation":{"type":"client"}}, "transport":{"type":"webrtc","sdp":"v=offer\r\n"}, "future":true }"#;
    let rejected = client
        .post(format!("{}/v1/live/sessions", h.url))
        .bearer_auth("client-key")
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert!(h.fake.received.lock().unwrap().is_empty());
    let response = client
        .post(format!("{}/v1/live/sessions", h.url))
        .header("x-codex-gateway-authorization", "Bearer client-key")
        .bearer_auth("explicit-platform-key")
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["content-type"], "application/json");
    let value: Value = response.json().await.unwrap();
    assert_eq!(value["session"]["id"], "live_test");
    assert_eq!(value["transport"]["sdp"], "v=answer\r\n");
    assert_eq!(value["future"], true);
    assert_eq!(h.fake.http_bodies.lock().unwrap()[0], body.as_bytes());
}

#[tokio::test]
async fn realtime_websockets_preserve_events_binary_and_explicit_auth_without_starting_a_session() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
    for (path, delegated) in [
        ("v1/realtime?call_id=rtc_test", false),
        ("backend-api/codex/live/rtc_test", false),
        ("v1/realtime?model=explicit-model", true),
        ("v1/live/sessions", true),
        ("v1/live/sessions/live_test/attach", true),
    ] {
        let event = json!({"type":"future.realtime.event","preserved":{"x":1}});
        let h = Harness::with_auth(
            vec![event.clone()],
            StatusCode::OK,
            false,
            Duration::from_secs(5),
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        )
        .await;
        let mut request = format!("{}/{path}", h.url.replace("http://", "ws://"))
            .into_client_request()
            .unwrap();
        request.headers_mut().insert(
            "authorization",
            if delegated {
                "Bearer explicit-platform-key"
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
        request
            .headers_mut()
            .insert("openai-alpha", "quicksilver=v2".parse().unwrap());
        request
            .headers_mut()
            .insert("x-session-id", "session-test".parse().unwrap());
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), socket.next())
                .await
                .is_err()
        );
        assert!(h.fake.ws_frames.lock().unwrap().is_empty());
        let outgoing = "{ \"type\":\"session.context.append\", \"future\":[1,2] }";
        socket.send(Message::Text(outgoing.into())).await.unwrap();
        assert_eq!(
            socket.next().await.unwrap().unwrap(),
            Message::Text(event.to_string().into())
        );
        socket
            .send(Message::Binary(Bytes::from_static(b"\0\xff")))
            .await
            .unwrap();
        assert_eq!(
            socket.next().await.unwrap().unwrap(),
            Message::Binary(Bytes::from_static(b"\0\xff"))
        );
        assert_eq!(h.fake.ws_frames.lock().unwrap()[0], outgoing);
        let received = h.fake.received.lock().unwrap();
        assert_eq!(received[0].0["openai-alpha"], "quicksilver=v2");
        assert_eq!(received[0].0["x-session-id"], "session-test");
        assert!(!received[0].0.contains_key("x-codex-gateway-authorization"));
        if delegated {
            assert_eq!(
                received[0].0["authorization"],
                "Bearer explicit-platform-key"
            );
        } else {
            assert_eq!(received[0].0["chatgpt-account-id"], "account_id");
        }
    }
}
