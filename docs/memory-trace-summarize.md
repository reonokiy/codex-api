# Memory trace summarize

`POST /backend-api/codex/memories/trace_summarize` · Alias: `/codex/memories/trace_summarize`

**Request:** JSON `{model:string,traces:[{id:string,metadata:{source_path:string},items:JSON[]}],reasoning?:{effort?:string,summary?:string}}`. Trace items are opaque JSON values.

**Response:** JSON `{output:[{trace_summary:string,memory_summary:string}]}`. Native responses are forwarded without parsing, including `raw_memory` when returned upstream. Status, headers, query and request/response body bytes are preserved; upstream validates requests.

`POST /v1/memories/trace_summarize` forwards the separate public API-key contract using caller credentials; see [Platform forwarding](platform.md).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/codex-api/src/endpoint/memories.rs#L33) · [Native rules](native.md)
