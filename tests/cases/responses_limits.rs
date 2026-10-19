use super::*;

const HTTP_LIMIT: usize = 64 * 1024 * 1024;

#[tokio::test]
async fn native_http_preserves_done_items_when_terminal_output_is_empty() {
    let mut fixture = events();
    *fixture.last_mut().unwrap() = completed(vec![]);
    let h = Harness::new(
        fixture.clone(),
        StatusCode::OK,
        false,
        Duration::from_secs(5),
    )
    .await;
    let response = reqwest::Client::new()
        .post(format!("{}/backend-api/codex/responses", h.url))
        .bearer_auth("client-key")
        .json(&json!({"model":h.model,"stream":true,"input":[]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(parse_sse(&response.text().await.unwrap()), fixture);
}

#[tokio::test]
async fn responses_accept_large_images_over_http_with_identity_and_zstd() {
    for path in [
        "/v1/responses",
        "/codex/responses",
        "/backend-api/codex/responses",
    ] {
        for compressed in [false, true] {
            let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(30)).await;
            let image = format!("data:image/png;base64,{}", "A".repeat(17 * 1024 * 1024));
            let body = json!({"model":h.model,"stream":true,"input":[{
                "type":"message","role":"user","content":[{"type":"input_image","image_url":image}]
            }]});
            let wire = serde_json::to_vec(&body).unwrap();
            let mut request = reqwest::Client::new()
                .post(format!("{}{path}", h.url))
                .bearer_auth("client-key")
                .header("content-type", "application/json");
            let wire = if compressed {
                request = request.header("content-encoding", "zstd");
                zstd::encode_all(wire.as_slice(), 1).unwrap()
            } else {
                wire
            };
            let response = request.body(wire).send().await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "{path}, compressed={compressed}"
            );
            assert!(
                response
                    .text()
                    .await
                    .unwrap()
                    .contains("response.completed")
            );
            let received = h.fake.received.lock().unwrap();
            assert_eq!(received.len(), 1);
            assert!(received[0].1["input"][0]["content"][0]["image_url"] == image);
        }
    }
}

#[tokio::test]
async fn responses_http_enforces_wire_and_decoded_boundaries() {
    for path in [
        "/v1/responses",
        "/codex/responses",
        "/backend-api/codex/responses",
    ] {
        for compressed in [false, true] {
            let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(30)).await;
            for size in [HTTP_LIMIT, HTTP_LIMIT + 1] {
                let mut wire = serde_json::to_vec(&json!({"model":h.model,"stream":true,"input":[{
                    "type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]
                }]}))
                .unwrap();
                wire.resize(size, b' ');
                let mut request = reqwest::Client::new()
                    .post(format!("{}{path}", h.url))
                    .bearer_auth("client-key")
                    .header("content-type", "application/json");
                let wire = if compressed {
                    request = request.header("content-encoding", "zstd");
                    zstd::encode_all(wire.as_slice(), 1).unwrap()
                } else {
                    wire
                };
                let response = request.body(wire).send().await.unwrap();
                let expected = if size == HTTP_LIMIT {
                    StatusCode::OK
                } else {
                    StatusCode::PAYLOAD_TOO_LARGE
                };
                assert_eq!(
                    response.status(),
                    expected,
                    "{path}, compressed={compressed}, size={size}"
                );
                let text = response.text().await.unwrap();
                if size == HTTP_LIMIT {
                    assert!(text.contains("response.completed"));
                }
            }
            assert_eq!(h.fake.received.lock().unwrap().len(), 1);
        }
    }
}

