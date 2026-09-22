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
