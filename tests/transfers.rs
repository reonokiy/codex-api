use axum::{
    Router,
    body::to_bytes,
    extract::{Path, Request, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
};
use codex_api_gateway::{error, transfers::Transfers};
use codex_http_client::{HttpClientFactory, OutboundProxyPolicy};
use serde_json::json;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

type RecordedTransfer = (Method, String, HeaderMap, Vec<u8>);

#[derive(Clone, Default)]
struct Storage(Arc<Mutex<Vec<RecordedTransfer>>>);

async fn storage(State(state): State<Storage>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, 1024 * 1024).await.unwrap();
    state.0.lock().unwrap().push((
        parts.method.clone(),
        parts.uri.to_string(),
        parts.headers,
        bytes.to_vec(),
    ));
    let (status, body) = if parts.uri.path() == "/failure" {
        (StatusCode::CONFLICT, b"storage conflict".to_vec())
    } else if parts.method == Method::PUT {
        (StatusCode::CREATED, bytes.to_vec())
    } else {
        (StatusCode::PARTIAL_CONTENT, b"bundle bytes\0\xff".to_vec())
    };
    let mut response = (status, body).into_response();
    response
        .headers_mut()
        .insert("etag", "\"original-etag\"".parse().unwrap());
    response
        .headers_mut()
        .insert("x-ms-request-id", "request-123".parse().unwrap());
    response
        .headers_mut()
        .insert("content-range", "bytes 0-13/20".parse().unwrap());
    response
}

async fn transfer(
    State(store): State<Arc<Transfers>>,
    Path(handle): Path<String>,
    request: Request,
) -> Result<Response, error::GatewayError> {
    store.serve(&handle, request).await
}

struct Harness {
    store: Arc<Transfers>,
    storage: Storage,
    upstream: String,
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
    async fn new(ttl: Duration, capacity: usize) -> Self {
        let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_url = format!("http://{}", upstream.local_addr().unwrap());
        let gateway = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let public_url = format!("http://{}", gateway.local_addr().unwrap());
        let storage_state = Storage::default();
        let router = Router::new()
            .fallback(storage)
            .with_state(storage_state.clone());
        let upstream_task = tokio::spawn(async move {
            axum::serve(upstream, router).await.unwrap();
        });
        let store = Arc::new(
            Transfers::with_limits(
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                &public_url,
                Duration::from_secs(2),
                ttl,
                capacity,
            )
            .unwrap(),
        );
        let router = Router::new()
            .route("/transfers/{handle}", any(transfer))
            .with_state(store.clone());
        let gateway_task = tokio::spawn(async move {
            axum::serve(gateway, router).await.unwrap();
        });
        Self {
            store,
            storage: storage_state,
            upstream: upstream_url,
            tasks: vec![upstream_task, gateway_task],
        }
    }
    fn upload(&self, target: &str) -> String {
        let mut response =
            json!({"file_id":"file-123","upload_url":format!("{}{target}",self.upstream)});
        assert!(
            self.store
                .rewrite_response(&Method::POST, "/backend-api/files", &mut response)
                .unwrap()
        );
        response["upload_url"].as_str().unwrap().to_owned()
    }
}

