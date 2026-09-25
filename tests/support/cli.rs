//! Always launch the pinned official binary, never an in-process CLI substitute.
pub fn home() -> tempfile::TempDir {
    // Codex refuses to install executable helpers under a system temporary directory.
    let parent = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache/cli-homes");
    std::fs::create_dir_all(&parent).unwrap();
    tempfile::tempdir_in(parent).unwrap()
}

pub async fn binary() -> String {
    let binary = std::env::var("CODEX_CLI_BIN").unwrap_or_else(|_| {
        format!(
            "{}/.cache/codex-baseline/rust-v{}/bin/codex-x86_64-unknown-linux-musl",
            env!("CARGO_MANIFEST_DIR"),
            codex_api_gateway::CODEX_RELEASE
        )
    });
    if std::env::var_os("CODEX_CLI_BIN").is_none() {
        let directory = std::path::Path::new(&binary).parent().unwrap();
        for helper in ["codex-code-mode-host", "codex-resources/bwrap"] {
            assert!(
                directory.join(helper).is_file(),
                "missing official {helper}; run cargo setup"
            );
        }
    }
    let version = tokio::process::Command::new(&binary)
        .arg("--version")
        .kill_on_drop(true)
        .output()
        .await
        .expect("official Codex CLI missing; run cargo setup");
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        format!("codex-cli {}", codex_api_gateway::CODEX_RELEASE),
        "CLI must match the pinned upstream release"
    );
    binary
}
