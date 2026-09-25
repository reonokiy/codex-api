//! Cargo entry points for prerequisite setup and external validation tools.
use std::{env, path::Path, process::Command};

fn run(program: &str, args: &[&str]) {
    let status = Command::new(program)
        .args(args)
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .status()
        .unwrap_or_else(|error| panic!("start {program}: {error}"));
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("setup") if args.len() == 1 => {
            run("python3", &["scripts/fetch-codex-baseline.py"]);
            run("uv", &["sync", "--locked", "--group", "live"]);
        }
        Some("ablation") => {
            let mut command = vec!["scripts/ablation.py"];
            command.extend(args[1..].iter().map(String::as_str));
            run("python3", &command);
        }
        Some("smoke-container") if args.len() == 2 => {
            run("python3", &["scripts/smoke-container.py", &args[1]]);
        }
        _ => {
            eprintln!(
                "Use cargo setup | cargo run --locked -p xtask -- ablation [--case NAME] | cargo run --locked -p xtask -- smoke-container IMAGE"
            );
            std::process::exit(2);
        }
    }
}