#[tokio::test]
async fn signed_transfers_preserve_storage_bytes_headers_status_and_queries_without_credentials() {
    let h = Harness::new(Duration::from_secs(30), 10).await;
    let upload_url = h.upload("/upload?sig=private%2Bsignature&order=2&order=1");
    assert!(!upload_url.contains("signature"));
    assert!(!upload_url.contains("upload?"));
    let token = upload_url.rsplit('/').next().unwrap();
    assert_eq!(token.len(), 32);
    assert!(token.bytes().all(|b| b.is_ascii_hexdigit()));
    let client = reqwest::Client::new();
    let response = client
        .put(&upload_url)
        .header("authorization", "Bearer must-not-forward")
        .header("cookie", "session=must-not-forward")
        .header("x-gateway-api-key", "must-not-forward")
        .header("x-openai-actor-authorization", "must-not-forward")
        .header("chatgpt-account-id", "must-not-forward")
        .header("x-codex-session-id", "must-not-forward")
        .header("connection", "x-drop-me")
        .header("x-drop-me", "must-not-forward")
        .header("x-ms-blob-type", "BlockBlob")
        .header("content-type", "application/gzip")
        .body(b"\0archive bytes\xff".to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["etag"], "\"original-etag\"");
    assert_eq!(response.headers()["x-ms-request-id"], "request-123");
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        b"\0archive bytes\xff"
    );
    let mut catalog = json!({"plugins":[{"id":"plugin-1","release":{"bundle_download_url":format!("{}/download?sig=private",h.upstream),"app_manifest":{"bundle_download_url":"https://attacker.example/"}}}],"pagination":{"next_page_token":null}});
    assert!(
        h.store
            .rewrite_response(
                &Method::GET,
                "/backend-api/ps/plugins/installed",
                &mut catalog
            )
            .unwrap()
    );
    assert_eq!(
        catalog["plugins"][0]["release"]["app_manifest"]["bundle_download_url"],
        "https://attacker.example/"
    );
    let download_url = catalog["plugins"][0]["release"]["bundle_download_url"]
        .as_str()
        .unwrap();
    let response = client
        .get(download_url)
        .header("range", "bytes=0-13")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()["content-range"], "bytes 0-13/20");
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        b"bundle bytes\0\xff"
    );
    let seen = h.storage.0.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].1, "/upload?sig=private%2Bsignature&order=2&order=1");
    assert_eq!(seen[0].2["x-ms-blob-type"], "BlockBlob");
    assert_eq!(seen[0].3, b"\0archive bytes\xff");
    for name in [
        "authorization",
        "cookie",
        "x-gateway-api-key",
        "x-openai-actor-authorization",
        "chatgpt-account-id",
        "x-codex-session-id",
        "x-drop-me",
    ] {
        assert!(!seen[0].2.contains_key(name), "leaked {name}");
    }
    assert_eq!(seen[1].1, "/download?sig=private");
    assert_eq!(seen[1].2["range"], "bytes=0-13");
    assert!(
        !seen[1].2.contains_key("transfer-encoding"),
        "Codex GET downloads have no streaming request body"
    );
}

#[tokio::test]
async fn capabilities_enforce_methods_expiry_capacity_and_do_not_register_caller_urls() {
    let h = Harness::new(Duration::from_secs(30), 1).await;
    let upload = h.upload("/upload?sig=private");
    let client = reqwest::Client::new();
    assert_eq!(
        client.get(&upload).send().await.unwrap().status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
    let unknown = format!("{}unknown", upload);
    assert_eq!(
        client.put(&unknown).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    assert!(h.storage.0.lock().unwrap().is_empty());
    let candidate =
        json!({"file_id":"file-2","upload_url":"https://signed.example/upload?sig=private"});
    let mut wrong_path = candidate.clone();
    assert!(
        !h.store
            .rewrite_response(
                &Method::POST,
                "/backend-api/codex/responses",
                &mut wrong_path
            )
            .unwrap()
    );
    assert_eq!(wrong_path, candidate);
    let mut too_many = candidate.clone();
    assert_eq!(
        h.store
            .rewrite_response(&Method::POST, "/backend-api/files", &mut too_many)
            .unwrap_err()
            .status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(too_many, candidate);
    let h = Harness::new(Duration::from_millis(20), 1).await;
    let upload = h.upload("/upload?sig=private");
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        client.put(&upload).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    assert!(
        h.store
            .rewrite_response(&Method::POST, "/backend-api/files", &mut too_many)
            .unwrap()
    );
    assert_ne!(too_many["upload_url"], upload);
}

#[tokio::test]
async fn transfers_reject_invalid_destinations_atomically_and_preserve_upstream_errors() {
    let h = Harness::new(Duration::from_secs(30), 3).await;
    let mut catalog = json!({"plugins":[
        {"id":"valid","release":{"bundle_download_url":format!("{}/download",h.upstream)}},
        {"id":"bad","release":{"bundle_download_url":"file:///etc/passwd"}}
    ]});
    let before = catalog.clone();
    assert!(
        h.store
            .rewrite_response(&Method::GET, "/backend-api/ps/plugins/list", &mut catalog)
            .is_err()
    );
    assert_eq!(catalog, before);
    for target in [
        "http://example.com/storage",
        "https://user:secret@example.com/x",
        "https://example.com/x#secret",
    ] {
        let mut body = json!({"file_id":"file","upload_url":target});
        assert!(
            h.store
                .rewrite_response(&Method::POST, "/backend-api/files", &mut body)
                .is_err()
        );
        assert_eq!(body["upload_url"], target);
    }
    let url = h.upload("/failure");
    let response = reqwest::Client::new()
        .put(url)
        .body("bytes")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(response.text().await.unwrap(), "storage conflict");
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    assert!(
        Transfers::new(
            factory.clone(),
            "https://gateway.example/prefix",
            Duration::from_secs(5)
        )
        .is_ok()
    );
    assert!(
        Transfers::new(
            factory,
            "https://gateway.example/?key=secret",
            Duration::from_secs(5)
        )
        .is_err()
    );
}
