"""Official SDK calls, parameterized across standard and Lite models."""
import base64
import io

import openai
from openai import AsyncOpenAI
from pydantic import BaseModel
import pytest
import pytest_asyncio

from sdk_contract import REJECTED_FIELDS


def completed(response):
    assert response.status == 'completed'
    assert response.output_text == '你好'
    assert response.usage.total_tokens == 15
    assert response.model_extra['future_field'] == {'kept': True}


def test_models(client, config):
    assert set(config['models']) <= {m.id for m in client.models.list()}


def test_models_raw_response(client):
    assert client.models.with_raw_response.list().status_code == 200


def test_authentication_error(client_factory):
    with pytest.raises(openai.AuthenticationError) as error:
        client_factory(key='incorrect').models.list()
    assert error.value.status_code == 401


def test_create(client, model, request_kwargs):
    completed(client.responses.create(**request_kwargs(model)))


def input_cases(config):
    return {
        'message_string': [{'role': 'user', 'content': 'Hello'}],
        'message_parts': [{'role': 'user', 'content': [{'type': 'input_text', 'text': 'Hello'}]}],
        'assistant_history': [{'role': 'assistant', 'content': 'Earlier answer'}, {'role': 'user', 'content': 'Continue'}],
        'encrypted_reasoning': [{'type': 'reasoning', 'id': 'rs_fixture', 'summary': [], 'encrypted_content': 'opaque-fixture'}, {'role': 'user', 'content': 'Continue'}],
        'compaction_history': [{'type': 'compaction', 'id': 'cmp_fixture', 'encrypted_content': 'opaque-fixture'}, {'role': 'user', 'content': 'Continue'}],
        'function_result': [{'type': 'function_call', 'call_id': 'call_fixture', 'name': 'lookup', 'arguments': '{}'}, {'type': 'function_call_output', 'call_id': 'call_fixture', 'output': '42'}],
        'custom_result': [{'type': 'custom_tool_call', 'call_id': 'call_custom', 'name': 'patch', 'input': 'abc'}, {'type': 'custom_tool_call_output', 'call_id': 'call_custom', 'output': 'done'}],
        'input_image': [{'role': 'user', 'content': [{'type': 'input_text', 'text': 'Describe'}, {'type': 'input_image', 'image_url': 'data:image/png;base64,' + config['png'], 'detail': 'auto'}]}],
    }


@pytest.mark.parametrize('kind', list(input_cases({'png': ''})))
def test_input(client, model, config, kind):
    completed(client.responses.create(model=model, input=input_cases(config)[kind]))


@pytest.mark.parametrize('tool', [
    pytest.param({'type': 'function', 'name': 'lookup', 'parameters': {'type': 'object', 'properties': {}, 'additionalProperties': False}, 'strict': True}, id='function'),
    pytest.param({'type': 'custom', 'name': 'patch', 'format': {'type': 'grammar', 'syntax': 'regex', 'definition': '.+'}}, id='custom'),
    pytest.param({'type': 'namespace', 'name': 'tools', 'tools': [{'type': 'function', 'name': 'lookup', 'parameters': {'type': 'object'}}]}, id='namespace'),
    pytest.param({'type': 'web_search', 'filters': {'allowed_domains': ['openai.com']}, 'search_context_size': 'low'}, id='web-search'),
])
def test_tools(client, model, request_kwargs, tool):
    completed(client.responses.create(**request_kwargs(model, tools=[tool])))


def test_hosted_search_all_turns(client, model, request_kwargs):
    completed(client.responses.create(**request_kwargs(
        model, tools=[{'type': 'web_search'}], reasoning={'context': 'all_turns'})))


def test_optional_nulls(client, model, request_kwargs):
    completed(client.responses.create(**request_kwargs(model, reasoning=None, text=None,
        instructions=None, parallel_tool_calls=None, tool_choice=None, service_tier=None,
        include=None, prompt_cache_key=None)))


