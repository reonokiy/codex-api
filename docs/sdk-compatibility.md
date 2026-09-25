# Official Python SDK compatibility

Run the gateway compatibility suite with the official `openai` Python package:

```sh
cargo setup
cargo test --locked --test gateway actual_openai_ -- --ignored
```

This tests the gateway with the pinned official SDK, not a replacement SDK or the
SDK repository's own client-unit tests. Rust starts isolated loopback gateways and
controlled upstream fixtures. No account login or subscription usage is required.
Dependencies are managed with `pyproject.toml` and `uv.lock`.
Missing Python dependencies fail the suite instead of silently skipping it.

Python cases are collected by **pytest**, with `pytest-asyncio` for async SDK
calls. Rust starts the gateways, runs `uv run --locked --no-sync python -m pytest`,
then checks captured upstream requests. Fixtures close SDK clients; model/input
combinations are parameterized tests with individual node IDs. A failed case
does not prevent independent cases from running, and pytest's exit status fails
the enclosing Cargo test.
Use `cargo test --locked --test gateway actual_openai_ -- --ignored --nocapture` or `cargo e2e real_subscription_openai_sdk --nocapture` to display pytest's
normal progress and summary even when all cases pass.

The modules in `tests/sdk/` cover the compatibility matrix
(`test_compatibility.py`), request parameters (`test_parameters.py`), images and
hosted search (`test_tools.py`), and real upstream (`test_live.py`). Local fixture
configuration is supplied by Cargo through `CODEX_SDK_TEST_CONFIG`.

Test support is separated by responsibility: `local_fixtures.py` owns fake-upstream
clients, `live_fixtures.py` owns real gateway clients, `sdk_contract.py` defines the
parameter contract, and `outcome_report.py` records outcomes and redacts private
live failures. Cases do not import `conftest.py`. The Rust parameter audit matches
requests by model and explicit scenario name rather than pytest execution order.

The SDK suite is also included in `cargo test-all`. Plain `cargo test` runs the
Rust regression tests; SDK tests are opt-in because they require Python packages.

## Coverage

| Area | SDK cases |
| --- | --- |
| Models and authentication | List, raw response, incorrect gateway key |
| Responses HTTP | Sync/async create, raw-response and streaming-response wrappers |
| SSE | Sync/async low-level streams, high-level `responses.stream`, final-response accumulation |
| WebSocket | Official sync/async `responses.connect`; same-connection continuation |
| Inputs | String, message arrays, typed content, image input, assistant history, encrypted reasoning, compaction history, function/custom-tool outputs, two-turn tool-call roundtrips |
| Reasoning | `context`: `auto`, `current_turn`, `all_turns`, null; explicit value preserved for standard and Lite models; legacy `generate_summary` alias |
| Tools | Function, custom grammar, namespaces, hosted web search; existing image/search SDK integration cases |
| Structured output | JSON schema, Pydantic `responses.parse`, parsed streaming, async variants; explicit plain-text format |
| Compaction | Official `responses.compact`, opaque compaction output; Rust verifies forwarded context and trigger |
| Files | Sync/async multipart `files.create`, using the native three-stage upload pipeline |
| Images | Official `images.generate` and multipart `images.edit` |
| Errors | SDK authentication/rate-limit exceptions, failed/incomplete responses, invalid enum values, unsupported parameters and endpoints |

The Responses matrix runs against one standard model and one Responses Lite model
from the pinned catalog. Rust inspects the upstream requests as well as the SDK
results: successful parsing alone is not sufficient. Model identifiers in these
synthetic fixtures do not establish real-account availability.

`artifacts/sdk/{compatibility,parameters,tools}.json` record pytest node IDs,
outcomes and the SDK version. The compatibility
report labels synthetic fixtures explicitly and classifies every top-level
`responses.create` parameter in the installed SDK as supported or rejected. Independent cases continue after a
failure; any failed case makes the command fail. The parameter matrix separately
checks rejected options never reach upstream.

## Parameter audit

The public adapter explicitly maps parameters into Codex requests. It must not
accept options that disappear during serialization.

| Parameter | Behavior |
| --- | --- |
| `reasoning.context` | Accepted and forwarded; explicit `auto`/`current_turn` uses regular Responses because Lite requires `all_turns` |
| `tools=[{"type":"web_search"}]` | Uses regular Responses even for Lite models; Lite does not execute the hosted search tool |
| `reasoning.generate_summary` | Accepted as the deprecated alias of `summary`; supplying both is rejected as ambiguous |
| `text.format.type = "text"` | Accepted as default text output, without a JSON schema requirement |
| `text.format.type = "json_schema"` | Accepted with name and object schema; Pydantic SDK helpers supported |
| `reasoning.mode`, `text.format.type = "json_object"` | Not implemented by the pinned public adapter; rejected |
| `max_output_tokens`, `background`, HTTP `previous_response_id` | Not implemented; rejected instead of ignored |
| `store=true`, forced tool choice | Unsupported by the adapter; rejected |
| Unknown public request fields / invalid enum values | Rejected; native Responses continues preserving opaque request fields |

The original SDK reproduction returned HTTP 422 for all three valid context
values before reaching OpenAI. That was an adapter schema omission, not an
upstream model restriction. Live verification also found that Responses Lite rejects `auto` and `current_turn`.
The public adapter sends those explicit modes through regular Responses, preserving
the requested value, tools and instructions. Omitted/null context retains the
existing Lite default; the native endpoint is unaffected.

## Real-upstream checks

