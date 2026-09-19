use super::*;
use axum::{Json, Router, http::StatusCode, routing::post};
use base64::Engine;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Issuer {
    url: String,
    task: tokio::task::JoinHandle<()>,
}

impl Issuer {
    async fn start(router: Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self { url, task }
    }
}

impl Drop for Issuer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn config(home: &std::path::Path) -> AuthConfig {
    AuthConfig {
        codex_home: home.to_path_buf(),
        auth_credentials_store_mode: codex_login::AuthCredentialsStoreMode::File,
        keyring_backend_kind: codex_login::AuthKeyringBackendKind::Direct,
        forced_login_method: None,
        forced_chatgpt_workspace_id: None,
        chatgpt_base_url: None,
        managed_auth_policy: Default::default(),
        auth_route_config: AuthRouteConfig::from_http_client_factory(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        )),
    }
}

async fn user_code(Json(body): Json<Value>) -> Json<Value> {
    assert!(body["client_id"].is_string());
    Json(json!({"device_auth_id":"test-device", "user_code":"TEST-CODE", "interval":"0"}))
}

#[tokio::test]
async fn device_login_polls_saves_credentials_and_reuses_them_on_restart() {
    let home = tempfile::tempdir().unwrap();
    let polls = Arc::new(AtomicUsize::new(0));
    let poll_count = polls.clone();
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        json!({"https://api.openai.com/auth":{"chatgpt_account_id":"test-account","chatgpt_plan_type":"plus"}}).to_string(),
    );
    let id_token = format!("e30.{claims}.signature");
    let issuer = Issuer::start(
        Router::new()
            .route("/api/accounts/deviceauth/usercode", post(user_code))
            .route("/api/accounts/deviceauth/token", post(move |Json(body): Json<Value>| {
                let count = poll_count.fetch_add(1, Ordering::SeqCst);
                async move {
                    assert_eq!(body["device_auth_id"], "test-device");
                    assert_eq!(body["user_code"], "TEST-CODE");
                    if count == 0 {
                        (StatusCode::FORBIDDEN, Json(json!({})))
                    } else {
                        (StatusCode::OK, Json(json!({"authorization_code":"test-authorization", "code_challenge":"test-challenge", "code_verifier":"test-verifier"})))
                    }
                }
            }))
            .route("/oauth/token", post(move |body: String| async move {
                assert!(body.contains("code=test-authorization"));
                assert!(body.contains("code_verifier=test-verifier"));
                Json(json!({"id_token":id_token, "access_token":"test-access", "refresh_token":"test-refresh"}))
            })),
    ).await;
    let mut opts = device_login_options(&config(home.path())).unwrap();
    opts.issuer = issuer.url.clone();
    login_with_timeout(opts, Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(polls.load(Ordering::SeqCst), 2);
    let path = home.path().join("auth.json");
    let saved = std::fs::read(&path).unwrap();
    let auth: Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(auth["tokens"]["refresh_token"], "test-refresh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    drop(issuer);
    let backend = load_backend_with_device_login(home.path().to_path_buf())
        .await
        .unwrap();
    assert!(backend.auth.auth_cached().unwrap().is_chatgpt_auth());
    assert_eq!(std::fs::read(path).unwrap(), saved);
}

#[tokio::test]
async fn device_login_deadline_cancels_pending_authorization_without_saving() {
    let home = tempfile::tempdir().unwrap();
    let issuer = Issuer::start(
        Router::new()
            .route("/api/accounts/deviceauth/usercode", post(user_code))
            .route(
                "/api/accounts/deviceauth/token",
                post(|| async { std::future::pending::<StatusCode>().await }),
            ),
    )
    .await;
    let mut opts = device_login_options(&config(home.path())).unwrap();
    opts.issuer = issuer.url.clone();
    let error = tokio::time::timeout(
        Duration::from_secs(5),
        login_with_timeout(opts, Duration::from_millis(200)),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(error.to_string().contains("timed out"), "{error:#}");
    assert!(!home.path().join("auth.json").exists());
}

#[tokio::test]
async fn device_login_reports_server_failure_without_saving() {
    let home = tempfile::tempdir().unwrap();
    let issuer = Issuer::start(Router::new().route(
        "/api/accounts/deviceauth/usercode",
        post(|| async { StatusCode::NOT_FOUND }),
    ))
    .await;
    let mut opts = device_login_options(&config(home.path())).unwrap();
    opts.issuer = issuer.url.clone();
    let error = login_with_timeout(opts, Duration::from_secs(5))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("not enabled"));
    assert!(!home.path().join("auth.json").exists());
}

#[test]
fn device_login_respects_login_policy_and_workspace_restrictions() {
    let home = tempfile::tempdir().unwrap();
    let mut config = config(home.path());
    config.forced_chatgpt_workspace_id = Some(vec!["workspace".into()]);
    let opts = device_login_options(&config).unwrap();
    assert_eq!(
        opts.forced_chatgpt_workspace_id,
        config.effective_chatgpt_workspaces()
    );
    assert!(!opts.open_browser);
    config.forced_chatgpt_workspace_id = None;
    config.forced_login_method = Some(codex_protocol::config_types::ForcedLoginMethod::Api);
    assert!(device_login_options(&config).is_err());
}
