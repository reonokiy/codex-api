# Task create

`POST /backend-api/wham/tasks` · Alias: `/api/codex/tasks`

**Request:** JSON `{new_task:{environment_id:string,branch:string,run_environment_in_qa_mode:boolean},input_items:[object],metadata?:{best_of_n:integer}}`. Text input is `{type:"message",role:"user",content:[{content_type:"text",text:string}]}`; an optional starting diff is `{type:"pre_apply_patch",output_diff:{diff:string}}`.

**Response:** server JSON containing `{task:{id:string}}` or `{id:string}`; additional task fields are preserved.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/client.rs#L520) · [Native rules](native.md)
