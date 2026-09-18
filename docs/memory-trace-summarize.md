# Memory trace summarize

`POST /backend-api/codex/memories/trace_summarize` · Alias: `/codex/memories/trace_summarize`

**Request:** JSON `{model:string,traces:[{id:string,metadata:{source_path:string},items:JSON[]}],reasoning?:{effort?:string,summary?:string}}`. Trace items are opaque JSON values.

**Response:** JSON `{output:[{trace_summary:string,memory_summary:string}]}`. The original parser also accepts `raw_memory` as an alias of `trace_summary`.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/codex-api/src/endpoint/memories.rs#L33) · [Native rules](native.md)