def test_request_controls(client, model, request_kwargs):
    completed(client.responses.create(**request_kwargs(model, instructions='Be brief', store=False,
        tool_choice='auto', parallel_tool_calls=False, prompt_cache_key='sdk-fixture',
        service_tier='auto', include=['reasoning.encrypted_content'])))


def test_raw_response(client, model, request_kwargs):
    raw = client.responses.with_raw_response.create(**request_kwargs(model))
    assert raw.status_code == 200
    completed(raw.parse())


def test_streaming_wrapper(client, model, request_kwargs):
    with client.responses.with_streaming_response.create(**request_kwargs(model)) as raw:
        completed(raw.parse())


def test_sse(client, model, request_kwargs):
    with client.responses.create(**request_kwargs(model, stream=True, reasoning={'context': 'current_turn'})) as stream:
        events = list(stream)
    assert ''.join(e.delta for e in events if e.type == 'response.output_text.delta') == '你好'
    completed(events[-1].response)


def test_stream_helper(client, model, request_kwargs):
    with client.responses.stream(**request_kwargs(model, reasoning={'context': 'all_turns'})) as stream:
        text = ''.join(event.delta for event in stream if event.type == 'response.output_text.delta')
        final = stream.get_final_response()
    completed(final)
    assert text == final.output_text


def test_websocket_continuation(client, model, request_kwargs):
    with client.responses.connect(max_retries=0,
            websocket_connection_options={'open_timeout': 3, 'close_timeout': 1}) as connection:
        for previous in (None, 'resp_1'):
            params = request_kwargs(model, reasoning={'context': 'current_turn'})
            if previous is not None:
                params['previous_response_id'] = previous
            connection.response.create(**params)
            for event in connection:
                assert event.type != 'error'
                if event.type == 'response.completed':
                    completed(event.response)
                    break
            else:
                pytest.fail('WebSocket ended before completion')


@pytest.mark.parametrize('kind,tool,call_type,result_type', [
    ('function', {'type': 'function', 'name': 'lookup', 'parameters': {'type': 'object'}}, 'function_call', 'function_call_output'),
    ('custom', {'type': 'custom', 'name': 'patch', 'format': {'type': 'grammar', 'syntax': 'regex', 'definition': '.+'}}, 'custom_tool_call', 'custom_tool_call_output'),
], ids=['function', 'custom'])
def test_tool_roundtrip(client_factory, model, kind, tool, call_type, result_type):
    client = client_factory(kind)
    history = [{'role': 'user', 'content': 'Use the tool'}]
    first = client.responses.create(model=model, input=history, tools=[tool])
    call = next(item for item in first.output if item.type == call_type)
    history.extend(first.output)
    history.append({'type': result_type, 'call_id': call.call_id, 'output': '42'})
    completed(client.responses.create(model=model, input=history, tools=[tool]))


class Result(BaseModel):
    ok: bool


def test_parse(client_factory, request_kwargs):
    assert client_factory('structured').responses.parse(**request_kwargs(), text_format=Result).output_parsed == Result(ok=True)


def test_stream_parse(client_factory, request_kwargs):
    with client_factory('structured').responses.stream(**request_kwargs(), text_format=Result) as stream:
        assert stream.get_final_response().output_parsed == Result(ok=True)


def test_compact(client_factory, request_kwargs):
    response = client_factory('compact').responses.compact(**request_kwargs())
    assert response.object == 'response.compaction'
    assert response.output[0].encrypted_content == 'opaque-fixture'


def test_files_create(client_factory):
    assert client_factory('files').files.create(
        file=('fixture.txt', b'fixture data', 'text/plain'), purpose='user_data').id == 'file_test'


def test_images_generate(client, config):
    assert client.images.generate(model='gpt-image-2', prompt='A square').data[0].b64_json == config['png']


