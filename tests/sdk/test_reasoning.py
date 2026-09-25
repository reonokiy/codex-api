"""Actually generate with every effort advertised for the selected gateway models."""
import json
import os

import openai
import pytest

pytestmark = pytest.mark.live


def check_response(response, case, record_property):
    assert response.status == 'completed'
    assert response.output_text.strip() == '42'
    assert response.reasoning is not None
    assert response.reasoning.effort == case['resolved_effort']
    assert response.usage is not None and response.usage.output_tokens > 0
    record_property('model', case['model'])
    record_property('requested_effort', case['effort'])
    record_property('returned_effort', response.reasoning.effort)
    details = response.usage.output_tokens_details
    record_property('reasoning_tokens', details.reasoning_tokens if details else None)


def test_reasoning_effort(live_client: openai.OpenAI, reasoning_case, record_property):
    response = live_client.responses.create(
        model=reasoning_case['model'], input='What is 19 + 23? Reply only with the integer.',
        reasoning={'effort': reasoning_case['effort']},
    )
    check_response(response, reasoning_case, record_property)


@pytest.mark.asyncio
async def test_async_stream_reasoning_effort(live_async_client: openai.AsyncOpenAI, reasoning_case, record_property):
    text, final = [], None
    async with await live_async_client.responses.create(
        model=reasoning_case['model'], input='What is 19 + 23? Reply only with the integer.',
        reasoning={'effort': reasoning_case['effort']}, stream=True,
    ) as stream:
        async for event in stream:
            assert event.type not in ('error', 'response.failed', 'response.incomplete')
            if event.type == 'response.output_text.delta':
                text.append(event.delta)
            elif event.type == 'response.completed':
                final = event.response
    assert final is not None and ''.join(text) == final.output_text
    check_response(final, reasoning_case, record_property)


def test_server_multi_agent_is_explicitly_unsupported(live_client: openai.OpenAI, live_model):
    # This is distinct from a Codex CLI coordinating its own child agents.
    with pytest.raises(openai.APIStatusError) as error:
        live_client.beta.responses.create(
            model=live_model, input='Delegate two small tasks.',
            multi_agent={'enabled': True, 'max_concurrent_subagents': 2},
            betas=['responses_multi_agent=v1'],
        )
    assert error.value.status_code in (400, 422)
    assert 'multi_agent' in error.value.message


def pytest_generate_tests(metafunc):
    if 'reasoning_case' in metafunc.fixturenames:
        value = os.environ.get('CODEX_SDK_REASONING_CASES')
        if not value:
            raise pytest.UsageError('Run cargo e2e real_subscription_reasoning_levels to supply the actual model effort catalog')
        cases = json.loads(value)
        if not cases:
            raise pytest.UsageError('No advertised reasoning levels to test')
        metafunc.parametrize('reasoning_case', cases,
                             ids=[f"{case['model']}-{case['effort']}" for case in cases])
