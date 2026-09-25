# Testing

Prepare the pinned official Codex CLI and uv-managed Python SDK once:

```sh
cargo setup
```

Use the Rust toolchain in `rust-toolchain.toml`, Python 3.12+, uv, and the usual
Rust native build dependencies (C/C++ compiler, CMake, OpenSSL development files).
CLI tests currently require Linux x86_64. Setup verifies the release binary's
SHA-256 and version and fetches the matching upstream source. It also installs
the release's Code Mode host and bundled bubblewrap, with pinned SHA-256 checks.
It does not log in.

Run everything with an existing `codex login`:

```sh
cargo test-all
```

This includes Rust and vendored-library tests, official CLI/SDK tests against
local fixtures, source provenance, and real subscription tests. Missing
prerequisites fail instead of silently skipping. Real tests consume subscription
usage; credentials and live reports stay local and are excluded from Git.

## Cargo commands

| Command | Scope |
| --- | --- |
| `cargo setup` | Fetch pinned CLI/source and install locked Python dependencies |
| `cargo test --workspace --locked` | Local Rust regression tests |
| `cargo e2e` | All real upstream suites, sequentially: protocol, SDK, CLI, reasoning and multiagent |
| `cargo test-all` | All tests, including local SDK/CLI, source audit and real upstream; setup and credentials required |

Select individual tests with Cargo's normal name filter instead of separate aliases:

```sh
cargo test --locked --test gateway actual_ -- --ignored
cargo test --locked --test source_audit -- --ignored
cargo e2e real_subscription_openai_sdk --nocapture
```

Optional tooling uses `cargo run --locked -p xtask -- ablation [--case NAME]`
and `cargo run --locked -p xtask -- smoke-container IMAGE`.

Formatting and static analysis remain `cargo fmt --all -- --check` and
`cargo clippy --workspace --locked --all-targets -- -D warnings`.
Container smoke and mutation audits remain explicit commands: they require a
built image/Docker or launch additional Cargo builds, respectively.
CI runs local tests, official clients and source audit with synthetic credentials.
It never reads subscription files or runs real E2E.

## Real test lifecycle

Each live test loads the gateway's ChatGPT credentials from `CODEX_HOME` or
`~/.codex`, starts a loopback gateway on an ephemeral port with a random key, and
closes it on exit. To test an existing gateway, set `CODEX_GATEWAY_LIVE_URL` and
`CODEX_GATEWAY_API_KEY`. This does not deploy or update that gateway.

The CLI test launches the exact pinned release with an empty temporary
`CODEX_HOME`. The downstream CLI gets only the gateway key, never the gateway's
subscription credentials. For GPT-6 Sol (HTTP) and Luna (WebSocket), it reads a
synthetic file through a shell tool and resumes the same thread to recall its
contents. Local gateway counters verify the requested transport was used;
external gateways do not expose these in-process counters.
Run live CLI tests from a normal terminal: another Codex sandbox masks the
daemon socket directory required to initialize the CLI's own read-only sandbox.

The multi-agent test checks both Sol/HTTP and Luna/WebSocket. It verifies two
independent child rollouts with the gateway provider, actual tool outputs,
completed child turns and the parent's combined answer. Child file contents are
random and absent from the parent prompt. Code Mode tool outputs are inspected
because exec JSONL does not project every V2 child event. Reports save only
verification flags and event types, not conversation contents.

The reasoning test uses the locally managed gateway's startup catalog (leave
`CODEX_GATEWAY_LIVE_URL` unset), rather than guessing an enum list. It verifies
the generated answer, returned effort, usage and streamed text for each level.
Aliases are resolved according to the model metadata; an accepted `ultra` may
resolve to `max`. This tests API behavior, not relative reasoning quality or a
guaranteed increase in reasoning-token usage. Results are written to
`artifacts/e2e/sdk/reasoning-results.json`.

Codex-managed subagents are separate from the hosted Responses multi-agent API.
The gateway currently rejects the SDK's
`client.beta.responses.create(multi_agent=..., betas=["responses_multi_agent=v1"])`;
the reasoning suite includes a rejection check and does not count it as support.

The pytest suite calls the official SDK's resource methods directly. It checks
actual results from `models.list`, `responses.create`, SSE, `responses.stream`,
`responses.parse`, streamed structured output, `responses.compact` followed by
continuation, function-call roundtrips, hosted web search, `responses.connect`,
independent async model/response methods, HTTP history and WebSocket continuation,
image input, `files.create`, `images.generate/edit`, and
`realtime.calls.create/connect/session.update/response.create` with received audio.
It has no manual HTTP dispatcher or custom `client.post` fallback. Endpoints
without SDK resource methods (including native standalone search) are excluded
from the Python suite. Native protocol regressions remain Rust tests.

Reports and synthetic/generated test images go under ignored `artifacts/`.
Pytest continues independent checks after failures and returns failure to Cargo.
Client fixtures handle cleanup, parameterized cases have separate results, and
pytest-asyncio runs async calls. Outcome-only JSON reports are written by pytest
hooks; they include setup/teardown failures and never replace pytest's exit status.
No mock substitutes for a live call; SDK generation retries are disabled.
Continuation and compaction tests verify random markers; stream tests compare
SDK deltas with the final response. Select live scenarios with
`CODEX_SDK_LIVE_FILTER='test_generate_text' cargo e2e real_subscription_openai_sdk --nocapture`.
Reports label this selection and include safe failure locations.
Reports omit account response bodies and credentials. Synthetic file uploads
remain upstream because the subscription adapter has no delete endpoint; IDs
are retained in the local report.

Direct Platform-key services, account mutations and microphone transcription are
outside the real suite. A passing run establishes only the implemented coverage.
See [SDK compatibility](sdk-compatibility.md) for parameter limitations.

Pytest is launched through `uv run --locked --no-sync python -m pytest`.
`CODEX_TEST_PYTHON` can
override the interpreter and `CODEX_CLI_BIN` can override the CLI location (its
version must still match). Run `cargo setup` again when client pins change.