#[tokio::test]
async fn responses_preserves_public_metadata_without_accepting_continuation_or_unknown_fields() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let metadata = json!({"trace":"keep","ws_request_header_x_codex_turn_metadata":"turn"});
    let response = h
        .request(json!({"input":"hello","client_metadata":metadata}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<Value>().await.unwrap()["status"],
        "completed"
    );
    assert_eq!(
        h.fake.received.lock().unwrap()[0].1["client_metadata"],
        metadata
    );
    for field in [
        json!({"client_metadata":{"trace":1}}),
        json!({"client_metadata":null}),
        json!({"previous_response_id":"resp_1"}),
        json!({"stream_id":"lane"}),
        json!({"unknown_field":true}),
    ] {
        let mut body = field;
        body["input"] = json!("hello");
        let response = h.request(body).send().await.unwrap();
        assert!(response.status().is_client_error());
    }
    assert_eq!(h.fake.received.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn responses_compression_validates_encoding_and_public_content_type() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    for path in [
        "/v1/responses",
        "/codex/responses",
        "/backend-api/codex/responses",
    ] {
        for (encoding, status) in [
            ("gzip", StatusCode::UNSUPPORTED_MEDIA_TYPE),
            ("zstd", StatusCode::BAD_REQUEST),
        ] {
            let response = reqwest::Client::new()
                .post(format!("{}{path}", h.url))
                .bearer_auth("client-key")
                .header("content-type", "application/json")
                .header("content-encoding", encoding)
                .body("invalid compressed body")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), status);
        }
    }
    let response = reqwest::Client::new()
        .post(format!("{}/v1/responses", h.url))
        .bearer_auth("client-key")
        .body(json!({"model":h.model,"input":"hello"}).to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(h.fake.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn responses_rejects_truncated_zstd_and_oversized_compressed_wire() {
    use std::io::Write;

    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    let body = serde_json::to_vec(&json!({"model":h.model,"stream":true,"input":"hello"})).unwrap();
    let mut encoder = zstd::Encoder::new(Vec::new(), 1).unwrap();
    encoder.include_checksum(true).unwrap();
    encoder.write_all(&body).unwrap();
    let wire = encoder.finish().unwrap();
    let mut truncated = wire.clone();
    truncated.pop();
    // Skippable padding grows only wire bytes, isolating the compressed ingress cap.
    let mut oversized = wire;
    oversized.extend_from_slice(&0x184D_2A50u32.to_le_bytes());
    let padding_len = HTTP_LIMIT + 1 - oversized.len() - 4;
    oversized.extend_from_slice(&(padding_len as u32).to_le_bytes());
    oversized.resize(HTTP_LIMIT + 1, 0);
    assert_eq!(zstd::decode_all(oversized.as_slice()).unwrap(), body);
    for path in [
        "/v1/responses",
        "/codex/responses",
        "/backend-api/codex/responses",
    ] {
        for (wire, status) in [
            (&truncated, StatusCode::BAD_REQUEST),
            (&oversized, StatusCode::PAYLOAD_TOO_LARGE),
        ] {
            let response = reqwest::Client::new()
                .post(format!("{}{path}", h.url))
                .bearer_auth("client-key")
                .header("content-type", "application/json")
                .header("content-encoding", "zstd")
                .body(wire.clone())
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), status, "{path}");
        }
    }
    assert!(h.fake.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn compact_rejects_response_metadata_including_null() {
    let h = Harness::new(
        vec![completed(vec![
            json!({"type":"compaction","id":"cmp_1","encrypted_content":"fixture"}),
        ])],
        StatusCode::OK,
        false,
        Duration::from_secs(5),
    )
    .await;
    for path in ["/v1/responses/compact", "/codex/responses/compact"] {
        for metadata in [json!({"trace":"keep"}), json!({}), Value::Null] {
            let response = reqwest::Client::new()
                .post(format!("{}{path}", h.url))
                .bearer_auth("client-key")
                .json(&json!({"model":h.model,"input":"hello","client_metadata":metadata}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        }
        let duplicated = format!(
            r#"{{"model":"{}","input":"hello","input":"duplicate"}}"#,
            h.model
        );
        let response = reqwest::Client::new()
            .post(format!("{}{path}", h.url))
            .bearer_auth("client-key")
            .header("content-type", "application/json")
            .body(duplicated)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert!(h.fake.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn responses_larger_budget_does_not_expand_unrelated_endpoints() {
    let h = Harness::new(events(), StatusCode::OK, false, Duration::from_secs(5)).await;
    for (path, status) in [
        ("/v1/responses/compact", StatusCode::BAD_REQUEST),
        ("/v1/images/generations", StatusCode::PAYLOAD_TOO_LARGE),
    ] {
        let response = reqwest::Client::new()
            .post(format!("{}{path}", h.url))
            .bearer_auth("client-key")
            .header("content-type", "application/json")
            .body(vec![b' '; 16 * 1024 * 1024 + 1])
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{path}");
        assert!(response.text().await.unwrap().contains("length limit"));
    }
    assert!(h.fake.received.lock().unwrap().is_empty());
}
