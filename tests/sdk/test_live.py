"""Real SDK behavior, independently selectable with pytest or cargo e2e real_subscription_openai_sdk."""
import asyncio
import base64
import hashlib
import json
import struct
from urllib.parse import urlsplit
import uuid
import zlib

import openai
from openai import AsyncOpenAI, OpenAI
from pydantic import BaseModel
import pytest

pytestmark = pytest.mark.live


class Answer(BaseModel):
    answer: str


def assert_text_response(response, expected=None):
    assert response.status == 'completed'
    assert response.output_text.strip()
    if expected is not None:
        assert response.output_text.strip() == expected
    assert response.id
    assert response.usage is not None and response.usage.output_tokens > 0


def test_generate_text(live_options, live_model):
    """One ordinary SDK call must produce the requested text and token usage."""
    with OpenAI(**live_options) as client:
        response = client.responses.create(model=live_model, input='Write a short greeting.')
    assert response.status == 'completed'
    assert response.output_text.strip()
    assert response.usage is not None and response.usage.output_tokens > 0


def test_models(live_client: OpenAI, live_model):
    models = list(live_client.models.list())
    assert any(model.id == live_model for model in models)


def test_authentication_error(live_options):
    with OpenAI(**(live_options | {'api_key': 'intentionally-wrong-e2e-key'})) as client:
        with pytest.raises(openai.AuthenticationError) as error:
            client.models.list()
    assert error.value.status_code == 401


def test_generate_text_stream(live_client: OpenAI, live_model):
    deltas, final = [], None
    with live_client.responses.create(model=live_model, input='Write a short greeting.', stream=True) as stream:
        for event in stream:
            assert event.type not in ('error', 'response.failed', 'response.incomplete')
            if event.type == 'response.output_text.delta':
                deltas.append(event.delta)
            elif event.type == 'response.completed':
                final = event.response
    assert deltas and final is not None
    assert_text_response(final)
    assert ''.join(deltas) == final.output_text


def test_stream_helper(live_client: OpenAI, live_model):
    with live_client.responses.stream(model=live_model, input='Write a short greeting.') as stream:
        text = ''.join(event.delta for event in stream if event.type == 'response.output_text.delta')
        final = stream.get_final_response()
    assert text == final.output_text
    assert_text_response(final)


def test_structured_output(live_client: OpenAI, live_model):
    response = live_client.responses.parse(model=live_model, input='Reply with answer OK.', text_format=Answer)
    assert response.status == 'completed'
    assert response.output_parsed == Answer(answer='OK')


def test_structured_output_stream(live_client: OpenAI, live_model):
    with live_client.responses.stream(model=live_model, input='Reply with answer OK.', text_format=Answer) as stream:
        response = stream.get_final_response()
    assert response.status == 'completed'
    assert response.output_parsed == Answer(answer='OK')


def test_conversation_history(live_client: OpenAI, live_model):
    marker = 'remember-' + uuid.uuid4().hex
    history = [{'role': 'user', 'content': f'Remember this marker: {marker}. Reply exactly OK.'}]
    first = live_client.responses.create(model=live_model, input=history)
    assert_text_response(first)
    history.extend(first.output)
    history.append({'role': 'user', 'content': 'What marker did I give you? Reply only with that exact marker.'})
    second = live_client.responses.create(model=live_model, input=history)
    assert_text_response(second, marker)
    assert second.id != first.id


def test_compaction_continuation(live_client: OpenAI, live_model):
    marker = 'remember-' + uuid.uuid4().hex
    response = live_client.responses.compact(model=live_model, input=[
        {'role': 'user', 'content': f'Remember this marker for later: {marker}.'},
        {'role': 'assistant', 'content': 'I will remember that marker.'},
    ])
    assert response.object == 'response.compaction'
    assert any(item.type == 'compaction' and item.encrypted_content for item in response.output)
    history = list(response.output)
    history.append({'role': 'user', 'content': 'What marker did I give you? Reply only with that exact marker.'})
    final = live_client.responses.create(model=live_model, input=history)
    assert_text_response(final, marker)


