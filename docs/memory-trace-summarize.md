# Memory trace summarize

`POST /backend-api/codex/memories/trace_summarize` · Alias: `/codex/memories/trace_summarize`

**Request:** JSON `{model:string,traces:[{id:string,metadata:{source_path:string},items:JSON[]}],reasoning?:{effort?:string,summary?:string}}`. Trace items are opaque JSON values.

**Response:** JSON `{output:[{trace_summary:string,memory_summary:string}]}`. The original parser also accepts `raw_memory` as an alias of `trace_summary`.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/codex-api/src/endpoint/memories.rs#L33) · [Native rules](native.md)
