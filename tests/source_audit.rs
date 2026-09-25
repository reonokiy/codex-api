#[test]
#[ignore = "requires cargo setup to fetch the pinned official source"]
fn pinned_codex_source_and_dependencies_match_release() {
    let output = std::process::Command::new("python3")
        .arg("scripts/verify-codex-source.py")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("start Python source audit");
    assert!(
        output.status.success(),
        "source audit failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
