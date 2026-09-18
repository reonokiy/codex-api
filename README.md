# Codex API Gateway

复用 Codex 原版 Rust 库，将 ChatGPT 订阅接入转换为 Responses API，也提供供另一个 Codex 使用的原生入口。支持 HTTP/SSE、WebSocket、Responses Lite 和远程上下文压缩。

实现与对照 CLI 固定为 **Codex 0.155.0**，Rust **1.95.0**。版本、commit 和发行包校验值见 [版本锁文件](baseline/codex-release.lock.json)。

## 启动

先用 Codex 完成订阅登录，然后启动网关：

```sh
codex login
export CODEX_GATEWAY_API_KEY='your-gateway-key'
cargo run --locked --release -- --listen 127.0.0.1:8080
```

默认读取 `CODEX_HOME` 或 `~/.codex`，也可用 `--codex-home` 指定登录目录。上游仅接受 ChatGPT 订阅登录；调用方使用独立的网关密钥。

默认并发 4、超时 300 秒，可通过 `--max-concurrency`、`--timeout-seconds` 调整。远程访问请使用 HTTPS/WSS。

## Responses API

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:8080/v1",
    api_key="your-gateway-key",
)
response = client.responses.create(model="gpt-5.6-terra", input="你好")
print(response.output_text)
```

`stream=True` 返回 SSE。工具由调用方执行；HTTP 续聊需发送完整历史。不支持服务端历史存储、`background` 等字段，未实现的参数会明确报错。

## 另一个 Codex 接入

调用方设置相同的 `CODEX_GATEWAY_API_KEY`，在其 `config.toml` 中配置：

```toml
model = "gpt-5.6-terra"
model_provider = "gateway"

[model_providers.gateway]
name = "OpenAI"
base_url = "http://127.0.0.1:8080/backend-api/codex"
wire_api = "responses"
env_key = "CODEX_GATEWAY_API_KEY"
requires_openai_auth = false
supports_websockets = true
```

保留 `name = "OpenAI"` 和上述 URL 后缀，以启用固定版本 Codex 的第一方协议能力。

## 接口

| 路径 | 用途 |
| --- | --- |
| `/v1/responses` | 通用 Responses，POST HTTP/SSE 或 GET WebSocket |
| `/codex/responses` | 原生 Codex，POST HTTP/SSE 或 GET WebSocket |
| `/backend-api/codex/responses` | 原生入口别名 |
| `/v1/responses/compact`、`/codex/responses/compact` | POST 远程压缩转换接口 |
| `/v1/models` | GET 内置模型目录 |
| `/codex/models`、`/backend-api/codex/models` | GET 上游原生模型目录 |
| `/healthz` | GET 健康状态及版本 |

原生 HTTP 请求要求 `model` 和 `stream:true`。除健康检查外，设置密钥后所有接口均要求 Bearer 认证。

## 验证与源码

```sh
cargo test --locked
cargo test --locked -p codex-api --lib
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings

# 下载、校验固定官方源码和 CLI，并运行版本审计与 CLI 对照
python3 scripts/fetch-codex-baseline.py
python3 scripts/verify-codex-source.py
cargo test --locked --test gateway actual_codex_cli -- --ignored
```

认证、请求构造、压缩和事件解析复用原版库。Vendored 库仅增加两个原始传输接口，变更见 [补丁](vendor/raw-transport.patch)。协议测试比较请求头、压缩请求体和 WS 请求帧；网关使用自己的订阅身份并重建连接，不承诺 TLS/TCP 字节完全相同。通用入口会转换输入并补齐输出，原生入口保留请求和事件内容。

Apache-2.0，第三方来源见 [NOTICE](NOTICE)。
