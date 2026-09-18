# File create

`POST /backend-api/files`

**Request:** `{file_name:string,file_size:integer,use_case:"codex",codex_connector_id?:string,codex_action_name?:string,codex_model?:string}`.

**Response:** `{file_id:string,upload_url:string,pdf_c2pa_reservation?:object}`.

Upload the file to `upload_url`, then call [upload completion](file-upload-complete.md). With `CODEX_GATEWAY_PUBLIC_URL`, the upload URL becomes a method-bound [transfer URL](transfers.md).

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/codex-api/src/files.rs#L138) · [Native rules](native.md)