def test_function_roundtrip(live_client: OpenAI, live_model):
    tools = [{'type': 'function', 'name': 'e2e_value', 'strict': True,
              'parameters': {'type': 'object', 'properties': {}, 'additionalProperties': False}}]
    history = [{'role': 'user', 'content': 'Call e2e_value once, then reply with its exact result.'}]
    first = live_client.responses.create(model=live_model, input=history, tools=tools)
    assert first.status == 'completed'
    calls = [item for item in first.output if item.type == 'function_call']
    assert len(calls) == 1 and calls[0].name == 'e2e_value'
    # Function arguments are JSON by API design; the enclosing response is an SDK object.
    assert json.loads(calls[0].arguments) == {}
    marker = 'tool-' + uuid.uuid4().hex
    history.extend(first.output)
    history.append({'type': 'function_call_output', 'call_id': calls[0].call_id, 'output': marker})
    final = live_client.responses.create(model=live_model, input=history, tools=tools)
    assert_text_response(final, marker)


def test_web_search(live_client: OpenAI, live_model, record_property):
    response = live_client.responses.create(model=live_model,
        instructions='Use the provided web_search tool to answer the request. No other tools are available.',
        input='Use web search to find the official OpenAI Codex page. Cite its URL. You must search.',
        tools=[{'type': 'web_search'}], include=['web_search_call.action.sources'])
    assert response.status == 'completed' and response.output_text
    calls = [item for item in response.output if item.type == 'web_search_call']
    assert calls and all(call.status == 'completed' for call in calls)
    sources = [source for call in calls for source in (getattr(call.action, 'sources', None) or [])
               if source.url.startswith('https://')]
    citations = [annotation for item in response.output if item.type == 'message'
                 for part in item.content if part.type == 'output_text'
                 for annotation in part.annotations if annotation.type == 'url_citation']
    assert sources or citations
    record_property('source_count', len(sources))
    record_property('citation_count', len(citations))


def test_websocket_continuation(live_client: OpenAI, live_model):
    marker = 'remember-' + uuid.uuid4().hex
    with live_client.responses.connect(max_retries=0,
            websocket_connection_options={'open_timeout': 30, 'close_timeout': 5}) as connection:
        previous = None
        for prompt, expected in [
            (f'Remember this marker: {marker}. Acknowledge briefly.', None),
            ('What marker did I give you? Reply only with that exact marker.', marker),
        ]:
            params = dict(model=live_model, input=prompt)
            if previous is not None:
                params['previous_response_id'] = previous
            connection.response.create(**params)
            for event in connection:
                assert event.type not in ('error', 'response.failed', 'response.incomplete')
                if event.type == 'response.completed':
                    assert_text_response(event.response, expected)
                    assert event.response.id != previous
                    previous = event.response.id
                    break
            else:
                pytest.fail('WebSocket ended before completion')


@pytest.mark.asyncio
async def test_async_models(live_async_client: AsyncOpenAI, live_model):
    models = [model async for model in await live_async_client.models.list()]
    assert any(model.id == live_model for model in models)


@pytest.mark.asyncio
async def test_async_generate_text(live_options, live_model):
    async with AsyncOpenAI(**live_options) as client:
        response = await client.responses.create(model=live_model, input='Write a short greeting.')
    assert_text_response(response)


@pytest.mark.asyncio
async def test_async_generate_text_stream(live_async_client: AsyncOpenAI, live_model):
    deltas, final = [], None
    async with await live_async_client.responses.create(model=live_model, input='Write a short greeting.', stream=True) as stream:
        async for event in stream:
            assert event.type not in ('error', 'response.failed', 'response.incomplete')
            if event.type == 'response.output_text.delta':
                deltas.append(event.delta)
            elif event.type == 'response.completed':
                final = event.response
    assert deltas and final is not None
    assert_text_response(final)
    assert ''.join(deltas) == final.output_text


