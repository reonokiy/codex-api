use codex_api_gateway::settings::load_backend;

#[tokio::test]
async fn startup_reports_missing_login_in_an_isolated_codex_home() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join("config.toml"),
        "cli_auth_credentials_store = \"file\"\n",
    )
    .unwrap();
    let error = load_backend(home.path().to_path_buf()).await.err().unwrap();
    assert!(error.to_string().contains("No Codex login"), "{error:#}");
}

#[tokio::test]
async fn startup_refuses_api_key_credentials_instead_of_billing_platform() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join("config.toml"),
        "cli_auth_credentials_store = \"file\"\n",
    )
    .unwrap();
    codex_login::login_with_api_key(
        home.path(),
        "synthetic-test-key",
        codex_login::AuthCredentialsStoreMode::File,
        codex_login::AuthKeyringBackendKind::Direct,
    )
    .unwrap();
    let error = load_backend(home.path().to_path_buf()).await.err().unwrap();
    assert!(
        error.to_string().contains("requires a ChatGPT Codex login"),
        "{error:#}"
    );
}
