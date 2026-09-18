use crate::backend::Backend;
use anyhow::{Context, bail};
use codex_config::config_toml::ConfigToml;
use codex_http_client::{HttpClientFactory, OutboundProxyPolicy};
use codex_login::{AuthConfig, AuthManager, AuthRouteConfig};
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use std::{path::PathBuf, sync::Arc};

pub async fn load_backend(codex_home: PathBuf) -> anyhow::Result<Backend> {
    let codex_home = std::path::absolute(codex_home)?;
    let stack = codex_config::loader::load_config_layers_state(
        codex_exec_server::LOCAL_FS.as_ref(),
        &codex_home,
        None,
        &[],
        codex_config::LoaderOverrides::default(),
        &codex_config::NoopThreadConfigLoader,
    )
    .await
    .context("loading Codex configuration layers")?;
    let config: ConfigToml = stack
        .effective_config()
        .try_into()
        .context("invalid Codex configuration")?;
    let requirements = stack.requirements();
    let mut features = codex_features::Features::from_sources(
        codex_features::FeatureConfigSource {
            features: config.features.as_ref(),
            ..Default::default()
        },
        Default::default(),
        Default::default(),
    );
    if let Some(required) = requirements.feature_requirements.as_ref() {
        features.apply_map(&required.value.entries);
    }
    let factory = HttpClientFactory::new(
        if features.enabled(codex_features::Feature::RespectSystemProxy) {
            OutboundProxyPolicy::RespectSystemProxy
        } else {
            OutboundProxyPolicy::ReqwestDefault
        },
    );
    let chatgpt_base_url = requirements
        .chatgpt_base_url
        .as_ref()
        .map(|v| v.value.clone())
        .or(config.chatgpt_base_url.clone())
        .unwrap_or_else(|| "https://chatgpt.com/backend-api".into());
    let auth_config = AuthConfig {
        codex_home,
        auth_credentials_store_mode: requirements
            .cli_auth_credentials_store
            .as_ref()
            .map(|v| v.value)
            .or(config.cli_auth_credentials_store)
            .unwrap_or_default(),
        keyring_backend_kind: if features.enabled(codex_features::Feature::SecretAuthStorage) {
            codex_login::AuthKeyringBackendKind::Secrets
        } else {
            codex_login::AuthKeyringBackendKind::Direct
        },
        forced_login_method: config.forced_login_method,
        forced_chatgpt_workspace_id: config.forced_chatgpt_workspace_id.map(|v| v.into_vec()),
        chatgpt_base_url: Some(chatgpt_base_url.clone()),
        managed_auth_policy: requirements.managed_auth_policy(),
        auth_route_config: AuthRouteConfig::from_http_client_factory(factory.clone()),
    };
    auth_config.validate()?;
    let auth = AuthManager::shared_from_auth_config(auth_config, false).await?;
    let cached = auth
        .auth_cached()
        .context("No Codex login found. Run `codex login` with this CODEX_HOME first.")?;
    if !cached.is_chatgpt_auth() {
        bail!(
            "This gateway requires a ChatGPT Codex login; current credentials are not subscription login credentials."
        );
    }
    let provider = create_model_provider(
        ModelProviderInfo::create_openai_provider(None),
        Some(Arc::clone(&auth)),
    );
    Ok(Backend {
        provider,
        auth,
        factory,
        chatgpt_base_url,
        subscription_only: true,
        compression: features.enabled(codex_features::Feature::EnableRequestCompression),
        agent_identity_policy: if features.enabled(codex_features::Feature::UseAgentIdentity) {
            codex_login::AgentIdentityAuthPolicy::ChatGptAuth
        } else {
            codex_login::AgentIdentityAuthPolicy::JwtOnly
        },
    })
}