@pytest.mark.asyncio
async def test_async_stream_helper(live_async_client: AsyncOpenAI, live_model):
    async with live_async_client.responses.stream(model=live_model, input='Write a short greeting.') as stream:
        deltas = [event.delta async for event in stream if event.type == 'response.output_text.delta']
        final = await stream.get_final_response()
    assert ''.join(deltas) == final.output_text
    assert_text_response(final)


@pytest.mark.asyncio
async def test_async_structured_output(live_async_client: AsyncOpenAI, live_model):
    response = await live_async_client.responses.parse(model=live_model, input='Reply with answer OK.', text_format=Answer)
    assert response.status == 'completed'
    assert response.output_parsed == Answer(answer='OK')


@pytest.mark.asyncio
async def test_async_structured_output_stream(live_async_client: AsyncOpenAI, live_model):
    async with live_async_client.responses.stream(model=live_model, input='Reply with answer OK.', text_format=Answer) as stream:
        response = await stream.get_final_response()
    assert response.status == 'completed'
    assert response.output_parsed == Answer(answer='OK')


@pytest.mark.asyncio
async def test_async_websocket_continuation(live_async_client: AsyncOpenAI, live_model):
    marker = 'remember-' + uuid.uuid4().hex
    async with live_async_client.responses.connect(max_retries=0,
            websocket_connection_options={'open_timeout': 30, 'close_timeout': 5}) as connection:
        previous = None
        for prompt, expected in [
            (f'Remember this marker: {marker}. Acknowledge briefly.', None),
            ('What marker did I give you? Reply only with that exact marker.', marker),
        ]:
            params = dict(model=live_model, input=prompt)
            if previous is not None:
                params['previous_response_id'] = previous
            await connection.response.create(**params)
            async for event in connection:
                assert event.type not in ('error', 'response.failed', 'response.incomplete')
                if event.type == 'response.completed':
                    assert_text_response(event.response, expected)
                    assert event.response.id != previous
                    previous = event.response.id
                    break
            else:
                pytest.fail('WebSocket ended before completion')


def png():
    def chunk(kind, data):
        return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', 32, 32, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress((b'\0' + b'\xff\0\0' * 32) * 32)) + chunk(b'IEND', b''))


def test_image_input(live_client: OpenAI, live_model):
    image = 'data:image/png;base64,' + base64.b64encode(png()).decode()
    response = live_client.responses.create(model=live_model, input=[{
        'role': 'user', 'content': [
            {'type': 'input_text', 'text': 'What primary color fills this image? Reply exactly RED, GREEN, or BLUE.'},
            {'type': 'input_image', 'image_url': image},
        ],
    }])
    assert_text_response(response, 'RED')


def test_files_create(live_client: OpenAI, record_property):
    data = png()
    response = live_client.files.create(file=('codex-sdk-e2e.png', data, 'image/png'), purpose='user_data')
    assert response.object == 'file' and response.id
    assert response.bytes == len(data) and response.status == 'processed'
    record_property('file_id', response.id)
    record_property('bytes', response.bytes)
    record_property('cleanup', 'subscription adapter has no delete endpoint')


def assert_image(response, name, sdk_artifacts, record_property):
    assert response.data and response.data[0].b64_json
    data = base64.b64decode(response.data[0].b64_json, validate=True)
    assert data.startswith(b'\x89PNG\r\n\x1a\n') and b'IEND' in data[-12:]
    width, height = struct.unpack('!II', data[16:24])
    assert width > 0 and height > 0
    (sdk_artifacts / name).write_bytes(data)
    for name, value in dict(bytes=len(data), width=width, height=height, sha256=hashlib.sha256(data).hexdigest()).items():
        record_property(name, value)


def test_images_generate(live_client: OpenAI, sdk_artifacts, record_property):
    response = live_client.images.generate(model='gpt-image-2',
        prompt='A plain blue square on a white background.', quality='low')
    assert_image(response, 'generation.png', sdk_artifacts, record_property)


