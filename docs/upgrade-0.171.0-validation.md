# Codex 0.171.0 upgrade validation

核验日期：2026-11-21。由 0.169.0 升级至当日最新的官方稳定版
[rust-v0.171.0](https://github.com/openai/codex/releases/tag/rust-v0.171.0)，
固定提交 `ccf9be8cc458bd6b8336fe84d68d8893f2691a5e`。

## 改动

更新所有 Codex 依赖、Cargo.lock、运行时版本与当前版本文档。
共享 registry 依赖按上游 Cargo.lock 对齐。同步 crossterm patch，
Rust 工具链和 Docker 构建基础镜像升级到 1.99.1，基础镜像继续固定 digest。
官方 CLI、bwrap 和 Code Mode host 的下载地址与 SHA-256 均已更新并实测校验。

刷新 vendored codex-api 源码和上游集成测试，迁移并重新生成 raw-transport.patch。
Realtime 补丁保留新版按协议选择默认语音的逻辑；网关测试的显式语音改用可选字段。
公共 Responses 适配器对新增 request_country 使用上游默认的省略行为，
HTTP 和 WebSocket 测试均检查不会意外发送该字段。

## 验证

- `cargo fmt --all -- --check`：通过。
- `cargo check --workspace --all-targets --locked`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `python3 scripts/fetch-codex-baseline.py`：官方版本及三个发布资源校验通过。
- `python3 scripts/verify-codex-source.py`：通过。
- `uv sync --locked --group live`：通过。
- `cargo test --workspace --locked`：342 passed，0 failed，12 ignored。
- `cargo test --locked --test gateway actual_ -- --ignored`：5 passed。
- `cargo test --locked --test source_audit -- --ignored`：1 passed。
- `docker build -t codex-api:upgrade-0.171.0 .`：通过。
- `python3 scripts/smoke-container.py codex-api:upgrade-0.171.0`：通过。
  镜像大小 112.4 MiB；启动、健康检查中的版本与提交、鉴权、模型目录、工具和受限路由均通过。

源码审计确认 72 个 Codex git 包、1005 个共享 registry 包、47 个未修改源码文件、
6 个原样上游集成测试。仍仅修改六个上游文件，另加 raw.rs 和网关集成测试；
SSE parser 与 0.171.0 上游一致。

凭据相关 live 测试未运行；客户端测试使用本地模拟上游，容器使用模拟凭据。
验证日志位于 `.cache/upgrade-0.171.0/`。镜像仅在本机构建，未发布或部署。
