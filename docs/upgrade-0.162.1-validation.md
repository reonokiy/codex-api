# Codex 0.162.1 upgrade validation

- Base: `main` / `8ae930e5ba3052433d041ca879e0aa96b6b0d503` (Codex 0.160.0).
- Upstream stable: `rust-v0.162.1`, commit `092d3acd6bec3e3a14bdc7e7a2810ab628ab759d`.
- Independent worktree and branch; incorporates the complete compatibility changes from PR #13 relative to main. Existing PRs are untouched.
- PR #13's Rust/client compatibility, image and publish checks were all successful when inspected.
- Updated all current Codex dependency pins, Cargo.lock packages, release asset checksums, runtime constants, vendored crate version, and documentation. Fixed the stale revision in the health example.
- Upstream codex-api sources are identical between 0.162.0 and 0.162.1; the full vendored refresh and raw-transport patch relative to main are included. Historical API inventory references intentionally remain 0.155.1.

## Local validation

Rust 1.95.0, CARGO_BUILD_JOBS=2, isolated target directory. All commands below completed successfully:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo setup
cargo test --workspace --locked
cargo test --locked --test gateway actual_ -- --ignored
cargo test --locked --test source_audit -- --ignored
```

Workspace: 287 passed, 0 failed, 12 ignored. Explicit official-client fixture compatibility: 5 passed. Explicit source audit: 1 passed.

Source audit verified 66 pinned Codex git packages, 980 shared registry packages, 42 unchanged source files and 5 unchanged upstream integration-test files. The six intentional raw-transport source modifications exactly match the checked-in patch. Release binaries and companions passed SHA-256 and CLI version checks.

## Limitations and environment recovery

The default mise cargo shim initially failed before executing checks; using the existing `/home/coder/.cargo/bin` toolchain resolved this without system installation or global configuration changes. Device relay interruptions did not stop the validation process; results were recovered from logs. Credential-dependent live endpoint tests and container build/smoke were not run locally. Open this PR as Draft pending remote container CI and review; fixture compatibility is not a claim of live service access.

## Evidence

Full logs remain in `artifacts/maintenance-0.162.1/` in the isolated worktree. Exit records and log SHA-256 digests:

```text
fmt exit=0 2026-10-09T22:03:21Z
check exit=0 2026-10-09T22:09:52Z
clippy exit=0 2026-10-09T22:10:26Z
setup exit=0 2026-10-09T22:10:33Z
workspace exit=0 2026-10-09T22:18:34Z
compatibility exit=0 2026-10-09T22:21:33Z
source-audit exit=0 2026-10-09T22:21:39Z
701eda61a442e6dbb1e244baa9e633501f86b8067c287009d8717e853844f3a1  check.log
01b0e1ae2c4b7bbe0a5f0f433ca46777d9311ec761f15c3f6cbbe62d8907a129  clippy.log
5a117d6255d80264e4d1aef832b09555dab1b8e941e6a6800ecbd53c7103b6a7  compatibility.log
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  fmt.log
07ba89728da8d2e1c66b4a69b324964a2402bf927f831297d81eb9caf59b23a3  setup.log
d8a19c00651429395aa23d082bf50679e680f99acbdeb39bec4ae3a46234e4d0  source-audit.log
637c2ab4da69fd4244cfc3a188e766df72c01961c3c89b70761409f46a2ac1d5  workspace.log
```
