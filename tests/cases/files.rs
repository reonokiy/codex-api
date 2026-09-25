use super::*;
use axum::response::IntoResponse;

pub(super) async fn upstream_file(
    State(fake): State<Fake>,
    uri: axum::extract::OriginalUri,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    fake.http_bodies.lock().unwrap().push(bytes.to_vec());
    let value = json!({"path": uri.path_and_query().unwrap().as_str(), "body":serde_json::from_slice::<Value>(&bytes).ok()});
    fake.received.lock().unwrap().push((headers.clone(), value));
    if fake.hang {
        std::future::pending::<()>().await;
    }
    if fake.status != StatusCode::OK {
        return (
            fake.status,
            axum::Json(json!({"error":{"code":"file_rejected","message":"file quota exceeded"}})),
        )
            .into_response();
    }
    let host = headers["host"].to_str().unwrap();
    if uri.path() == "/files" {
        axum::Json(json!({"file_id":"file_test", "upload_url":format!("http://{host}/storage/file_test?sig=signed%2Bsecret")})).into_response()
    } else if uri.path().ends_with("/uploaded") {
        axum::Json(json!({"status":"success","download_url":format!("http://{host}/download/file_test"),"file_name":"canonical.txt","mime_type":"text/plain"})).into_response()
    } else {
        (StatusCode::CREATED, "stored").into_response()
    }
}

fn form(bytes: Vec<u8>) -> reqwest::multipart::Form {
    reqwest::multipart::Form::new()
        .text("purpose", "user_data")
        .part(
            "file",
            reqwest::multipart::Part::bytes(bytes)
                .file_name("../source.txt")
                .mime_str("text/plain")
                .unwrap(),
        )
}

