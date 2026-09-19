use crate::backend::Backend;
use anyhow::{Context, bail};
use codex_config::config_toml::ConfigToml;
use codex_http_client::{HttpClientFactory, OutboundProxyPolicy};
use codex_login::{AuthConfig, AuthManager, AuthRouteConfig, ServerOptions};
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use std::{path::PathBuf, sync::Arc, time::Duration};

const DEVICE_LOGIN_TIMEOUT: Duration = Duration::from_secs(10 * 60);

pub async fn load_backend(codex_home: PathBuf) -> anyhow::Result<Backend> {
    load_backend_inner(codex_home, false).await
}

/// Bootstrap an unattended gateway, prompting in its logs when credentials are missing.
pub async fn load_backend_with_device_login(codex_home: PathBuf) -> anyhow::Result<Backend> {
    load_backend_inner(codex_home, true).await
}

async fn load_backend_inner(codex_home: PathBuf, device_login: bool) -> anyhow::Result<Backend> {
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
    let mut auth = AuthManager::shared_from_auth_config(auth_config.clone(), false).await?;
    if device_login && auth.auth_cached().is_none() {
        // Never replace an existing, unreadable or invalid credential file with a new login.
        match tokio::fs::symlink_metadata(auth_config.codex_home.join("auth.json")).await {
            Ok(_) => bail!(
                "auth.json exists but no usable Codex login was found; repair the credentials before restarting"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("checking auth.json before device login"),
        }
        let opts = device_login_options(&auth_config)?;
        login_with_timeout(opts, DEVICE_LOGIN_TIMEOUT).await?;
        auth = AuthManager::shared_from_auth_config(auth_config, false).await?;
    }
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
        platform_base_url: "https://api.openai.com/v1".into(),
        auth_base_url: "https://auth.openai.com".into(),
        subscription_only: true,
        compression: features.enabled(codex_features::Feature::EnableRequestCompression),
        agent_identity_policy: if features.enabled(codex_features::Feature::UseAgentIdentity) {
            codex_login::AgentIdentityAuthPolicy::ChatGptAuth
        } else {
            codex_login::AgentIdentityAuthPolicy::JwtOnly
        },
    })
}

fn device_login_options(config: &AuthConfig) -> anyhow::Result<ServerOptions> {
    if !config.is_login_method_allowed(codex_protocol::config_types::ForcedLoginMethod::Chatgpt) {
        bail!("ChatGPT device login is disabled by the Codex authentication configuration");
    }
    let mut opts = ServerOptions::new(
        config.codex_home.clone(),
        codex_login::oauth_client_id(),
        config.effective_chatgpt_workspaces(),
        config.auth_credentials_store_mode,
        config.keyring_backend_kind,
        config.auth_route_config.clone(),
    );
    opts.open_browser = false;
    Ok(opts)
}

async fn login_with_timeout(opts: ServerOptions, timeout: Duration) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(&opts.codex_home)
        .await
        .context("creating CODEX_HOME for device login")?;
    tracing::info!(
        timeout_seconds = timeout.as_secs(),
        "No Codex login found; starting device code login"
    );
    tokio::time::timeout(timeout, async {
        let code = codex_login::request_device_code(&opts)
            .await
            .context("requesting device code; enable device code login in ChatGPT security settings or workspace permissions")?;
        // Print directly so the login instructions remain visible even with RUST_LOG=off.
        eprintln!(
            "Open {} and enter code {} to sign in.\nComplete login within {} seconds. Only use this code if you started this gateway.",
            code.verification_url, code.user_code, timeout.as_secs()
        );
        codex_login::complete_device_code_login(opts, code)
            .await
            .context("completing device code login")
    })
    .await
    .with_context(|| format!("Device code login timed out after {} seconds; exiting so the container restart policy can retry", timeout.as_secs()))??;
    tracing::info!("Device code login saved; starting gateway");
    Ok(())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
