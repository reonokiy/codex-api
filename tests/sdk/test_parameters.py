"""Each SDK parameter combination is an independently reported pytest case."""
from openai import APIStatusError
import pytest

CASES = [
    pytest.param({'reasoning': {'context': context}}, stream, True,
                 id=f'context-{context}-stream-{stream}')
    for context in ('auto', 'current_turn', 'all_turns', None)
    for stream in (False, True)
] + [
    pytest.param({'max_output_tokens': limit}, stream, True,
                 id=f'max-output-tokens-{name}-stream-{stream}')
    for name, limit in [('positive', 32), ('null', None)]
    for stream in (False, True)
] + [
    pytest.param({'safety_identifier': identifier}, stream, True,
                 id=f'safety-identifier-{name}-stream-{stream}')
    for name, identifier in [('string', 'user-compatibility'), ('null', None)]
    for stream in (False, True)
] + [
    pytest.param(params, False, True, id=name) for name, params in [
        ('summary-alias', {'reasoning': {'generate_summary': 'auto'}}),
        ('plain-text', {'text': {'format': {'type': 'text'}}}),
        ('json-schema', {'text': {'format': {'type': 'json_schema', 'name': 'result',
                                        'schema': {'type': 'object'}, 'strict': True}}}),
    ]
] + [
    pytest.param(params, False, False, id=name) for name, params in [
        ('invalid-context', {'reasoning': {'context': 'typo'}}),
        ('context-type', {'reasoning': {'context': 1}}),
        ('reasoning-mode', {'reasoning': {'mode': 'pro'}}),
        ('text-schema', {'text': {'format': {'type': 'text', 'schema': {}}}}),
        ('json-object', {'text': {'format': {'type': 'json_object'}}}),
        ('max-output-tokens-zero', {'max_output_tokens': 0}),
        ('max-output-tokens-negative', {'max_output_tokens': -1}),
        ('max-output-tokens-fraction', {'max_output_tokens': 1.5}),
        ('max-output-tokens-type', {'max_output_tokens': '32'}),
        ('max-output-tokens-bool', {'max_output_tokens': True}),
        ('max-output-tokens-overflow', {'max_output_tokens': 2**64}),
        ('background', {'background': True}),
        ('store', {'store': True}),
        ('safety-identifier-type', {'safety_identifier': 42}),
        ('safety-identifier-length', {'safety_identifier': 'a' * 65}),
    ]
]


@pytest.mark.parametrize('case_name,params,stream,accepted', [
    pytest.param(case.id, *case.values, id=case.id) for case in CASES
])
def test_response_parameters(client, model, case_name, params, stream, accepted):
    if not accepted:
        with pytest.raises(APIStatusError) as error:
            client.responses.create(model=model, input=case_name, **params)
        assert error.value.status_code in (400, 422)
    else:
        response = client.responses.create(model=model, input=case_name, stream=stream, **params)
        if stream:
            with response:
                assert list(response)[-1].response.status == 'completed'
        else:
            assert response.status == 'completed'
