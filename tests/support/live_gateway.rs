//! A real authenticated gateway on an ephemeral loopback port, owned by its test.
use codex_api_gateway::{
    server::{Gateway, router},
    settings::load_backend,
};
use std::{path::PathBuf, sync::Arc, time::Duration};

pub struct LiveGateway {
    pub url: String,
    pub key: String,
    pub transports: Option<Arc<Transports>>,
    pub model_catalog: Option<Vec<codex_protocol::openai_models::ModelInfo>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Default)]
pub struct Transports {
    pub http: std::sync::atomic::AtomicUsize,
    pub websocket: std::sync::atomic::AtomicUsize,
}

impl LiveGateway {
    pub async fn start() -> Self {
        if let Ok(url) = std::env::var("CODEX_GATEWAY_LIVE_URL") {
            return Self {
                url: url.trim_end_matches('/').into(),
                key: std::env::var("CODEX_GATEWAY_API_KEY")
                    .expect("external gateway needs CODEX_GATEWAY_API_KEY"),
                transports: None,
                model_catalog: None,
                task: None,
            };
        }
        let home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(
                    std::env::var_os("HOME")
                        .or_else(|| std::env::var_os("USERPROFILE"))
                        .expect("set CODEX_HOME or HOME"),
                )
                .join(".codex")
            });
        let backend = load_backend(home)
            .await
            .expect("real E2E requires codex login; see docs/e2e.md");
        let key = uuid::Uuid::new_v4().to_string();
        let models = backend
            .model_catalog(Duration::from_secs(30))
            .await
            .unwrap();
        let transports = Arc::new(Transports::default());
        let model_catalog = Some(models.clone());
        let observed = transports.clone();
        let app = router(Gateway {
            backend,
            models,
            key: Some(key.clone()),
            concurrency: Arc::new(tokio::sync::Semaphore::new(4)),
            timeout: Duration::from_secs(300),
            transfers: None,
        })
        .layer(axum::middleware::from_fn(
            move |request: axum::extract::Request, next: axum::middleware::Next| {
                let observed = observed.clone();
                async move {
                    let responses = request.uri().path().ends_with("/responses");
                    let method = request.method().clone();
                    let response = next.run(request).await;
                    if responses {
                        use std::sync::atomic::Ordering;
                        if method == http::Method::POST && response.status().is_success() {
                            observed.http.fetch_add(1, Ordering::Relaxed);
                        } else if method == http::Method::GET
                            && response.status() == http::StatusCode::SWITCHING_PROTOCOLS
                        {
                            observed.websocket.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    response
                }
            },
        ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url,
            key,
            transports: Some(transports),
            model_catalog,
            task: Some(task),
        }
    }
}

impl Drop for LiveGateway {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
