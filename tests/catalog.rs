use axum::{Json, Router, http::StatusCode, routing::get};
use codex_api_gateway::{
    backend::Backend,
    server::{Gateway, router},
};
use codex_http_client::{HttpClientFactory, OutboundProxyPolicy};
use codex_login::{AuthManager, CodexAuth};
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};

async fn backend(app: Router) -> (Backend, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let auth = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test-key"));
    (
        Backend {
            provider: create_model_provider(
                ModelProviderInfo::create_openai_provider(Some(url.clone())),
                Some(auth.clone()),
            ),
            auth,
            factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            chatgpt_base_url: url.clone(),
            platform_base_url: url.clone(),
            auth_base_url: url,
            subscription_only: false,
            compression: false,
            agent_identity_policy: codex_login::AgentIdentityAuthPolicy::JwtOnly,
        },
        task,
    )
}

#[tokio::test]
async fn startup_catalog_exposes_new_models_and_uses_their_capabilities() {
    let mut catalog = codex_models_manager::bundled_models_response().unwrap();
    let template = catalog
        .models
        .iter()
        .find(|m| !m.use_responses_lite)
        .unwrap()
        .clone();
    catalog.models = ["gpt-6-sol", "gpt-6-luna"]
        .into_iter()
        .map(|name| {
            let mut model = template.clone();
            model.slug = name.into();
            model
        })
        .collect();
    let expected = catalog.models.clone();
    let app = Router::new().route(
        "/models",
        get(move || {
            let catalog = catalog.clone();
            async move { Json(catalog) }
        }),
    );
    let (backend, upstream) = backend(app).await;
    let models = backend.model_catalog(Duration::from_secs(2)).await.unwrap();
    assert_eq!(models, expected);
    for model in &models {
        let request: codex_api_gateway::request::CreateResponse =
            serde_json::from_value(json!({"model":model.slug,"input":"hello"})).unwrap();
        let request = request.into_codex(model, "catalog-test").unwrap();
        assert_eq!(request.model, model.slug);
        assert_eq!(
            request.reasoning.unwrap().effort,
            model.default_reasoning_level
        );
    }
    let app = router(Gateway {
        backend,
        models,
        key: None,
        concurrency: Arc::new(tokio::sync::Semaphore::new(1)),
        timeout: Duration::from_secs(2),
        transfers: None,
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1/models", listener.local_addr().unwrap());
    let gateway = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let response: Value = reqwest::get(url).await.unwrap().json().await.unwrap();
    assert_eq!(
        response["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["gpt-6-sol", "gpt-6-luna"]
    );
    gateway.abort();
    upstream.abort();
}

#[tokio::test]
async fn startup_catalog_falls_back_on_rejected_empty_or_invalid_response() {
    let bundled = codex_models_manager::bundled_models_response()
        .unwrap()
        .models;
    for (status, body) in [
        (StatusCode::UNAUTHORIZED, json!({"error":"unauthorized"})),
        (StatusCode::OK, json!({"models":[]})),
        (StatusCode::OK, json!({"models":[{"slug":"incomplete"}]})),
    ] {
        let app = Router::new().route(
            "/models",
            get(move || {
                let body = body.clone();
                async move { (status, Json(body)) }
            }),
        );
        let (backend, task) = backend(app).await;
        assert_eq!(
            backend.model_catalog(Duration::from_secs(2)).await.unwrap(),
            bundled
        );
        task.abort();
    }
}

#[tokio::test]
async fn startup_catalog_timeout_does_not_block_startup() {
    let app = Router::new().route(
        "/models",
        get(|| async {
            std::future::pending::<()>().await;
            Json(json!({"models":[]}))
        }),
    );
    let (backend, task) = backend(app).await;
    let models = tokio::time::timeout(
        Duration::from_secs(1),
        backend.model_catalog(Duration::from_millis(20)),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        models,
        codex_models_manager::bundled_models_response()
            .unwrap()
            .models
    );
    task.abort();
}
