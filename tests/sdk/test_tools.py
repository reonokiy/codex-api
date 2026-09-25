"""SDK image and hosted-search calls against Cargo's captured upstream."""
import base64
import io


def test_images_generate_and_edit(client):
    result = client.images.generate(model='gpt-image-2', prompt='A square')
    assert result.data[0].b64_json
    image = io.BytesIO(base64.b64decode(result.data[0].b64_json))
    image.name = 'square.png'
    edited = client.images.edit(model='gpt-image-2', image=image, prompt='Make it blue')
    assert edited.data[0].b64_json


def test_hosted_search(client, config):
    result = client.responses.create(
        model=config['models'][0], input='Search for Codex', tools=[{'type': 'web_search'}],
        include=['web_search_call.action.sources'],
    )
    assert result.output[0].type == 'web_search_call'
    assert result.output_text == '你好'


def test_hosted_search_stream(client, config):
    with client.responses.create(
        model=config['models'][0], input='Search again', tools=[{'type': 'web_search'}], stream=True,
    ) as stream:
        events = list(stream)
    assert any(event.type == 'response.web_search_call.searching' for event in events)
    assert events[-1].response.output[0].type == 'web_search_call'