#[tokio::test]
async fn files_multipart_uses_original_three_stage_upload_without_auth_on_blob() {
    let h = Harness::with_auth(
        vec![],
        StatusCode::OK,
        false,
        Duration::from_secs(5),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    )
    .await;
    let content = b"file data\0\xff".to_vec();
    let auth = codex_model_provider::auth_provider_from_auth(
        &CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let pool = codex_http_client::RouteAwareClientPool::new_without_request_logging(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        codex_http_client::ClientRouteClass::Api,
    )
    .with_legacy_custom_ca_fallback();
    let direct_bytes = content.clone();
    let original = codex_api::upload_openai_file(
        &h.upstream_url,
        auth.as_ref(),
        &pool,
        "../source.txt".to_owned(),
        content.len() as u64,
        || {
            let bytes = Bytes::from(direct_bytes.clone());
            async move { Ok(futures::stream::once(async move { Ok(bytes) })) }
        },
        None,
    )
    .await
    .unwrap();
    let response = reqwest::Client::new()
        .post(format!("{}/v1/files", h.url))
        .bearer_auth("client-key")
        .multipart(form(content.clone()))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.json::<Value>().await.unwrap();
    assert_eq!(body["id"], original.file_id);
    assert_eq!(body["filename"], original.file_name);
    assert_eq!(body["bytes"], original.file_size_bytes);
    assert_eq!(body["object"], "file");
    assert_eq!(body["purpose"], "user_data");
    assert_eq!(body["status"], "processed");
    assert!(body["created_at"].as_u64().unwrap() > 0);
    assert!(body.get("download_url").is_none());
    let received = h.fake.received.lock().unwrap();
    assert_eq!(received.len(), 6);
    let mut direct = received[..3].to_vec();
    let mut proxied = received[3..].to_vec();
    for requests in [&mut direct, &mut proxied] {
        let upload_headers = &mut requests[1].0;
        assert!(!upload_headers.contains_key("authorization"));
        assert!(!upload_headers.contains_key("chatgpt-account-id"));
        assert_eq!(upload_headers["x-ms-blob-type"], "BlockBlob");
        assert!(
            uuid::Uuid::parse_str(upload_headers["x-ms-client-request-id"].to_str().unwrap())
                .is_ok()
        );
        upload_headers.remove("x-ms-client-request-id");
        for index in [0, 2] {
            assert!(requests[index].0.contains_key("authorization"));
            assert!(requests[index].0.contains_key("chatgpt-account-id"));
        }
    }
    assert_eq!(proxied, direct);
    assert_eq!(
        direct[0].1["body"],
        json!({"file_name":"../source.txt","file_size":content.len(),"use_case":"codex"})
    );
    assert_eq!(
        direct[1].1["path"],
        "/storage/file_test?sig=signed%2Bsecret"
    );
    assert_eq!(direct[2].1["body"], json!({}));
    let bodies = h.fake.http_bodies.lock().unwrap();
    assert_eq!(&bodies[..3], &bodies[3..]);
    assert_eq!(bodies[1], content);
}

#[tokio::test]
async fn files_validate_authorization_multipart_purpose_and_size_before_uploading() {
    let h = Harness::new(vec![], StatusCode::OK, false, Duration::from_secs(5)).await;
    let client = reqwest::Client::new();
    let url = format!("{}/v1/files", h.url);
    assert_eq!(
        client
            .post(&url)
            .multipart(form(vec![1]))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    for invalid in [
        reqwest::multipart::Form::new()
            .text("purpose", "assistants")
            .part(
                "file",
                reqwest::multipart::Part::bytes(vec![1]).file_name("x.txt"),
            ),
        reqwest::multipart::Form::new().text("purpose", "user_data"),
        form(vec![1]).text("purpose", "user_data"),
        form(vec![1]).text("expires_after[seconds]", "1000"),
    ] {
        let response = client
            .post(&url)
            .bearer_auth("client-key")
            .multipart(invalid)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let oversized = client
        .post(&url)
        .bearer_auth("client-key")
        .multipart(form(vec![0; 16 * 1024 * 1024]))
        .send()
        .await
        .unwrap();
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(h.fake.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn files_preserve_reservation_errors_without_replay_and_release_timeout_slots() {
    let h = Harness::new(
        vec![],
        StatusCode::TOO_MANY_REQUESTS,
        false,
        Duration::from_secs(5),
    )
    .await;
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/files", h.url))
        .bearer_auth("client-key")
        .multipart(form(vec![1]))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({"error":{"code":"file_rejected","message":"file quota exceeded"}})
    );
    assert_eq!(h.fake.received.lock().unwrap().len(), 1);
    let h = Harness::new(vec![], StatusCode::OK, true, Duration::from_millis(50)).await;
    for _ in 0..2 {
        assert_eq!(
            client
                .post(format!("{}/v1/files", h.url))
                .bearer_auth("client-key")
                .multipart(form(vec![1]))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::GATEWAY_TIMEOUT
        );
    }
}

#[tokio::test]
async fn native_files_rewrite_signed_uploads_and_allow_anonymous_binary_transfers() {
    let h = Harness::with_options(
        vec![],
        StatusCode::OK,
        false,
        Duration::from_secs(5),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        true,
    )
    .await;
    let client = reqwest::Client::new();
    let content = b"native file\0\xff".to_vec();
    let reservation = client
        .post(format!("{}/backend-api/files", h.url))
        .bearer_auth("client-key")
        .json(&json!({"file_name":"native.txt","file_size":content.len(),"use_case":"codex"}))
        .send()
        .await
        .unwrap();
    assert_eq!(reservation.status(), StatusCode::OK);
    let reservation = reservation.json::<Value>().await.unwrap();
    assert_eq!(reservation["file_id"], "file_test");
    let upload_url = reservation["upload_url"].as_str().unwrap();
    let handle = upload_url
        .strip_prefix(&format!("{}/transfers/", h.url))
        .unwrap();
    assert_eq!(handle.len(), 32);
    assert!(handle.chars().all(|ch| ch.is_ascii_hexdigit()));
    assert!(!upload_url.contains("sig="));
    assert!(!upload_url.contains("secret"));

    // Original Codex follows the signed URL without gateway authentication. A retry carrying
    // unrelated client credentials must still reach storage without forwarding those credentials.
    for with_credentials in [false, true] {
        let mut request = client
            .put(upload_url)
            .header("content-type", "application/octet-stream")
            .header("x-ms-blob-type", "BlockBlob")
            .body(content.clone());
        if with_credentials {
            request = request
                .bearer_auth("caller-secret")
                .header("cookie", "session=caller-cookie")
                .header("x-codex-gateway-authorization", "Bearer client-key")
                .header("chatgpt-account-id", "caller-account");
        }
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.bytes().await.unwrap().as_ref(), b"stored");
    }
    let finalized = client
        .post(format!("{}/backend-api/files/file_test/uploaded", h.url))
        .bearer_auth("client-key")
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(finalized.status(), StatusCode::OK);
    let finalized = finalized.json::<Value>().await.unwrap();
    assert_eq!(finalized["status"], "success");
    // Hosted tools fetch this URL themselves; only the caller's upload URL belongs on the gateway.
    let download_url = finalized["download_url"].as_str().unwrap();
    assert!(download_url.ends_with("/download/file_test"));
    assert!(!download_url.starts_with(&h.url));

    let received = h.fake.received.lock().unwrap();
    assert_eq!(received.len(), 4);
    for index in [0, 3] {
        assert!(received[index].0.contains_key("authorization"));
        assert!(received[index].0.contains_key("chatgpt-account-id"));
    }
    for index in [1, 2] {
        let (headers, request) = &received[index];
        assert_eq!(request["path"], "/storage/file_test?sig=signed%2Bsecret");
        assert_eq!(headers["content-type"], "application/octet-stream");
        assert_eq!(headers["x-ms-blob-type"], "BlockBlob");
        for name in [
            "authorization",
            "cookie",
            "x-codex-gateway-authorization",
            "chatgpt-account-id",
        ] {
            assert!(!headers.contains_key(name));
        }
    }
    let bodies = h.fake.http_bodies.lock().unwrap();
    assert_eq!(bodies[1], content);
    assert_eq!(bodies[2], content);
}
