//! A real authenticated gateway on an ephemeral loopback port, owned by its test.
use codex_api_gateway::{
    server::{Gateway, router},
    settings::load_backend,
};
use std::{path::PathBuf, sync::Arc, time::Duration};

pub struct LiveGateway {
    pub url: String,
    pub key: String,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl LiveGateway {
    pub async fn start() -> Self {
        if let Ok(url) = std::env::var("CODEX_GATEWAY_LIVE_URL") {
            return Self {
                url: url.trim_end_matches('/').into(),
                key: std::env::var("CODEX_GATEWAY_API_KEY")
                    .expect("external gateway needs CODEX_GATEWAY_API_KEY"),
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
        let app = router(Gateway {
            backend,
            models: codex_models_manager::bundled_models_response()
                .unwrap()
                .models,
            key: Some(key.clone()),
            concurrency: Arc::new(tokio::sync::Semaphore::new(4)),
            timeout: Duration::from_secs(300),
            transfers: None,
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url,
            key,
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
