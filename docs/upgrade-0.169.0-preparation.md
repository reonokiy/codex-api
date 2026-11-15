# Codex 0.169.0 升级准备

核验日期：2026-11-15。准备阶段记录。升级已实施，结果见 [验证记录](upgrade-0.169.0-validation.md)。

## 已核验的目标

- 当前：0.162.1，提交 `092d3acd6bec3e3a14bdc7e7a2810ab628ab759d`。
- 最新稳定版：0.169.0，标签 `rust-v0.169.0`，发布时间 2026-11-13。
- 标签对象：`a41fe5b443652cbdbee57717fbb9fd78d8d8b32a`。
- 目标提交：`d32063f96c2dff1dc96af839b3952abb6acfeda4`。
- Rust 仍为 1.95.0；上游还配置了 rust-src。

来源：[官方发布](https://github.com/openai/codex/releases/tag/rust-v0.169.0)、[标签元数据](https://api.github.com/repos/openai/codex/git/tags/a41fe5b443652cbdbee57717fbb9fd78d8d8b32a)、[上游 Cargo.toml](https://github.com/openai/codex/blob/rust-v0.169.0/codex-rs/Cargo.toml)、[工具链](https://github.com/openai/codex/blob/rust-v0.169.0/codex-rs/rust-toolchain.toml)。

## 实施顺序

1. 更新 baseline/codex-release.lock.json 的版本、标签、标签对象、提交、发布时间及资源下载地址。下载目标 CLI 与 companions，计算 SHA-256，并核对目标发布实际提供的资源；不能复用旧校验值。
2. 更新 Cargo.toml 所有 Codex git rev。同步上游补丁：crossterm `ed1cdab335221515706178d68495bba2aed1924f`，tokio-tungstenite `7447c427513cfe685f236e1ac0513c4ea3dd9c0b`，tungstenite `bc589d684755a8c02da0fc7e58d0861cbfc306f1`（含 SSH source patch）。
3. 从目标提交刷新 vendor/codex-api，迁移 vendor/raw-transport.patch，保留 gateway_extensions 集成测试。逐个检查 patch 冲突，重新生成补丁；不可直接覆盖本地扩展。
4. 更新 vendor manifest 并重新解析 Cargo.lock。按目标上游 Cargo.lock 对齐共享 registry 版本，避免无关依赖漂移；scripts/verify-codex-source.py 会严格校验。
5. 编译后适配 src 中实际发生变化的 API；检查 src/lib.rs 版本常量、src/lite_instructions.rs 及 tests/cases/response_types.rs 中版本引用的语义，再更新 README 和当前行为文档。保留历史升级记录。
6. 根据实际上游文件变化调整 source audit 的文件清单；保留源码、测试及补丁一致性断言。

## 验收命令

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo setup
cargo test --workspace --locked
cargo test --locked --test gateway actual_ -- --ignored
cargo test --locked --test source_audit -- --ignored
```

另按 CI 构建容器，并用 xtask smoke-container 验证实际镜像。凭据相关 live 测试单独记录执行条件和结果。

## 尚待验证

尚未比较全部上游源码差异、迁移 patch、下载并校验发布二进制或运行编译与测试。API 兼容性、补丁可应用性和容器兼容性都不能据此判定通过。重点检查 raw responses/SSE、WebSocket/realtime，以及官方客户端 fixture；现有 source audit 的六个修改文件断言可能需要随上游布局变化更新。
