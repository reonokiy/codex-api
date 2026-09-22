# Codex API Gateway

A gateway that uses your ChatGPT subscription through the Codex Rust libraries.

Supports Responses, streaming, WebSocket, images, files, search and Realtime.
Built on Codex **0.155.1** and Rust **1.95.0**.

The gateway loads your account's model catalog at startup, including
`gpt-6-sol` and `gpt-6-luna` when available. Restart to refresh the catalog;
see [model discovery and fallback behavior](docs/models.md).

## Run

Start the gateway with a writable data directory:

```sh
mkdir -p "$HOME/.codex"
export CODEX_GATEWAY_API_KEY='your-gateway-key'

docker run -d --name codex-api --restart unless-stopped \
  --user "$(id -u):$(id -g)" \
  -p 127.0.0.1:8080:8080 \
  -e CODEX_GATEWAY_API_KEY \
  -v "$HOME/.codex:/data" \
  ghcr.io/reonokiy/codex-api:latest
```

If no credentials or `auth.json` exist, the gateway prints a device login link
and code in `docker logs -f codex-api`. Complete login in your browser within
10 minutes; credentials are saved to `/data/auth.json` and the gateway starts.
On timeout it exits with an error and the restart policy retries. Existing
Codex credentials are reused. See [login details](docs/common.md#startup-login).

Or run from source with the same login and environment variable:

```sh
cargo run --locked --release -- --listen 127.0.0.1:8080
```

## Use

Point the OpenAI Python SDK at the gateway:

```python
from openai import OpenAI

client = OpenAI(base_url="http://127.0.0.1:8080/v1", api_key="your-gateway-key")
response = client.responses.create(model="gpt-5.5", input="Hello")
print(response.output_text)
```

Use Responses instead of Chat Completions. Supported parameters and account
requirements vary by endpoint; see the [API reference](docs/index.md).

- [Connect another Codex](docs/native.md#connect-another-codex)
- [Standalone search](docs/search.md)
- [Image generation](docs/images-generations.md) and [editing](docs/images-edits.md)
- [Authentication and limits](docs/common.md)

## Test

Complete the [one-time setup](docs/e2e.md), then run:

```sh
cargo test-all
```

This includes real subscription E2E and consumes usage.
For local tests only: `cargo test --workspace --locked`.

Apache-2.0 · [Third-party notices](NOTICE)
