//! Shared interpreter and dependencies for official SDK tests.
pub fn command() -> tokio::process::Command {
    let local = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(if cfg!(windows) {
        ".venv/Scripts/python.exe"
    } else {
        ".venv/bin/python"
    });
    let interpreter = std::env::var_os("CODEX_TEST_PYTHON").unwrap_or_else(|| {
        if local.is_file() {
            local.into_os_string()
        } else {
            "python3".into()
        }
    });
    let mut command = tokio::process::Command::new(interpreter);
    command.env("PYTHONDONTWRITEBYTECODE", "1");
    command.kill_on_drop(true);
    command
}