def test_images_edit(live_client: OpenAI, sdk_artifacts, record_property):
    response = live_client.images.edit(model='gpt-image-2', image=('e2e.png', png(), 'image/png'),
        prompt='Make this square blue.', quality='low')
    assert_image(response, 'edit.png', sdk_artifacts, record_property)


@pytest.mark.asyncio
async def test_realtime(live_async_client: AsyncOpenAI, record_property):
    from aiortc import RTCPeerConnection, RTCConfiguration, RTCSessionDescription

    async def call():
        peer = RTCPeerConnection(RTCConfiguration(iceServers=[]))
        peer.createDataChannel('oai-events')
        received_audio = asyncio.Event()
        tasks = []

        @peer.on('track')
        def track_received(track):

            async def receive():
                await track.recv()
                received_audio.set()
            tasks.append(asyncio.create_task(receive()))
        try:
            peer.addTransceiver('audio', direction='recvonly')
            await peer.setLocalDescription(await peer.createOffer())
            client = live_async_client
            result = await client.realtime.calls.create(sdp=peer.localDescription.sdp, session={'type': 'realtime', 'model': 'gpt-realtime'}, timeout=60)
            answer = (await result.aread()).decode()
            assert answer.startswith('v=0'), 'expected SDP answer'
            location = result.response.headers.get('location')
            assert location, 'missing call Location'
            await peer.setRemoteDescription(RTCSessionDescription(sdp=answer, type='answer'))
            call_id = urlsplit(location).path.rsplit('/', 1)[-1]
            async with client.realtime.connect(call_id=call_id, max_retries=0, websocket_connection_options={'open_timeout': 20, 'close_timeout': 5}) as connection:
                marker = 'Reply briefly. Session marker: ' + uuid.uuid4().hex
                await connection.session.update(session={'type': 'realtime', 'instructions': marker})

                async def wait_for(kind):
                    async for event in connection:
                        assert event.type != 'error', 'Realtime returned error'
                        if event.type == kind:
                            return event
                    raise AssertionError('Realtime connection ended early')
                event = await asyncio.wait_for(wait_for('session.updated'), 15)
                assert event.session.instructions == marker, 'session update did not roundtrip'
                await connection.response.create(response={'instructions': 'Say hello briefly.', 'output_modalities': ['audio']})
                event = await asyncio.wait_for(wait_for('response.done'), 45)
                assert event.response.status == 'completed', 'Realtime response did not complete'
                await asyncio.wait_for(received_audio.wait(), 15)
            return {'session_updated': True, 'response_completed': True, 'audio_frame_received': True}
        finally:
            for task in tasks:
                task.cancel()
            await asyncio.gather(*tasks, return_exceptions=True)
            await peer.close()
    for name, value in (await call()).items():
        record_property(name, value)


@pytest.mark.asyncio
async def test_async_conversation_history(live_async_client: AsyncOpenAI, live_model):
    marker = 'remember-' + uuid.uuid4().hex
    history = [{'role': 'user', 'content': f'Remember this marker: {marker}. Reply exactly OK.'}]
    first = await live_async_client.responses.create(model=live_model, input=history)
    assert_text_response(first)
    history.extend(first.output)
    history.append({'role': 'user', 'content': 'What marker did I give you? Reply only with that exact marker.'})
    second = await live_async_client.responses.create(model=live_model, input=history)
    assert_text_response(second, marker)
    assert second.id != first.id


@pytest.mark.asyncio
async def test_async_compaction_continuation(live_async_client: AsyncOpenAI, live_model):
    marker = 'remember-' + uuid.uuid4().hex
    response = await live_async_client.responses.compact(model=live_model, input=[{'role': 'user', 'content': f'Remember this marker for later: {marker}.'}, {'role': 'assistant', 'content': 'I will remember that marker.'}])
    assert response.object == 'response.compaction'
    assert any((item.type == 'compaction' and item.encrypted_content for item in response.output))
    history = list(response.output)
    history.append({'role': 'user', 'content': 'What marker did I give you? Reply only with that exact marker.'})
    final = await live_async_client.responses.create(model=live_model, input=history)
    assert_text_response(final, marker)


