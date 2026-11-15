# Codex 0.169.0 upgrade validation

日期：2026-11-15。由 0.162.1 升级至官方稳定版 `rust-v0.169.0`，固定提交 `d32063f96c2dff1dc96af839b3952abb6acfeda4`。

## 改动

更新所有 Codex 依赖提交、Cargo.lock、官方发布资源与 SHA-256、运行时版本常量和当前版本文档。同步上游 SQLx、WebSocket 相关 Cargo patches。以目标上游锁文件为基础解析依赖，避免共享 registry 版本漂移。

刷新 vendored codex-api 的源码及上游集成测试，保留网关原始传输扩展并重新生成 raw-transport.patch。保留新版 WebSocket probe/TLS 错误分类；适配异步鉴权及鉴权错误、ContentItem 上下文片段、共享工具列表、新增的可选图像 output 和 prompt_cache_options 字段。

修复 Clippy 指出的 map_err 副作用写法；测试夹具的多参数工厂使用局部 lint allow。

## 验证

- `cargo fmt --all -- --check`：通过。
- `cargo check --workspace --all-targets --locked`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo test --workspace --locked`：342 passed，0 failed，12 ignored。
- `cargo test --locked --test gateway actual_ -- --ignored`：5 passed。
- `cargo test --locked --test source_audit -- --ignored`：1 passed。
- 官方 CLI 版本及二进制、companions SHA-256 校验：通过；`uv sync --locked --group live`：通过。

源码审计确认 70 个 Codex git 包、990 个共享 registry 包、47 个未改动源码文件、6 个原样上游集成测试。仅六个上游源码文件包含有意修改，另增加 raw.rs 和网关集成测试；SSE parser 与上游一致。

日志位于 `artifacts/maintenance-0.169.0/`。本机 mise shim 无默认 Rust 配置，使用现有 `/home/nokiy/.cargo/bin` 和项目固定的 Rust 1.95.0。缺少 OpenSSL 开发文件，通过下载 Ubuntu libssl-dev 3.0.13 包并解压到 `.cache/build-deps/openssl3/` 提供，仅为本机构建环境设置 OPENSSL_DIR、OPENSSL_LIB_DIR 和 OPENSSL_STATIC=1，未改系统安装。

## 限制

容器构建尝试两次，均在拉取固定 Rust 基础镜像时失败：`failed to copy: local error: tls: bad record MAC`。尚未执行镜像冒烟测试。凭据相关 live 测试未执行；官方客户端兼容性检查使用本地 fixture，不表示已验证真实服务。
