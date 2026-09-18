# Codex API Gateway

A ChatGPT-subscription gateway built from the original Codex Rust libraries. It exposes native Codex routes and OpenAI SDK-compatible Responses, Images, Files and Realtime adapters, including SSE, WebSocket, Responses Lite, compaction and web search.

Pinned to **Codex 0.155.0** and **Rust 1.95.0**. The [release lock](baseline/codex-release.lock.json) records the source commit and official CLI checksums. API coverage and compatibility limits are documented per endpoint below.

## Run

Log in with `codex login` on the host, then mount that writable credential directory so Codex can save refreshed tokens:

```sh
export CODEX_GATEWAY_API_KEY='your-gateway-key'
docker run -d --name codex-api --restart unless-stopped \
  --user "$(id -u):$(id -g)" \
  -p 127.0.0.1:8080:8080 \
  -e CODEX_GATEWAY_API_KEY \
  -v "$HOME/.codex:/data" \
  ghcr.io/reonokiy/codex-api:latest
```

The `linux/amd64` image uses `scratch` with the gateway, required shared libraries and CA certificates. GitHub Actions publishes `latest` from `main`, `v*` tags and `sha-<commit>` tags after a container smoke test; pull requests build and test without publishing.

To run from source:

```sh
codex login
export CODEX_GATEWAY_API_KEY='your-gateway-key'
cargo run --locked --release -- --listen 127.0.0.1:8080
```

Credentials come from `--codex-home`, `CODEX_HOME`, or `~/.codex`. Subscription routes use ChatGPT credentials upstream; clients use a separate gateway key. `/platform/*` forwards official APIs using explicit upstream API credentials. Defaults: 4 concurrent requests, 300-second request/idle timeout (`--max-concurrency`, `--timeout-seconds`). Use HTTPS/WSS for remote access. Credential storage uses Codex's existing backend; a Kubernetes Secret API storage backend is not implemented.

## OpenAI SDK

```python
import base64
from pathlib import Path
from openai import OpenAI

client = OpenAI(base_url="http://127.0.0.1:8080/v1", api_key="your-gateway-key")
response = client.responses.create(model="gpt-5.5", input="Hello")
print(response.output_text)

response = client.responses.create(
    model="gpt-5.5", input="Find the official Codex page and cite it.",
    tools=[{"type": "web_search", "external_web_access": True}],
    include=["web_search_call.action.sources"],
)

image = client.images.generate(model="gpt-image-2", prompt="A blue bird")
Path("bird.png").write_bytes(base64.b64decode(image.data[0].b64_json))
with open("bird.png", "rb") as source:
    edited = client.images.edit(model="gpt-image-2", image=source, prompt="Use a white background")
```

Set `CODEX_GATEWAY_PUBLIC_URL=https://gateway.example.com` to proxy signed native file/plugin uploads and plugin bundle downloads through expiring [transfer URLs](docs/transfers.md).

Image results contain `data[].b64_json`; decode them to save image bytes. Responses supports `stream=True`. Send complete history for HTTP continuation. Hosted web search runs upstream; function and custom tools execute in the caller. Supported fields follow the pinned Codex protocol; unsupported public API options return explicit errors.

## Connect another Codex

Set the same `CODEX_GATEWAY_API_KEY` for the client and add this to its `config.toml`:

```toml
model = "gpt-5.5"
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

Keep the provider name and URL suffix shown above. The actor header enables the pinned client's native image-tool capability; the gateway strips it and authenticates with the Bearer key. Image generation is enabled by default, or explicitly with `codex --enable image_generation`. For native `web.run`, use `codex --enable standalone_web_search --search`.

Backend services use a separate `chatgpt_base_url`; fixed auth, telemetry and distribution origins need explicit routing. See [native configuration and credential rules](docs/native.md).

## API reference

The [complete endpoint index](docs/index.md) covers native backend, account/quota, cloud tasks, files, plugins/skills, Apps/MCP, history/notes, remote control, authentication and distribution. Each logical endpoint has one concise request/response page. See [common rules](docs/common.md) and the [official compatibility mapping](baseline/openai-api-counterparts.json).

| API | Reference |
| --- | --- |
| Public Responses, SSE and hosted search | [POST /v1/responses](docs/responses.md) |
| Native Codex Responses | [POST /codex/responses](docs/codex-responses.md) |
| Responses WebSocket | [GET /v1/responses and native aliases](docs/websocket.md) |
| Remote context compaction | [POST /v1/responses/compact](docs/responses-compact.md) |
| Public model catalog | [GET /v1/models](docs/models.md) |
| Native model catalog | [GET /codex/models](docs/codex-models.md) |
| Image generation | [POST /v1/images/generations and native aliases](docs/images-generations.md) |
| Image editing and SDK uploads | [POST /v1/images/edits and native aliases](docs/images-edits.md) |
| Native search, browsing and related commands | [POST /codex/alpha/search](docs/search.md) |
| File upload | [POST /v1/files](docs/files.md) |
| Realtime calls and sockets | [Calls](docs/realtime-calls.md), [WebSocket](docs/realtime.md), [Live](docs/live-sessions.md) |
| Health and pinned version | [GET /healthz](docs/health.md) |

## Verify

```sh
cargo test --locked
cargo test --locked -p codex-api --lib
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
python3 scripts/fetch-codex-baseline.py
python3 scripts/verify-codex-source.py
cargo test --locked --test gateway actual_codex_cli -- --ignored
python3 scripts/ablation.py
```

Protocol tests compare request headers, compressed HTTP bodies, WebSocket frames and preserved outputs with the pinned official clients. Ablation removes one adapter behavior at a time in an isolated copy and requires its contract test to fail, then verifies the restored baseline; reports go to `artifacts/ablation/`.

Authentication, serialization, compression and event parsing reuse Codex libraries. The vendored API changes add raw transport access; see the [patch](vendor/raw-transport.patch). The gateway uses its own subscription identity and network connections; protocol parity does not imply identical TLS/TCP traffic.

Apache-2.0. Third-party notices: [NOTICE](NOTICE).
