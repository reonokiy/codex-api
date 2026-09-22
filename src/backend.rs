use crate::{
    error::GatewayError,
    transport::{TapTransport, WireEvent},
};
use codex_api::{
    ApiError, Compression, ReqwestTransport, ResponsesApiRequest, ResponsesClient, ResponsesOptions,
};
use codex_http_client::{ClientRouteClass, HttpClientFactory};
use codex_login::{AuthManager, default_client::create_client_for_route};
use codex_model_provider::SharedModelProvider;
use codex_protocol::protocol::SessionSource;
use futures::StreamExt;
use std::sync::{Arc, OnceLock};
use tokio::sync::{mpsc, oneshot};

pub struct Backend {
    pub provider: SharedModelProvider,
    pub auth: Arc<AuthManager>,
    pub factory: HttpClientFactory,
    pub chatgpt_base_url: String,
    pub platform_base_url: String,
    pub auth_base_url: String,
    pub subscription_only: bool,
    pub compression: bool,
    pub agent_identity_policy: codex_login::AgentIdentityAuthPolicy,
}

pub struct RunningResponse {
    pub events: mpsc::Receiver<WireEvent>,
    pub finished: oneshot::Receiver<Result<(), String>>,
    pub headers: http::HeaderMap,
    observer: tokio::task::JoinHandle<()>,
}
impl Drop for RunningResponse {
    fn drop(&mut self) {
        self.observer.abort();
    }
}

impl Backend {
    pub async fn start(
        &self,
        request: ResponsesApiRequest,
        session_id: String,
    ) -> Result<RunningResponse, GatewayError> {
        self.start_inner(Some((request, session_id)), None).await
    }

    /// Use Codex's raw JSON entry point so native tools and future fields survive.
    pub async fn start_native(
        &self,
        body: Box<serde_json::value::RawValue>,
        headers: http::HeaderMap,
    ) -> Result<RunningResponse, GatewayError> {
        self.start_inner(None, Some((body, headers))).await
    }

    pub async fn connect_websocket(
        &self,
        headers: http::HeaderMap,
    ) -> Result<(codex_websocket_client::WebSocketConnection, http::HeaderMap), GatewayError> {
        self.connect_websocket_endpoint(headers, codex_api::ResponsesEndpoint::Responses, None)
            .await
    }

    pub async fn connect_websocket_endpoint(
        &self,
        headers: http::HeaderMap,
        endpoint: codex_api::ResponsesEndpoint,
        query: Option<&str>,
    ) -> Result<(codex_websocket_client::WebSocketConnection, http::HeaderMap), GatewayError> {
        let mut recovery = self.auth.unauthorized_recovery();
        let original = self.provider.auth().await.ok_or_else(GatewayError::auth)?;
        let account = (original.get_account_id(), original.get_chatgpt_user_id());
        let changes = self.auth.auth_change_receiver();
        let fallback = codex_model_provider::AgentIdentitySessionFallback::default();
        loop {
            let revision = *changes.borrow();
            let auth = self.provider.auth().await.ok_or_else(GatewayError::auth)?;
            if (self.subscription_only && !auth.is_chatgpt_auth())
                || (auth.get_account_id(), auth.get_chatgpt_user_id()) != account
            {
                return Err(GatewayError::auth());
            }
            let resolved = self
                .provider
                .api_provider()
                .await
                .map_err(GatewayError::internal)?;
            let api_auth = self
                .provider
                .api_auth_for_scope(codex_model_provider::ProviderAuthScope {
                    agent_identity_policy: self.agent_identity_policy,
                    session_source: SessionSource::Exec,
                    agent_identity_session_fallback: fallback.clone(),
                })
                .await
                .map_err(GatewayError::internal)?
                .auth;
            if *changes.borrow() != revision {
                continue;
            }
            let client = codex_api::ResponsesWebsocketClient::new(resolved, api_auth)
                .with_endpoint(endpoint);
            match client
                .connect_raw_with_query(
                    &self.factory,
                    headers.clone(),
                    codex_login::default_client::default_headers(),
                    query,
                )
                .await
            {
                Ok((socket, response)) => return Ok((socket, response.headers().clone())),
                Err(ApiError::Transport(ref e))
                    if self.provider.is_recoverable_auth_error(e) && recovery.has_next() =>
                {
                    recovery.next().await.map_err(|_| GatewayError::auth())?;
                }
                Err(e) => return Err(GatewayError::from_api(e)),
            }
        }
    }

    /// Resolve account model capabilities once at startup, with an offline fallback.
    pub async fn model_catalog(
        &self,
        timeout: std::time::Duration,
    ) -> anyhow::Result<Vec<codex_protocol::openai_models::ModelInfo>> {
        let fetch = async {
            let (body, _) = self
                .models(crate::CODEX_RELEASE, http::HeaderMap::new())
                .await?;
            let catalog: codex_protocol::openai_models::ModelsResponse =
                serde_json::from_slice(&body)?;
            anyhow::ensure!(
                !catalog.models.is_empty(),
                "upstream model catalog is empty"
            );
            Ok::<_, anyhow::Error>(catalog.models)
        };
        match tokio::time::timeout(timeout, fetch).await {
            Ok(Ok(models)) => {
                tracing::info!(count = models.len(), "Loaded account model catalog");
                return Ok(models);
            }
            Ok(Err(_)) => tracing::warn!("Model catalog unavailable; using bundled Codex models"),
            Err(_) => tracing::warn!("Model catalog timed out; using bundled Codex models"),
        }
        Ok(codex_models_manager::bundled_models_response()?.models)
    }

