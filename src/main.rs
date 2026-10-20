use anyhow::{Context, bail};
use clap::Parser;
use codex_api_gateway::{
    server::{Gateway, router},
    settings::load_backend_with_device_login,
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
    /// Maximum HTTP body, decoded zstd body, and WebSocket message/frame size in bytes.
    #[arg(long, env = "CODEX_GATEWAY_MAX_REQUEST_BYTES", default_value_t = 64 * 1024 * 1024, value_parser = parse_request_bytes)]
    max_request_bytes: usize,
}
fn parse_request_bytes(value: &str) -> Result<usize, String> {
    let bytes = value.parse::<usize>().map_err(|e| e.to_string())?;
    if bytes == 0 || bytes > isize::MAX as usize {
        return Err("must be between 1 and isize::MAX bytes".into());
    }
    Ok(bytes)
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
    let backend = load_backend_with_device_login(home).await?;
    let timeout = Duration::from_secs(args.timeout_seconds);
    let models = backend
        .model_catalog(timeout.min(Duration::from_secs(30)))
        .await?;
    let transfers = std::env::var("CODEX_GATEWAY_PUBLIC_URL")
        .ok()
        .map(|url| {
            codex_api_gateway::transfers::Transfers::new(backend.factory.clone(), &url, timeout)
        })
        .transpose()?;
    let telemetry =
        tokio::task::spawn_blocking(codex_api_gateway::telemetry::Telemetry::from_env).await??;
    let gateway = Gateway {
        telemetry: telemetry.clone(),
        backend,
        models,
        key,
        concurrency: Arc::new(tokio::sync::Semaphore::new(usize::from(
            args.max_concurrency,
        ))),
        timeout,
        max_request_bytes: args.max_request_bytes,
        transfers,
    };
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    tracing::info!(listen=%listener.local_addr()?, "Codex Responses gateway ready");
    axum::serve(listener, router(gateway))
        .with_graceful_shutdown(async {
            #[cfg(unix)]
            {
                let mut terminate =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("install SIGTERM handler");
                tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
            }
            #[cfg(not(unix))]
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    if let Some(telemetry) = telemetry {
        tokio::task::spawn_blocking(move || telemetry.shutdown()).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_limit_defaults_to_64_mib() {
        use clap::CommandFactory;
        let command = Args::command();
        let argument = command
            .get_arguments()
            .find(|arg| arg.get_id() == "max_request_bytes")
            .unwrap();
        assert_eq!(
            argument.get_default_values(),
            &[std::ffi::OsString::from("67108864")]
        );
    }

    #[test]
    fn request_limit_cli_validation() {
        for value in ["1", "16777216", "67108864"] {
            let args = Args::try_parse_from(["gateway", "--max-request-bytes", value]).unwrap();
            assert_eq!(args.max_request_bytes, value.parse::<usize>().unwrap());
        }
        for value in ["0", "-1", "invalid", "18446744073709551615"] {
            assert!(Args::try_parse_from(["gateway", "--max-request-bytes", value]).is_err());
        }
    }
}
