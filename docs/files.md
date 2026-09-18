# File upload

`POST /v1/files`  
[Common rules](common.md)

**Request:** `multipart/form-data` with exactly one `file` part containing bytes and a nonempty filename, and one `purpose` field set to `user_data`. The complete multipart request, including overhead, is limited to 16 MiB. Other purposes, duplicate fields and `expires_after` are rejected.

```python
from openai import OpenAI

client = OpenAI(base_url="http://127.0.0.1:8080/v1", api_key="your-gateway-key")
with open("report.pdf", "rb") as source:
    uploaded = client.files.create(file=source, purpose="user_data")
print(uploaded.id)
```

**Response:** a public FileObject shape:

```json
{"id":"file-example","object":"file","bytes":1024,"created_at":1789689600,"filename":"report.pdf","purpose":"user_data","status":"processed"}
```

The original Codex library reserves the file, uploads to signed storage and finalizes it, retaining one subscription identity throughout. The ID and canonical file metadata come from the native backend; `created_at` is the gateway's successful upload time. Storage requests contain no account credentials.

This adapter covers upload only. It does not invent metadata retrieval, listing, deletion or content-download routes for native file IDs. The separate [Platform namespace](platform.md) supports those official operations using actual Platform credentials and Platform file IDs.

[Pinned upload pipeline](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/codex-api/src/files.rs#L95) · [Official upload API](https://developers.openai.com/api/reference/resources/files/methods/create)
