# Testing

After the one-time setup, run the full suite with one Cargo command:

```sh
cargo test-all
```

The alias expands to `cargo test --workspace --locked -- --include-ignored`: gateway
and vendored-library tests, official CLI/SDK compatibility tests and real
subscription E2E. Missing prerequisites fail the command rather than silently
skipping tests. Real E2E consumes subscription usage.

## One-time setup

Use the Rust toolchain in `rust-toolchain.toml` and an existing `codex login`.
Install the test clients:

```sh
python3 scripts/fetch-codex-baseline.py
uv sync --locked --group live
```

Python dependencies are managed by `pyproject.toml` and `uv.lock`. For SDK tests
without live Realtime media, `uv sync --locked` is sufficient. Cargo launches
Python through `uv run --locked --no-sync`; `CODEX_TEST_PYTHON` can override the
interpreter for diagnostics. Run `uv sync` explicitly before testing.
The CLI fixtures currently require the pinned Linux
x86_64 release binary. `.venv/`, cached clients and reports are excluded from Git.

No manually running gateway or gateway API key is needed. Each real test loads
ChatGPT credentials from `CODEX_HOME` or `~/.codex`, starts a gateway on an ephemeral
loopback port with a random gateway key, and closes it on exit. The Python helper
is owned by the Rust test and terminated on timeout. To test an existing gateway,
set `CODEX_GATEWAY_LIVE_URL` and `CODEX_GATEWAY_API_KEY` instead.

## Smaller runs

| Command | Scope |
| --- | --- |
| `cargo test --workspace --locked` | Local tests, without real credentials or upstream usage |
| `cargo sdk` | Official Python SDK compatibility suite against local fixtures; no account needed |
| `cargo e2e` | Both real subscription suites, with automatic gateway lifecycle |
| `cargo e2e real_subscription_openai_sdk` | Official SDK real-upstream suite only |
| `cargo test --locked --test gateway actual_ -- --ignored` | Official-client tests against fake upstreams |

Formatting and static analysis remain standard Cargo commands:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
```

Source provenance and protocol ablation are separate audits:
`python3 scripts/verify-codex-source.py` and `python3 scripts/ablation.py`.

## Coverage and output

The Rust suite verifies public/native Responses HTTP and WebSocket, warmup,
continuation, Lite and compaction across two models. The official OpenAI Python
SDK suite covers models, Responses/SSE, function-call continuation, compaction,
hosted search, native search/page opening, file upload, image generation/editing
and Realtime WebRTC audio reception plus sideband session updates. Native search
uses the SDK custom POST method; health/auth and native model probes use HTTP.

Reports and decoded test images go under ignored `artifacts/`. The SDK continues
independent checks after failures and reports a failed exit status to Cargo. It
never substitutes a mock or automatically retries a generation. No credentials
or private account response bodies are saved. Small synthetic PNG uploads remain
upstream because the subscription adapter has no delete endpoint; their IDs are
retained in the local report.

Direct Platform-key services, account mutations, microphone transcription and
interruptions are outside the real suite. Passing the command means the
implemented suite passed, not that every native endpoint or entitlement was tested.

Detailed SDK coverage and parameter limitations: [SDK compatibility](sdk-compatibility.md).
