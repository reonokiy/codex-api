use anyhow::{Context, bail};
use clap::Parser;
use codex_api_gateway::{
    server::{Gateway, router},
    settings::load_backend,
};
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

#[derive(Parser)]
#[command(
    version,
    about = "Responses API gateway using pinned Codex Rust libraries"
)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8080")]
    listen: SocketAddr,
    #[arg(long, env = "CODEX_HOME")]
    codex_home: Option<PathBuf>,
    #[arg(long, default_value_t=4, value_parser=clap::value_parser!(u16).range(1..))]
    max_concurrency: u16,
    #[arg(long, default_value_t=300, value_parser=clap::value_parser!(u64).range(1..))]
    timeout_seconds: u64,
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "codex_api_gateway=info".into()),
        )
        .init();
    let args = Args::parse();
    let key = std::env::var("CODEX_GATEWAY_API_KEY").ok();
    if key.as_ref().is_some_and(|v| v.trim().is_empty()) {
        bail!("CODEX_GATEWAY_API_KEY must not be empty");
    }
    if !args.listen.ip().is_loopback() && key.is_none() {
        bail!("Set CODEX_GATEWAY_API_KEY before listening outside loopback");
    }
    let home = match args.codex_home {
        Some(path) => path,
        None => PathBuf::from(
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .context("set CODEX_HOME")?,
        )
        .join(".codex"),
    };
    let backend = load_backend(home).await?;
    let models = codex_models_manager::bundled_models_response()?.models;
    let timeout = Duration::from_secs(args.timeout_seconds);
    let transfers = std::env::var("CODEX_GATEWAY_PUBLIC_URL")
        .ok()
        .map(|url| {
            codex_api_gateway::transfers::Transfers::new(backend.factory.clone(), &url, timeout)
        })
        .transpose()?;
    let gateway = Gateway {
        backend,
        models,
        key,
        concurrency: Arc::new(tokio::sync::Semaphore::new(usize::from(
            args.max_concurrency,
        ))),
        timeout,
        transfers,
    };
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    tracing::info!(listen=%listener.local_addr()?, "Codex Responses gateway ready");
    axum::serve(listener, router(gateway))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
