use bytes::Bytes;
use codex_api::{
    AuthProvider, Provider, RawClient, RealtimeCallClient, RealtimeEventParser,
    RealtimeOutputModality, RealtimeSessionConfig, RealtimeSessionMode, RealtimeWebsocketClient,
    RetryConfig, session_update_session_json,
};
use codex_client::{HttpTransport, Request, RequestBody, Response, StreamResponse, TransportError};
use codex_protocol::protocol::RealtimeVoice;
use futures::{SinkExt, StreamExt};
use http::{HeaderMap, HeaderValue, Method, StatusCode};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

fn provider(base_url: String) -> Provider {
    Provider {
        name: "test".into(),
        base_url,
        query_params: None,
        headers: HeaderMap::new(),
        retry: RetryConfig {
            max_attempts: 1,
            base_delay: Duration::from_millis(1),
            retry_429: false,
            retry_5xx: false,
            retry_transport: false,
        },
        stream_idle_timeout: Duration::from_secs(1),
    }
}

fn config() -> RealtimeSessionConfig {
    RealtimeSessionConfig {
        instructions: "Keep answers brief".into(),
        initial_items: vec![],
        delegation_ack_filler: None,
        model: Some("test-model".into()),
        session_id: Some("session-test".into()),
        event_parser: RealtimeEventParser::V1,
        session_mode: RealtimeSessionMode::Conversational,
        output_modality: RealtimeOutputModality::Audio,
        voice: RealtimeVoice::Cove,
    }
}

struct Auth;
impl AuthProvider for Auth {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        headers.insert("authorization", HeaderValue::from_static("Bearer upstream"));
    }
}

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<Request>>>);
impl HttpTransport for Capture {
    async fn execute(&self, request: Request) -> Result<Response, TransportError> {
        self.0.lock().unwrap().push(request);
        let mut headers = HeaderMap::new();
        headers.insert("location", HeaderValue::from_static("/v1/live/rtc_test"));
        Ok(Response {
            status: StatusCode::CREATED,
            headers,
            body: Bytes::from_static(b"v=answer\r\n"),
        })
    }
    async fn stream(&self, request: Request) -> Result<StreamResponse, TransportError> {
        let response = self.execute(request).await?;
        Ok(StreamResponse {
            status: response.status,
            headers: response.headers,
            bytes: Box::pin(futures::stream::once(async { Ok(response.body) })),
        })
    }
}

#[tokio::test]
async fn raw_http_retains_original_call_requests_and_responses() {
    for parser in [RealtimeEventParser::V1, RealtimeEventParser::FramelessBidi] {
        let capture = Capture::default();
        let provider = provider("https://chatgpt.com/backend-api/codex".into());
        let mut config = config();
        config.event_parser = parser;
        let native = RealtimeCallClient::new(capture.clone(), provider.clone(), Arc::new(Auth));
        let answer = native
            .create_with_session("v=offer\r\n".into(), config.clone())
            .await
            .unwrap();
        let mut session = session_update_session_json(config).unwrap();
        session.as_object_mut().unwrap().remove("id");
        let raw = RawClient::new(capture.clone(), provider, Arc::new(Auth));
        let answer_raw = raw
            .execute(
                Method::POST,
                "realtime/calls?intent=quicksilver&architecture=avas",
                HeaderMap::new(),
                Some(RequestBody::Json(
                    serde_json::json!({"sdp":"v=offer\r\n", "session":session}),
                )),
            )
            .await
            .unwrap();
        assert_eq!(answer_raw.status, StatusCode::CREATED);
        assert_eq!(answer_raw.headers["location"], "/v1/live/rtc_test");
        assert_eq!(answer.sdp.as_bytes(), answer_raw.body.as_ref());
        let requests = capture.0.lock().unwrap();
        assert_eq!(requests[0].url, requests[1].url);
        assert_eq!(requests[0].method, requests[1].method);
        assert_eq!(requests[0].headers, requests[1].headers);
        assert_eq!(requests[0].body, requests[1].body);
        assert_eq!(requests[0].compression, requests[1].compression);
        assert_eq!(requests[0].timeout, requests[1].timeout);
    }
}

#[tokio::test]
async fn raw_http_preserves_binary_body_and_explicit_media_type() {
    let capture = Capture::default();
    let raw = RawClient::new(
        capture.clone(),
        provider("https://example.invalid/v1".into()),
        Arc::new(Auth),
    );
    let body = Bytes::from_static(b"--boundary\r\n\0\xff\r\n--boundary--\r\n");
    let mut headers = HeaderMap::new();
    headers.insert(
        "content-type",
        HeaderValue::from_static("multipart/form-data; boundary=boundary"),
    );
    headers.insert("authorization", HeaderValue::from_static("Bearer caller"));
    let mut response = raw
        .stream(
            Method::POST,
            "realtime/calls",
            headers,
            Some(RequestBody::Raw(body.clone())),
        )
        .await
        .unwrap();
    assert_eq!(response.status, StatusCode::CREATED);
    assert_eq!(
        response.bytes.next().await.unwrap().unwrap(),
        b"v=answer\r\n"[..]
    );
    let requests = capture.0.lock().unwrap();
    assert_eq!(requests[0].body, Some(RequestBody::Raw(body)));
    assert_eq!(
        requests[0].headers["content-type"],
        "multipart/form-data; boundary=boundary"
    );
    assert_eq!(requests[0].headers["authorization"], "Bearer upstream");
}

