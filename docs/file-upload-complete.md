# File upload complete

`POST /backend-api/files/{file_id}/uploaded`

**Request:** `{} or {pdf_c2pa_create_request:<reservation create body>}`.

**Response:** `{status:string,download_url?:string,file_name?:string,mime_type?:string,error_message?:string,file_size_bytes?:integer}`.

The source can finalize a PDF with the server-provided C2PA reservation. A returned `download_url` remains the original signed URL. File IDs belong to this subscription backend.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/codex-api/src/files.rs#L256) · [Native rules](native.md)