    pub async fn models(
        &self,
        version: &str,
        headers: http::HeaderMap,
    ) -> Result<(bytes::Bytes, Option<String>), GatewayError> {
        let mut recovery = self.auth.unauthorized_recovery();
        let original = self.provider.auth().await.ok_or_else(GatewayError::auth)?;
        let account = (original.get_account_id(), original.get_chatgpt_user_id());
        let changes = self.auth.auth_change_receiver();
        loop {
            let revision = *changes.borrow();
            let auth = self.provider.auth().await.ok_or_else(GatewayError::auth)?;
            if (self.subscription_only && !auth.is_chatgpt_auth())
                || (auth.get_account_id(), auth.get_chatgpt_user_id()) != account
            {
                return Err(GatewayError::auth());
            }
            let resolved = self
                .provider
                .api_provider()
                .await
                .map_err(GatewayError::internal)?;
            let api_auth = self
                .provider
                .api_auth_for_scope(codex_model_provider::ProviderAuthScope {
                    agent_identity_policy: self.agent_identity_policy,
                    session_source: SessionSource::Exec,
                    agent_identity_session_fallback: Default::default(),
                })
                .await
                .map_err(GatewayError::internal)?
                .auth;
            if *changes.borrow() != revision {
                continue;
            }
            let url = codex_api::ModelsClient::<ReqwestTransport>::request_url(&resolved, version);
            let http = create_client_for_route(&self.factory, &url, ClientRouteClass::Api)
                .map_err(GatewayError::internal)?;
            let catalog = Arc::new(OnceLock::new());
            let client = codex_api::ModelsClient::new(
                crate::transport::ModelCatalogTransport {
                    inner: ReqwestTransport::from_http_client(http),
                    body: catalog.clone(),
                },
                resolved,
                api_auth,
            );
            match client.list_models(url, headers.clone()).await {
                Ok((_, etag)) => {
                    return Ok((
                        catalog
                            .get()
                            .cloned()
                            .ok_or_else(|| GatewayError::internal("missing model catalog"))?,
                        etag,
                    ));
                }
                Err(ApiError::Transport(ref e))
                    if self.provider.is_recoverable_auth_error(e) && recovery.has_next() =>
                {
                    recovery.next().await.map_err(|_| GatewayError::auth())?;
                }
                Err(e) => return Err(GatewayError::from_api(e)),
            }
        }
    }

    /// Standalone tools use the same provider/auth entry points as Codex extensions.
    pub async fn standalone(
        &self,
        request: &crate::standalone::ToolRequest,
        headers: http::HeaderMap,
    ) -> Result<(http::HeaderMap, bytes::Bytes), GatewayError> {
        let mut recovery = self.auth.unauthorized_recovery();
        let original = self.provider.auth().await.ok_or_else(GatewayError::auth)?;
        let account = (original.get_account_id(), original.get_chatgpt_user_id());
        let changes = self.auth.auth_change_receiver();
        loop {
            let revision = *changes.borrow();
            let auth = self.provider.auth().await.ok_or_else(GatewayError::auth)?;
            if (self.subscription_only && !auth.is_chatgpt_auth())
                || (auth.get_account_id(), auth.get_chatgpt_user_id()) != account
            {
                return Err(GatewayError::auth());
            }
            let provider = self
                .provider
                .api_provider()
                .await
                .map_err(GatewayError::internal)?;
            let api_auth = self
                .provider
                .api_auth()
                .await
                .map_err(GatewayError::internal)?;
            if *changes.borrow() != revision {
                continue;
            }
            let http = create_client_for_route(
                &self.factory,
                &provider.url_for_path(request.path()),
                ClientRouteClass::Api,
            )
            .map_err(GatewayError::internal)?;
            let captured = Arc::new(OnceLock::new());
            let transport = crate::transport::StandaloneTransport {
                inner: ReqwestTransport::from_http_client(http),
                response: captured.clone(),
            };
            use crate::standalone::ToolRequest;
            let result = match request {
                ToolRequest::Generate(request) => {
                    codex_api::ImagesClient::new(transport, provider, api_auth)
                        .generate(request, headers.clone())
                        .await
                        .map(|_| ())
                }
                ToolRequest::Edit(request) => {
                    codex_api::ImagesClient::new(transport, provider, api_auth)
                        .edit(request, headers.clone())
                        .await
                        .map(|_| ())
                }
                ToolRequest::Search(request) => {
                    codex_api::SearchClient::new(transport, provider, api_auth)
                        .search(request, headers.clone())
                        .await
                        .map(|_| ())
                }
                ToolRequest::Memory(request) => {
                    codex_api::MemoriesClient::new(transport, provider, api_auth)
                        .summarize(request.clone(), headers.clone())
                        .await
                        .map(|_| ())
                }
            };
            match result {
                Ok(_) => {
                    return captured
                        .get()
                        .cloned()
                        .ok_or_else(|| GatewayError::internal("missing standalone tool response"));
                }
                Err(ApiError::Transport(ref error))
                    if self.provider.is_recoverable_auth_error(error) && recovery.has_next() =>
                {
                    recovery.next().await.map_err(|_| GatewayError::auth())?;
                }
                Err(error) => return Err(GatewayError::from_api(error)),
            }
        }
    }

