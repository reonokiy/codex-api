# Edit images

`POST /v1/images/edits`  
Native: `/codex/images/edits`, `/backend-api/codex/images/edits`  
[Common rules](common.md)

## Request

The public path accepts multipart uploads or native JSON. Native paths accept JSON only. Shared parameters and defaults match [image generation](images-generations.md).

| Image field | Format |
| --- | --- |
| Multipart `image` or repeated `image[]` | PNG, JPEG or WebP binary uploads; 1–5 images |
| JSON `images` | Array of 1–5 `{ "image_url": string }` objects; HTTPS URLs or image data URLs |

The total request limit is 16 MiB, including JSON/Base64 or multipart overhead. Uploads are converted to data URLs in memory; the gateway does not write image files or fetch supplied URLs. Duplicate text form fields are rejected.

```python
with open("bird.png", "rb") as image:
    result = client.images.edit(
        model="gpt-image-2", image=image, prompt="Make the background white",
    )
```

Native JSON:

```json
{"model":"gpt-image-2","prompt":"Make the background white","images":[{"image_url":"data:image/png;base64,..."}]}
```

Multipart text fields: `prompt`, `model`, `n`, `size`, `quality`, `background`, `response_format`, `output_format`. `n` is parsed as an unsigned integer. Local filenames are not valid JSON references.

## Response

Same [response schema](images-generations.md#response) as image generation, including Base64 image data, optional metadata and upstream request ID.

Mask uploads, image variations, `input_fidelity`, image streaming and unlisted options are unsupported by this adapter. SDK compatibility covers the documented generation/edit subset.