`cargo e2e real_subscription_openai_sdk` directly calls official SDK resource methods against a gateway
backed by real subscription credentials. It checks model listing, authentication
exceptions, Responses (sync, async, SSE, high-level streams, structured parsing,
WebSocket continuation), function-call roundtrips, compaction followed by reuse,
hosted search, file upload, image generation/editing and Realtime calls/session
updates/audio. See [test setup](e2e.md).

### SDK methods exercised against real upstream

Every row below executes the official resource method inside pytest. `live_client`
is an `OpenAI` instance and `live_async_client` is an `AsyncOpenAI` instance;
fixtures close them after each test. These are not mock clients.

| SDK method | Sync / async cases in `tests/sdk/test_live.py` | Result assertions |
| --- | --- | --- |
| `models.list()` | `test_models` / `test_async_models` | Returned catalog contains the selected model |
| `responses.create()` | `test_generate_text` / `test_async_generate_text` | Completed response, generated text and output-token usage |
| `responses.create(stream=True)` | `test_generate_text_stream` / `test_async_generate_text_stream` | SDK event deltas reconstruct the completed response text |
| `responses.stream()` | `test_stream_helper` / `test_async_stream_helper` | Consumed events agree with `get_final_response()` |
| `responses.parse()` | `test_structured_output` / `test_async_structured_output` | Parsed object has the expected field value |
| `responses.stream(text_format=...)` | `test_structured_output_stream` / `test_async_structured_output_stream` | Final parsed object has the expected field value |
| `responses.create(input=history)` | `test_conversation_history` / `test_async_conversation_history` | Second turn recalls a random marker |
| `responses.create(tools=...)` | `test_function_roundtrip` / `test_async_function_roundtrip` | Function call returned, tool result supplied, final answer matches that result |
| `responses.create(tools=[web_search])` | `test_web_search` / `test_async_web_search` | Search executed and sources or citations returned |
| `responses.create(input=[...input_image...])` | `test_image_input` / `test_async_image_input` | Answer identifies the uploaded synthetic image's color |
| `responses.compact()` | `test_compaction_continuation` / `test_async_compaction_continuation` | Returned compaction items can be reused to recall a random marker |
| `responses.connect()` + `connection.response.create()` | `test_websocket_continuation` / `test_async_websocket_continuation` | Same-connection second turn recalls the first turn's marker |
| `files.create()` | `test_files_create` / `test_async_files_create` | Returned file ID, byte count and processed status |
| `images.generate()` | `test_images_generate` / `test_async_images_generate` | Returned base64 decodes to PNG data with positive dimensions |
| `images.edit()` | `test_images_edit` / `test_async_images_edit` | Multipart edit returns PNG data with positive dimensions |
| `realtime.calls.create()` + `realtime.connect()` + session/response methods | Async `test_realtime` | SDP establishes media, session update roundtrips, response completes and audio arrives |

File tests establish successful upload only; this subscription gateway does not
provide SDK file-download/delete resources. Explicit Platform-key routes and
current Live sessions require separate upstream credentials and are outside this
subscription suite. Unsupported-resource exception checks in local tests are
not counted as successful feature coverage.

There is no raw HTTP or custom SDK request fallback in the Python suite. Native
standalone search, health and native catalog routes have no SDK resource method
and are excluded. Continuation passes the SDK's returned conversation items directly to the next
SDK call; checks consume SDK response objects directly.
The JSON report is an output artifact, not an API client implementation.

`cargo e2e real_subscription_codex_cli` separately runs the actual pinned Codex CLI with only a gateway
key, checks shell-tool use and resume against real upstream, and verifies HTTP
and WebSocket usage on the local gateway. `cargo e2e` includes all real suites;
`cargo test-all` includes them alongside local regressions and source audit.

Synthetic SDK tests establish client and wire compatibility; they cannot establish
model behavior, entitlements, quota, or production deployment status. Stored
response CRUD, file listing, Chat Completions and other unimplemented public
resources have SDK exception tests; these do not imply support for those methods.

To select real SDK tests against an already-running gateway, set
`CODEX_GATEWAY_LIVE_URL` and `CODEX_GATEWAY_API_KEY`, then use pytest directly:

```sh
uv run --locked --group live pytest tests/sdk/test_live.py --live-sdk -k test_generate_text
```

`--live-sdk` explicitly enables subscription usage. Standard pytest selection
(`-k` or node IDs) and `--collect-only` work here; Cargo remains the normal entry
point when it should manage the gateway lifecycle. Live failure reports omit
exception bodies and traceback locals to avoid exposing account data.

Cargo can also start the gateway and select live cases with pytest's `-k` syntax:

```sh
CODEX_SDK_LIVE_FILTER='test_generate_text' cargo e2e real_subscription_openai_sdk --nocapture
```

Omit the filter to run every live case. Reports record selected/deselected counts
and the filter, so a selected run is not mistaken for full coverage.

## Generation and continuation assertions

The live module contains independent tests for basic text generation, sync/async
SSE, high-level streaming, structured output and parsed streams. Streaming tests
consume SDK event objects and compare accumulated deltas with the final text.
Basic generation requires completed status, nonempty text and output-token usage;
it does not confuse a model's choice of greeting with an API compatibility error.
HTTP history, sync/async WebSocket continuation and compaction reuse each verify
recall of a random marker absent from the final question. Function calls must
return a separately generated tool result in the follow-up answer.

Image input is checked by asking the model to identify a synthetic red image;
image generation/editing check the returned PNG, and Realtime checks session
updates, response completion and received audio. Each scenario has its own pytest
node ID and failure location; ordinary generation is `test_generate_text`.