    async fn start_inner(
        &self,
        adapted: Option<(ResponsesApiRequest, String)>,
        native: Option<(Box<serde_json::value::RawValue>, http::HeaderMap)>,
    ) -> Result<RunningResponse, GatewayError> {
        let mut recovery = self.auth.unauthorized_recovery();
        let original = self.provider.auth().await.ok_or_else(GatewayError::auth)?;
        let account = (original.get_account_id(), original.get_chatgpt_user_id());
        let auth_changes = self.auth.auth_change_receiver();
        let fallback = codex_model_provider::AgentIdentitySessionFallback::default();
        loop {
            let revision = *auth_changes.borrow();
            let auth = self.provider.auth().await.ok_or_else(GatewayError::auth)?;
            if self.subscription_only && !auth.is_chatgpt_auth() {
                return Err(GatewayError::auth());
            }
            if (auth.get_account_id(), auth.get_chatgpt_user_id()) != account {
                return Err(GatewayError::auth());
            }
            let resolved = self
                .provider
                .api_provider()
                .await
                .map_err(GatewayError::internal)?;
            let api_auth = self
                .provider
                .api_auth_for_scope(codex_model_provider::ProviderAuthScope {
                    agent_identity_policy: self.agent_identity_policy,
                    session_source: SessionSource::Exec,
                    agent_identity_session_fallback: fallback.clone(),
                })
                .await
                .map_err(GatewayError::internal)?
                .auth;
            if *auth_changes.borrow() != revision {
                continue;
            }
            let http = create_client_for_route(
                &self.factory,
                &resolved.url_for_path("/responses"),
                ClientRouteClass::Api,
            )
            .map_err(GatewayError::internal)?;
            let (sender, receiver) = mpsc::channel(8);
            let response_headers = Arc::new(OnceLock::new());
            let transport = TapTransport {
                response_headers: response_headers.clone(),
                inner: ReqwestTransport::from_http_client(http),
                sender,
            };
            let client = ResponsesClient::new(transport, resolved, api_auth);
            let compression = if self.compression && auth.uses_codex_backend() {
                Compression::Zstd
            } else {
                Compression::None
            };
            let result = if let Some((body, headers)) = &native {
                client
                    .stream_raw_json(
                        body,
                        headers.clone(),
                        compression,
                        Some(Arc::new(OnceLock::new())),
                    )
                    .await
            } else {
                let (request, session_id) = adapted.as_ref().expect("one request variant");
                let mut headers = http::HeaderMap::new();
                let routing_hint = match &request.service_tier {
                    Some(tier) => format!("model={};tier={tier}", request.model),
                    None => format!("model={}", request.model),
                };
                headers.insert(
                    "x-codex-routing-hint",
                    http::HeaderValue::from_str(&routing_hint).map_err(GatewayError::internal)?,
                );
                let options = ResponsesOptions {
                    session_id: Some(session_id.clone()),
                    thread_id: Some(session_id.clone()),
                    session_source: Some(SessionSource::Exec),
                    extra_headers: headers,
                    compression,
                    turn_state: Some(Arc::new(OnceLock::new())),
                };
                client.stream_request(request.clone(), options).await
            };
            // Drop the client so only the active byte stream holds the raw-event sender.
            drop(client);
            match result {
                Ok(mut parsed) => {
                    let (finished_tx, finished) = oneshot::channel();
                    let observer = tokio::spawn(async move {
                        let mut completed = false;
                        while let Some(event) = parsed.next().await {
                            match event {
                                Ok(codex_api::ResponseEvent::Completed { .. }) => {
                                    completed = true;
                                    break;
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    let _ = finished_tx.send(Err(e.to_string()));
                                    return;
                                }
                            }
                        }
                        let result = if completed {
                            Ok(())
                        } else {
                            Err("stream closed before response.completed".into())
                        };
                        let _ = finished_tx.send(result);
                    });
                    return Ok(RunningResponse {
                        headers: response_headers.get().cloned().unwrap_or_default(),
                        events: receiver,
                        finished,
                        observer,
                    });
                }
                Err(ApiError::Transport(ref error))
                    if self.provider.is_recoverable_auth_error(error) && recovery.has_next() =>
                {
                    recovery.next().await.map_err(|_| GatewayError::auth())?;
                }
                Err(error) => return Err(GatewayError::from_api(error)),
            }
        }
    }
}
