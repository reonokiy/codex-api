"""Official SDK parameter coverage against the Rust test's captured fake upstream."""
import sys
from openai import APIStatusError, OpenAI

client = OpenAI(base_url=sys.argv[1] + '/v1', api_key='client-key', max_retries=0)
for model in sys.argv[2:]:
    for context in ('auto', 'current_turn', 'all_turns', None):
        for stream in (False, True):
            response = client.responses.create(
                model=model, input='Hello', stream=stream,
                reasoning={'context': context},
            )
            if stream:
                with response:
                    assert list(response)[-1].response.status == 'completed'
            else:
                assert response.status == 'completed'
    for params in (
        {'reasoning': {'generate_summary': 'auto'}},
        {'text': {'format': {'type': 'text'}}},
        {'text': {'format': {'type': 'json_schema', 'name': 'result', 'schema': {'type': 'object'}, 'strict': True}}},
    ):
        assert client.responses.create(model=model, input='Hello', **params).status == 'completed'
    for params in (
        {'reasoning': {'context': 'typo'}},
        {'reasoning': {'context': 1}},
        {'reasoning': {'mode': 'pro'}},
        {'reasoning': {'future_field': True}},
        {'text': {'format': {'type': 'text', 'schema': {}}}},
        {'text': {'format': {'type': 'json_object'}}},
        {'max_output_tokens': 32},
        {'background': True},
        {'store': True},
    ):
        try:
            client.responses.create(model=model, input='Hello', extra_body=params)
        except APIStatusError as error:
            assert error.status_code in (400, 422), error.status_code
        else:
            raise AssertionError(f'Unsupported parameter accepted: {params}')
print('SDK type matrix passed: 40 requests across standard and Lite models')
