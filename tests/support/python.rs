//! Shared interpreter and dependencies for official SDK tests.
pub fn command() -> tokio::process::Command {
    let mut command = if let Some(interpreter) = std::env::var_os("CODEX_TEST_PYTHON") {
        tokio::process::Command::new(interpreter)
    } else {
        let mut command = tokio::process::Command::new("uv");
        command.args([
            "run",
            "--project",
            env!("CARGO_MANIFEST_DIR"),
            "--locked",
            "--no-sync",
            "python",
        ]);
        command
    };
    command.env("PYTHONDONTWRITEBYTECODE", "1");
    command.kill_on_drop(true);
    command
}

/// Run collected pytest cases while Rust owns the gateway and captured upstream.
pub fn pytest(module: &str, report: &str) -> tokio::process::Command {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut command = command();
    command
        .current_dir(root)
        .args(["-m", "pytest", "-q", "-ra", "--tb=short"])
        .arg(root.join("tests/sdk").join(module))
        .arg("--sdk-report")
        .arg(root.join(report));
    command
}