@pytest.mark.asyncio
async def test_async_function_roundtrip(live_async_client: AsyncOpenAI, live_model):
    tools = [{'type': 'function', 'name': 'e2e_value', 'strict': True, 'parameters': {'type': 'object', 'properties': {}, 'additionalProperties': False}}]
    history = [{'role': 'user', 'content': 'Call e2e_value once, then reply with its exact result.'}]
    first = await live_async_client.responses.create(model=live_model, input=history, tools=tools)
    assert first.status == 'completed'
    calls = [item for item in first.output if item.type == 'function_call']
    assert len(calls) == 1 and calls[0].name == 'e2e_value'
    assert json.loads(calls[0].arguments) == {}
    marker = 'tool-' + uuid.uuid4().hex
    history.extend(first.output)
    history.append({'type': 'function_call_output', 'call_id': calls[0].call_id, 'output': marker})
    final = await live_async_client.responses.create(model=live_model, input=history, tools=tools)
    assert_text_response(final, marker)


@pytest.mark.asyncio
async def test_async_web_search(live_async_client: AsyncOpenAI, live_model, record_property):
    response = await live_async_client.responses.create(model=live_model, instructions='Use the provided web_search tool to answer the request. No other tools are available.', input='Use web search to find the official OpenAI Codex page. Cite its URL. You must search.', tools=[{'type': 'web_search'}], include=['web_search_call.action.sources'])
    assert response.status == 'completed' and response.output_text
    calls = [item for item in response.output if item.type == 'web_search_call']
    assert calls and all((call.status == 'completed' for call in calls))
    sources = [source for call in calls for source in getattr(call.action, 'sources', None) or [] if source.url.startswith('https://')]
    citations = [annotation for item in response.output if item.type == 'message' for part in item.content if part.type == 'output_text' for annotation in part.annotations if annotation.type == 'url_citation']
    assert sources or citations
    record_property('source_count', len(sources))
    record_property('citation_count', len(citations))


@pytest.mark.asyncio
async def test_async_image_input(live_async_client: AsyncOpenAI, live_model):
    image = 'data:image/png;base64,' + base64.b64encode(png()).decode()
    response = await live_async_client.responses.create(model=live_model, input=[{'role': 'user', 'content': [{'type': 'input_text', 'text': 'What primary color fills this image? Reply exactly RED, GREEN, or BLUE.'}, {'type': 'input_image', 'image_url': image}]}])
    assert_text_response(response, 'RED')


@pytest.mark.asyncio
async def test_async_files_create(live_async_client: AsyncOpenAI, record_property):
    data = png()
    response = await live_async_client.files.create(file=('codex-sdk-e2e.png', data, 'image/png'), purpose='user_data')
    assert response.object == 'file' and response.id
    assert response.bytes == len(data) and response.status == 'processed'
    record_property('file_id', response.id)
    record_property('bytes', response.bytes)
    record_property('cleanup', 'subscription adapter has no delete endpoint')


@pytest.mark.asyncio
async def test_async_images_generate(live_async_client: AsyncOpenAI, sdk_artifacts, record_property):
    response = await live_async_client.images.generate(model='gpt-image-2', prompt='A plain blue square on a white background.', quality='low')
    assert_image(response, 'async-generation.png', sdk_artifacts, record_property)


@pytest.mark.asyncio
async def test_async_images_edit(live_async_client: AsyncOpenAI, sdk_artifacts, record_property):
    response = await live_async_client.images.edit(model='gpt-image-2', image=('e2e.png', png(), 'image/png'), prompt='Make this square blue.', quality='low')
    assert_image(response, 'async-edit.png', sdk_artifacts, record_property)