#[tokio::test]
async fn raw_realtime_matches_native_handshake_without_initializing_a_session() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1/realtime", listener.local_addr().unwrap());
    let captures = Arc::new(Mutex::new(Vec::new()));
    let captured = captures.clone();
    let server = tokio::spawn(async move {
        for native in [true, false] {
            let (stream, _) = listener.accept().await.unwrap();
            let captured = captured.clone();
            let mut ws = tokio_tungstenite::accept_hdr_async(stream, move |request: &tokio_tungstenite::tungstenite::handshake::server::Request, mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                let mut headers = request.headers().clone();
                headers.remove("sec-websocket-key");
                captured.lock().unwrap().push((request.uri().to_string(), headers));
                response.headers_mut().insert("x-request-id", HeaderValue::from_static("realtime-request"));
                Ok(response)
            }).await.unwrap();
            if native {
                let frame = ws.next().await.unwrap().unwrap().into_text().unwrap();
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&frame).unwrap()["type"],
                    "session.update"
                );
            } else {
                assert!(
                    tokio::time::timeout(Duration::from_millis(100), ws.next())
                        .await
                        .is_err(),
                    "raw connection injected a session frame"
                );
                ws.send(Message::Text(
                    "{\"type\":\"future.event\",\"extra\":true}".into(),
                ))
                .await
                .unwrap();
                ws.send(Message::Binary(Bytes::from_static(b"\0\xff")))
                    .await
                    .unwrap();
                assert_eq!(
                    ws.next().await.unwrap().unwrap(),
                    Message::Text("{\"type\":\"session.close\",\"future\":1}".into())
                );
            }
        }
    });
    let mut provider = provider(base.clone());
    provider
        .headers
        .insert("x-header-priority", HeaderValue::from_static("provider"));
    let client = RealtimeWebsocketClient::new(provider);
    let mut extra = HeaderMap::new();
    extra.insert("authorization", HeaderValue::from_static("Bearer upstream"));
    extra.insert(
        "chatgpt-account-id",
        HeaderValue::from_static("account-test"),
    );
    extra.insert("openai-alpha", HeaderValue::from_static("quicksilver=v1"));
    extra.insert("x-header-priority", HeaderValue::from_static("extra"));
    let mut defaults = HeaderMap::new();
    defaults.insert("x-header-priority", HeaderValue::from_static("default"));
    defaults.insert("originator", HeaderValue::from_static("codex_cli_rs"));
    let native = client
        .connect(config(), extra.clone(), defaults.clone())
        .await
        .unwrap();
    drop(native);
    extra.insert("x-session-id", HeaderValue::from_static("session-test"));
    let (mut raw, response) = client
        .connect_raw(
            &format!("{base}?intent=quicksilver&model=test-model"),
            extra,
            defaults,
        )
        .await
        .unwrap();
    assert_eq!(response.headers()["x-request-id"], "realtime-request");
    assert_eq!(
        raw.next().await.unwrap().unwrap(),
        Message::Text("{\"type\":\"future.event\",\"extra\":true}".into())
    );
    assert_eq!(
        raw.next().await.unwrap().unwrap(),
        Message::Binary(Bytes::from_static(b"\0\xff"))
    );
    raw.send(Message::Text(
        "{\"type\":\"session.close\",\"future\":1}".into(),
    ))
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
    let captures = captures.lock().unwrap();
    assert_eq!(captures.len(), 2);
    assert_eq!(captures[0], captures[1]);
    assert_eq!(captures[0].1["x-header-priority"], "extra");
}

#[tokio::test]
async fn raw_realtime_preserves_handshake_error_status_headers_and_body() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::path("/v1/realtime"))
        .respond_with(
            wiremock::ResponseTemplate::new(429)
                .insert_header("retry-after", "7")
                .set_body_raw("{\"error\":{\"code\":\"quota\"}}", "application/json"),
        )
        .mount(&server)
        .await;
    let client = RealtimeWebsocketClient::new(provider(server.uri()));
    let error = client
        .connect_raw(
            &format!("{}/v1/realtime", server.uri()),
            HeaderMap::new(),
            HeaderMap::new(),
        )
        .await
        .unwrap_err();
    let codex_api::ApiError::Transport(TransportError::Http {
        status,
        headers,
        body,
        ..
    }) = error
    else {
        panic!("expected complete HTTP error");
    };
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(headers.unwrap()["retry-after"], "7");
    assert_eq!(body.unwrap(), "{\"error\":{\"code\":\"quota\"}}");
}

#[tokio::test]
async fn raw_guardian_preserves_query_duplicates_and_lexical_encoding() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();
        config.extensions.permessage_deflate = Some(Default::default());
        let _socket = tokio_tungstenite::accept_hdr_async_with_config(stream, |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response: tokio_tungstenite::tungstenite::handshake::server::Response| {
            assert_eq!(request.uri().path_and_query().unwrap().as_str(), "/v1/guardian?api-version=pinned&duplicate=one&duplicate=two&encoded=%2f%252F&space=a+b&empty=");
            assert_eq!(request.headers()["authorization"], "Bearer upstream");
            assert!(request.headers().contains_key("sec-websocket-extensions"), "Guardian must retain the original Responses compression handshake");
            Ok(response)
        }, Some(config)).await.unwrap();
    });
    let mut provider = provider(base);
    provider.query_params = Some(std::collections::HashMap::from([(
        "api-version".into(),
        "pinned".into(),
    )]));
    let client = codex_api::ResponsesWebsocketClient::new(provider, Arc::new(Auth))
        .with_endpoint(codex_api::ResponsesEndpoint::Guardian);
    let factory = codex_http_client::HttpClientFactory::new(
        codex_http_client::OutboundProxyPolicy::ReqwestDefault,
    );
    let _socket = client
        .connect_raw_with_query(
            &factory,
            HeaderMap::new(),
            HeaderMap::new(),
            Some("duplicate=one&duplicate=two&encoded=%2f%252F&space=a+b&empty="),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
}
