# Codex 0.163.0 upgrade validation

- Remote default branch: `main`, fetched at `991fb8c3df34b42e67763eaf600a0c1f49b13838` (Codex 0.162.1).
- Upstream latest stable: `rust-v0.163.0`, published `2026-10-15T21:30:38Z`; GitHub release has `draft=false`, `prerelease=false`. Both `/releases/latest` and the first 100 releases were checked.
- Annotated tag object: `129b8ec7ee6f4511557a4856279f529025c25af4`; peeled commit: `83086188ae0614bd882818ff1aa23f3194962012`, independently checked with `git ls-remote` and the clean downloaded source's HEAD.
- No existing Codex upgrade PR was open (only unrelated Renovate configuration PR #1).
- Work performed on independent branch/worktree `chore/codex-0.163.0`, preserving the original checkout's uncommitted development changes.

## Changes

Updated direct Codex Cargo dependencies, all transitive Codex lockfile entries, the vendored crate and its upstream integration tests, runtime version/revision, release binaries and companion checksums, active documentation, and NOTICE. Updated `hyper` and `quinn-proto` to the versions in the upstream release lockfile. Historical API inventories and prior upgrade reports remain historical snapshots.

Refreshed every vendored Rust source from the pinned release, then reapplied the existing raw-transport patch. The patch's content is unchanged apart from hunk offsets. New upstream SSE, WebSocket interruption and other transport changes are included. The lite instructions fragment retains the upstream role, kind, markers and body; unused upstream matching helpers remain omitted. Rust remains pinned to upstream's 1.95.0. Replaced an existing error-observation `map_err` with equivalent `inspect_err` to satisfy strict Clippy; error and usage-accounting behavior are preserved.

## Validation

Release binary and companions verified against the release SHA-256 digests; CLI reports `codex-cli 0.163.0`.

Source audit passes: 68 pinned Codex git packages, 981 shared registry packages, 42 identical upstream source files and 5 identical upstream integration test files. The six deliberate modified source files exactly match `vendor/raw-transport.patch` (SHA-256 `b2b366d0466c52e18f80582277c856aca8822cf38e708da71cbdfc3dac2b9383`). Vendored dependency expansion was checked against upstream's workspace declarations.

Full local validation logs are retained under `artifacts/maintenance-0.163.0/` in the isolated worktree.

All final commands passed, with `CARGO_BUILD_JOBS=4`:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo setup
cargo test --workspace --locked
cargo test --locked --test gateway actual_ -- --ignored
cargo test --locked --test source_audit -- --ignored
```

Workspace: **295 passed, 0 failed, 12 ignored**. Explicit official CLI/Python SDK fixtures: **5 passed**. Explicit source audit: **1 passed**. Credential-dependent live tests and container build/smoke were not run locally; the PR's remote container CI remains required before merge.

The initial lockfile check overlapped dependency resolution and was rerun after the lockfile was complete. An earlier compilation was interrupted to increase build concurrency. Strict Clippy initially found the error-observation style issue described above; all final checks passed after the fix.

Final successful exit records and log SHA-256 digests:

```text
fmt exit=0 2026-10-16T22:11:55Z
check exit=0 2026-10-16T22:12:13Z
clippy exit=0 2026-10-16T22:12:35Z
setup exit=0 2026-10-16T22:12:45Z
workspace exit=0 2026-10-16T22:19:43Z
compatibility exit=0 2026-10-16T22:22:43Z
source-audit exit=0 2026-10-16T22:22:56Z
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  fmt.log
bf03fe7e5f2c909fb3dbb87f1055189c3626575e3963e24f8496b87f79fb3fbc  check.log
ec139379b9388519caae2fff77792b841e3cdfc22662970ae32aedc3eedefcaa  clippy.log
3565fa18cf683be299575361d284d4f289ee400616dbbeb9eadbbc49860978b2  setup.log
ad3aaf8c4491db7b207fd487a34ed6d2ebd486eeb8a8a98c88ccfc224f5c1803  workspace.log
5f1e8e2535d1ff72d0acd30cbe47573d7251c2c60d5135bf13f5d28f7aa17c88  compatibility.log
549a44d09d4c046a93fa6f1f873ca8695a133297fe6372ad18c7bda54d7bbae3  source-audit.log
```