def test_images_edit(client, config):
    image = io.BytesIO(base64.b64decode(config['png']))
    image.name = 'fixture.png'
    assert client.images.edit(model='gpt-image-2', prompt='Blue', image=image).data[0].b64_json == config['png']


@pytest.mark.parametrize('params', [
    pytest.param({'store': True}, id='store'),
    pytest.param({'max_output_tokens': 32}, id='max-output-tokens'),
    pytest.param({'previous_response_id': 'resp_1'}, id='previous-response-id'),
    pytest.param({'background': True}, id='background'),
    pytest.param({'tool_choice': 'required'}, id='tool-choice'),
    pytest.param({'text': {'format': {'type': 'json_object'}}}, id='json-object'),
])
def test_unsupported_values(client, request_kwargs, params):
    with pytest.raises(openai.APIStatusError) as error:
        client.responses.create(**request_kwargs(), **params)
    assert error.value.status_code in (400, 422)


@pytest.mark.parametrize('method', ['retrieve', 'delete'])
def test_unsupported_response_resource(client, method):
    with pytest.raises(openai.NotFoundError):
        getattr(client.responses, method)('resp_missing')


def test_unsupported_model_retrieve(client):
    with pytest.raises(openai.NotFoundError):
        client.models.retrieve('missing')


def test_unsupported_chat_completions(client, config):
    with pytest.raises(openai.NotFoundError):
        client.chat.completions.create(model=config['models'][0], messages=[{'role': 'user', 'content': 'Hi'}])


def test_unsupported_files_list(client):
    with pytest.raises(openai.APIStatusError) as error:
        client.files.list()
    assert error.value.status_code == 405


@pytest.mark.parametrize('field', REJECTED_FIELDS)
def test_unsupported_create_parameter(client, request_kwargs, field):
    # Each discovered SDK parameter is actually sent through the SDK to the gateway.
    with pytest.raises(openai.APIStatusError) as error:
        client.responses.create(**request_kwargs(), **{field: None})
    assert error.value.status_code in (400, 422)


def test_transport_options(client, request_kwargs):
    completed(client.responses.create(**request_kwargs(), timeout=3, extra_headers={'session-id': 'sdk-session'}))


def test_rate_limit(client_factory, request_kwargs):
    with pytest.raises(openai.RateLimitError) as error:
        client_factory('limited').responses.create(**request_kwargs())
    assert error.value.status_code == 429


@pytest.mark.parametrize('kind,status', [('failed', 'failed'), ('incomplete', 'incomplete')])
def test_upstream_status(client_factory, request_kwargs, kind, status):
    assert client_factory(kind).responses.create(**request_kwargs()).status == status


@pytest_asyncio.fixture
async def async_client_factory(config):
    from contextlib import AsyncExitStack
    async with AsyncExitStack() as stack:
        async def create(kind='text'):
            return await stack.enter_async_context(AsyncOpenAI(
                base_url=config[kind] + '/v1', api_key='client-key', timeout=5, max_retries=0))
        yield create


@pytest_asyncio.fixture
async def async_client(async_client_factory):
    return await async_client_factory()


@pytest.mark.asyncio
async def test_async_models(async_client, config):
    assert set(config['models']) <= {m.id async for m in await async_client.models.list()}


@pytest.mark.asyncio
async def test_async_create(async_client, model, request_kwargs):
    completed(await async_client.responses.create(**request_kwargs(model, reasoning={'context': 'all_turns'})))


@pytest.mark.asyncio
async def test_async_sse(async_client, model, request_kwargs):
    async with await async_client.responses.create(**request_kwargs(model, stream=True)) as stream:
        events = [event async for event in stream]
    completed(events[-1].response)
    assert ''.join(e.delta for e in events if e.type == 'response.output_text.delta') == events[-1].response.output_text


@pytest.mark.asyncio
async def test_async_stream_helper(async_client, model, request_kwargs):
    async with async_client.responses.stream(**request_kwargs(model)) as stream:
        deltas = [event.delta async for event in stream if event.type == 'response.output_text.delta']
        final = await stream.get_final_response()
    completed(final)
    assert ''.join(deltas) == final.output_text


