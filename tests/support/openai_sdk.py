"""Run only against the integration test's local fake upstream."""
import base64
import io
import sys
from openai import OpenAI

client = OpenAI(base_url=sys.argv[1] + '/v1', api_key='client-key', max_retries=0)
result = client.images.generate(model='gpt-image-2', prompt='A square')
assert result.data[0].b64_json
image = io.BytesIO(base64.b64decode(result.data[0].b64_json))
image.name = 'square.png'
edited = client.images.edit(model='gpt-image-2', image=image, prompt='Make it blue')
assert edited.data[0].b64_json
result = client.responses.create(
    model=sys.argv[2], input='Search for Codex', tools=[{'type': 'web_search'}],
    include=['web_search_call.action.sources'],
)
assert result.output[0].type == 'web_search_call'
assert result.output_text == '你好'
with client.responses.create(
    model=sys.argv[2], input='Search again', tools=[{'type': 'web_search'}], stream=True,
) as stream:
    events = list(stream)
assert any(event.type == 'response.web_search_call.searching' for event in events)
assert events[-1].response.output[0].type == 'web_search_call'
print('OpenAI Python SDK: generation, multipart edit, web search and SSE passed')
