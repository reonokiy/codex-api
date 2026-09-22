# File upload complete

`POST /backend-api/files/{file_id}/uploaded`

**Request:** `{} or {pdf_c2pa_create_request:<reservation create body>}`.

**Response:** `{status:string,download_url?:string,file_name?:string,mime_type?:string,error_message?:string,file_size_bytes?:integer}`.

The source can finalize a PDF with the server-provided C2PA reservation. A returned `download_url` remains the original signed URL. File IDs belong to this subscription backend.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/codex-api/src/files.rs#L256) · [Native rules](native.md)
