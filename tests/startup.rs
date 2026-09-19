use codex_api_gateway::settings::{load_backend, load_backend_with_device_login};

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
    let error = load_backend_with_device_login(home.path().to_path_buf())
        .await
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("requires a ChatGPT Codex login"),
        "{error:#}"
    );
}

#[tokio::test]
async fn automatic_login_does_not_overwrite_invalid_auth_file() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("auth.json");
    std::fs::write(&path, "{invalid-json").unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        load_backend_with_device_login(home.path().to_path_buf()),
    )
    .await
    .unwrap();
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "{invalid-json");
}