@pytest.mark.asyncio
async def test_async_raw_response(async_client, model, request_kwargs):
    response = await async_client.responses.with_raw_response.create(**request_kwargs(model))
    completed(response.parse())


@pytest.mark.asyncio
async def test_async_streaming_wrapper(async_client, model, request_kwargs):
    async with async_client.responses.with_streaming_response.create(**request_kwargs(model)) as response:
        completed(await response.parse())


@pytest.mark.asyncio
async def test_async_websocket(async_client, model, request_kwargs):
    async with async_client.responses.connect(max_retries=0,
            websocket_connection_options={'open_timeout': 3, 'close_timeout': 1}) as connection:
        await connection.response.create(**request_kwargs(model, reasoning={'context': 'current_turn'}))
        async for event in connection:
            assert event.type != 'error'
            if event.type == 'response.completed':
                completed(event.response)
                break
        else:
            pytest.fail('Async WebSocket ended early')


@pytest.mark.asyncio
async def test_async_invalid_context(async_client, request_kwargs):
    with pytest.raises(openai.UnprocessableEntityError):
        await async_client.responses.create(**request_kwargs(), reasoning={'context': 'invalid'})


@pytest.mark.asyncio
async def test_async_parse(async_client_factory, request_kwargs):
    client = await async_client_factory('structured')
    assert (await client.responses.parse(**request_kwargs(), text_format=Result)).output_parsed == Result(ok=True)


@pytest.mark.asyncio
async def test_async_stream_parse(async_client_factory, request_kwargs):
    client = await async_client_factory('structured')
    async with client.responses.stream(**request_kwargs(), text_format=Result) as stream:
        assert (await stream.get_final_response()).output_parsed == Result(ok=True)


@pytest.mark.asyncio
async def test_async_files_create(async_client_factory):
    client = await async_client_factory('files')
    assert (await client.files.create(file=('async.txt', b'async fixture', 'text/plain'), purpose='user_data')).id == 'file_test'


@pytest.mark.asyncio
async def test_async_compact(async_client_factory, request_kwargs):
    client = await async_client_factory('compact')
    response = await client.responses.compact(**request_kwargs())
    assert response.object == 'response.compaction'
    assert response.output[0].encrypted_content == 'opaque-fixture'


@pytest.mark.asyncio
async def test_async_images_generate(async_client, config):
    response = await async_client.images.generate(model='gpt-image-2', prompt='A square')
    assert response.data[0].b64_json == config['png']


@pytest.mark.asyncio
async def test_async_images_edit(async_client, config):
    response = await async_client.images.edit(model='gpt-image-2', prompt='Make it blue',
        image=('fixture.png', base64.b64decode(config['png']), 'image/png'))
    assert response.data[0].b64_json == config['png']


@pytest.mark.asyncio
@pytest.mark.parametrize('kind,tool,call_type,result_type', [
    ('function', {'type': 'function', 'name': 'lookup', 'parameters': {'type': 'object'}}, 'function_call', 'function_call_output'),
    ('custom', {'type': 'custom', 'name': 'patch', 'format': {'type': 'grammar', 'syntax': 'regex', 'definition': '.+'}}, 'custom_tool_call', 'custom_tool_call_output'),
], ids=['function', 'custom'])
async def test_async_tool_roundtrip(async_client_factory, model, kind, tool, call_type, result_type):
    client = await async_client_factory(kind)
    history = [{'role': 'user', 'content': 'Use the tool'}]
    first = await client.responses.create(model=model, input=history, tools=[tool])
    call = next(item for item in first.output if item.type == call_type)
    history.extend(first.output)
    history.append({'type': result_type, 'call_id': call.call_id, 'output': '42'})
    response = await client.responses.create(model=model, input=history, tools=[tool])
    completed(response)
