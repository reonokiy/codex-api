# Codex API Gateway

复用 Codex 原版 Rust 库，将 ChatGPT 订阅接入转换为 Responses API，也提供供另一个 Codex 使用的原生入口。支持 HTTP/SSE、WebSocket、Responses Lite 和远程上下文压缩。

实现与对照 CLI 固定为 **Codex 0.155.0**，Rust **1.95.0**。版本、commit 和发行包校验值见 [版本锁文件](baseline/codex-release.lock.json)。

## 容器运行

镜像：`ghcr.io/reonokiy/codex-api:latest`（`linux/amd64`）。基于 `scratch`，仅包含程序、必要动态库和 CA 证书。

```sh
export CODEX_GATEWAY_API_KEY='your-gateway-key'
docker run -d --name codex-api --restart unless-stopped \
  --user "$(id -u):$(id -g)" \
  -p 127.0.0.1:8080:8080 \
  -e CODEX_GATEWAY_API_KEY \
  -v "$HOME/.codex:/data" \
  ghcr.io/reonokiy/codex-api:latest
```

先在宿主机执行 `codex login`，挂载的登录目录需可写，以保存刷新后的凭据。Actions 在推送 `main` 时更新 `latest`，推送 `v*` 标签时发布同名镜像，也保留 `sha-<commit>` 标签；PR 只构建和测试。

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

`stream=True` 返回 SSE。函数及自定义工具由调用方执行，网页搜索由上游执行；HTTP 续聊需发送完整历史。不支持服务端历史存储、`background` 等字段，未实现的参数会明确报错。

## 网页搜索与工具

```python
response = client.responses.create(
    model="gpt-5.5",
    input="搜索 OpenAI 官网的 Codex 介绍，给出来源链接。",
    tools=[{"type": "web_search", "external_web_access": True}],
    include=["web_search_call.action.sources"],
)
print(response.output_text)
```

复用原版 `ToolSpec`，支持 `web_search`、`function`、grammar 格式的 `custom` 和 `namespace`。网页搜索支持域名过滤、位置、上下文大小、缓存/实时访问及文本/图片搜索；完整保留搜索事件、来源和引用。此用法中，打开网页、页内查找由模型通过同一搜索工具完成。HTTP/SSE、WebSocket 和 Responses Lite 均支持。

另提供 Codex 原生 `POST /codex/alpha/search`（也支持 `/backend-api/codex` 别名），复用原版 `SearchClient`。请求包含 `id`（搜索会话 ID）、`model` 和 `commands`，例如 `{"id":"search-1","model":"gpt-5.5","commands":{"search_query":[{"q":"OpenAI Codex"}]}}`。可直接调用搜索、图片搜索、打开/点击/查找、PDF 截图、天气、行情、体育和时间命令；保留同一个 `id` 继续引用之前的搜索结果。此接口是 Codex 扩展，不是 OpenAI 官方公共 API。

固定版本未接入托管 `file_search`、`code_interpreter`、`mcp` 和 Responses `image_generation`，这些类型在通用入口返回 400。原生入口保留 Codex 的工具定义及调用，由调用方执行本地工具。仅支持 `tool_choice="auto"`，搜索是否发生由模型决定。

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
http_headers = { "x-openai-actor-authorization" = "gateway" }
```

保留 `name = "OpenAI"` 和上述 URL 后缀。`x-openai-actor-authorization` 是供固定版本 Codex 识别代理并启用图像工具的标记；网关不将它用于鉴权或透传给上游，调用方仍需网关密钥。图像功能默认开启，被关闭时可通过 `codex --enable image_generation` 开启。需要原生 `web.run` 时可运行 `codex --enable standalone_web_search --search`。

## 图像生成与编辑

复用 Codex 原版 `ImagesClient`，默认模型为 `gpt-image-2`，返回 Base64 图片：

```python
import base64
from pathlib import Path

result = client.images.generate(model="gpt-image-2", prompt="一只蓝色的小鸟")
Path("bird.png").write_bytes(base64.b64decode(result.data[0].b64_json))

with open("bird.png", "rb") as image:
    edited = client.images.edit(model="gpt-image-2", image=image, prompt="将背景改为白色")
```

支持 `prompt`、`model`、`n`、`size`、`quality`、`background`；编辑接受 multipart 上传或原生 JSON 的 `images: [{"image_url": "data:image/png;base64,..."}]`。最多 5 张编辑图片，请求总上限 16 MiB。`response_format` 仅支持 `b64_json`，不支持 mask、图像流式输出和非 PNG 输出。图像调用走下列独立接口；`/v1/responses` 尚不执行内置 `image_generation` 工具。

## 接口

| 路径 | 用途 |
| --- | --- |
| `/v1/responses` | 通用 Responses，POST HTTP/SSE 或 GET WebSocket |
| `/codex/responses` | 原生 Codex，POST HTTP/SSE 或 GET WebSocket |
| `/backend-api/codex/responses` | 原生入口别名 |
| `/v1/responses/compact`、`/codex/responses/compact` | POST 远程压缩转换接口 |
| `/v1/models` | GET 内置模型目录 |
| `/codex/models`、`/backend-api/codex/models` | GET 上游原生模型目录 |
| `/v1/images/generations`、`/v1/images/edits` | POST 图像生成及编辑，兼容 SDK 上传 |
| `/codex/images/generations`、`/codex/images/edits` | POST 原生 Codex JSON 图像接口，支持 `/backend-api/codex` 别名 |
| `/codex/alpha/search`、`/backend-api/codex/alpha/search` | POST 原生独立搜索及相关命令 |
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
