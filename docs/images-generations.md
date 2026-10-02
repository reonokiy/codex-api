# Generate images

`POST /v1/images/generations`  
Native: `/codex/images/generations`, `/backend-api/codex/images/generations`  
[Common rules](common.md)

## JSON request

```json
{"model":"gpt-image-2","prompt":"A blue bird","size":"auto","quality":"auto","background":"auto"}
```

| Field | Type / behavior |
| --- | --- |
| `prompt` | Required non-empty string |
| `model` | Non-empty string; public default `gpt-image-2`, required on native paths |
| `n` | Optional integer, 1–10; upstream model limits also apply |
| `size` | Optional non-empty string, e.g. `auto`; upstream validates supported dimensions |
| `quality` | `auto`, `low`, `medium`, `high` |
| `background` | `auto`, `opaque`, `transparent` |
| `response_format` | Public only; omitted or `b64_json` |
| `output_format` | Public only; omitted or `png` |

Public defaults for size, quality and background are `auto`, matching the Codex image extension. Native requests preserve omitted optional fields. The public adapter uses Codex's default User-Agent/originator and creates `x-codex-image-turn-id` if absent. Native Codex metadata is forwarded.

## Response

```json
{"created":1780000000,"data":[{"b64_json":"...","generation_id":"..."}],"background":"opaque","quality":"low","size":"1024x1024"}
```

`created` and `data[].b64_json` are the core fields. Generation IDs, output metadata, usage and additional upstream fields are preserved when present. `x-codex-imagegen-request-id` is forwarded when returned. Decode `b64_json` to get the image bytes.

```python
import base64
from pathlib import Path

result = client.images.generate(model="gpt-image-2", prompt="A blue bird")
Path("bird.png").write_bytes(base64.b64decode(result.data[0].b64_json))
```

This endpoint uses the original `ImagesClient`. URL output, image streaming, output compression, non-PNG output and unlisted parameters are unsupported. It does not implement the Responses `image_generation` tool.
