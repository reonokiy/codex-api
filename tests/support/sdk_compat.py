"""Official SDK compatibility checks against controlled gateway/upstream fixtures.

Each case uses an actual SDK method, not an emulated HTTP client. No account
credentials or external services are used. Results are printed as JSON for Rust.
"""
import asyncio
import base64
import io
import inspect
import json
import sys

import openai
from openai import AsyncOpenAI, OpenAI
from pydantic import BaseModel

config = json.loads(sys.argv[1])
results = []


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def check(name, fn):
    try:
        fn()
        results.append({'case': name, 'status': 'passed'})
    except Exception as error:
        results.append({'case': name, 'status': 'failed', 'error': type(error).__name__, 'detail': str(error)})


def client(kind='text', key='client-key'):
    return OpenAI(base_url=config[kind] + '/v1', api_key=key, timeout=5, max_retries=0)


def kwargs(model=None, **extra):
    return dict(model=model or config['models'][0], input='Hello', **extra)


def completed(response):
    require(response.status == 'completed', 'expected completed response')
    require(response.output_text == '你好', 'output text lost')
    require(response.usage.total_tokens == 15, 'usage lost')
    require(response.model_extra['future_field'] == {'kept': True}, 'unknown upstream field lost')


def rejected(fn, error_type, statuses):
    try:
        fn()
    except error_type as error:
        require(error.status_code in statuses, 'incorrect HTTP status')
    else:
        raise AssertionError('expected SDK exception')


c = client()
check('models.list', lambda: require(set(config['models']) <= {m.id for m in c.models.list()}, 'models missing'))
check('models.raw_response', lambda: require(c.models.with_raw_response.list().status_code == 200, 'raw models failed'))
check('auth.invalid_key', lambda: rejected(lambda: client(key='incorrect').models.list(), openai.AuthenticationError, [401]))

for model in config['models']:
    prefix = model + ':'
    check(prefix+'responses.create', lambda: completed(c.responses.create(**kwargs(model))))
    for name, value in [
        ('message_string', [{'role': 'user', 'content': 'Hello'}]),
        ('message_parts', [{'role': 'user', 'content': [{'type': 'input_text', 'text': 'Hello'}]}]),
        ('assistant_history', [{'role': 'assistant', 'content': 'Earlier answer'}, {'role': 'user', 'content': 'Continue'}]),
        ('encrypted_reasoning', [{'type': 'reasoning', 'id': 'rs_fixture', 'summary': [], 'encrypted_content': 'opaque-fixture'}, {'role': 'user', 'content': 'Continue'}]),
        ('compaction_history', [{'type': 'compaction', 'id': 'cmp_fixture', 'encrypted_content': 'opaque-fixture'}, {'role': 'user', 'content': 'Continue'}]),
        ('function_result', [{'type':'function_call','call_id':'call_fixture','name':'lookup','arguments':'{}'}, {'type':'function_call_output','call_id':'call_fixture','output':'42'}]),
        ('custom_result', [{'type':'custom_tool_call','call_id':'call_custom','name':'patch','input':'abc'}, {'type':'custom_tool_call_output','call_id':'call_custom','output':'done'}]),
        ('input_image', [{'role':'user','content':[{'type':'input_text','text':'Describe'}, {'type':'input_image','image_url':'data:image/png;base64,'+config['png'],'detail':'auto'}]}]),
    ]:
        check(prefix+'input.'+name, lambda value=value: completed(c.responses.create(model=model,input=value)))
    for name, tool in [
        ('function', {'type':'function','name':'lookup','parameters':{'type':'object','properties':{},'additionalProperties':False},'strict':True}),
        ('custom', {'type':'custom','name':'patch','format':{'type':'grammar','syntax':'regex','definition':'.+'}}),
        ('namespace', {'type':'namespace','name':'tools','tools':[{'type':'function','name':'lookup','parameters':{'type':'object'}}]}),
        ('web_search', {'type':'web_search','filters':{'allowed_domains':['openai.com']},'search_context_size':'low'}),
    ]:
        check(prefix+'tools.'+name, lambda tool=tool: completed(c.responses.create(**kwargs(model, tools=[tool]))))
    check(prefix+'optional_nulls', lambda: completed(c.responses.create(**kwargs(model,reasoning=None,text=None,instructions=None,parallel_tool_calls=None,tool_choice=None,service_tier=None,include=None,prompt_cache_key=None))))
    check(prefix+'request_controls', lambda: completed(c.responses.create(**kwargs(model,instructions='Be brief',store=False,tool_choice='auto',parallel_tool_calls=False,prompt_cache_key='sdk-fixture',service_tier='auto',include=['reasoning.encrypted_content']))))

    def raw_response():
        raw = c.responses.with_raw_response.create(**kwargs(model))
        require(raw.status_code == 200, 'raw response status')
        completed(raw.parse())
    check(prefix+'responses.raw_response', raw_response)

    def streaming_response():
        with c.responses.with_streaming_response.create(**kwargs(model)) as raw:
            completed(raw.parse())
    check(prefix+'responses.streaming_wrapper', streaming_response)

    def sse():
        with c.responses.create(**kwargs(model, stream=True, reasoning={'context':'current_turn'})) as stream:
            events = list(stream)
        require(''.join(e.delta for e in events if e.type == 'response.output_text.delta') == '你好', 'SSE delta lost')
        completed(events[-1].response)
    check(prefix+'responses.sse', sse)

    def stream_helper():
        with c.responses.stream(**kwargs(model,reasoning={'context':'all_turns'})) as stream:
            completed(stream.get_final_response())
    check(prefix+'responses.stream_helper', stream_helper)

    def ws():
        with c.responses.connect(max_retries=0, websocket_connection_options={'open_timeout':3, 'close_timeout':1}) as connection:
            for previous in (None, 'resp_1'):
                params = kwargs(model, reasoning={'context':'current_turn'})
                if previous is not None:
                    params['previous_response_id'] = previous
                connection.response.create(**params)
                for event in connection:
                    require(event.type != 'error', 'WebSocket error')
                    if event.type == 'response.completed':
                        completed(event.response)
                        break
                else:
                    raise AssertionError('WebSocket ended before completion')
    check(prefix+'responses.websocket_continuation', ws)

for model in config['models']:
    for kind,tool,call_type,result_type in [
        ('function',{'type':'function','name':'lookup','parameters':{'type':'object'}},'function_call','function_call_output'),
        ('custom',{'type':'custom','name':'patch','format':{'type':'grammar','syntax':'regex','definition':'.+'}},'custom_tool_call','custom_tool_call_output'),
    ]:
        def roundtrip():
            with client(kind) as tc:
                history=[{'role':'user','content':'Use the tool'}]
                first=tc.responses.create(model=model,input=history,tools=[tool])
                call=next(item for item in first.output if item.type==call_type)
                history.extend(item.model_dump(exclude_none=True) for item in first.output)
                history.append({'type':result_type,'call_id':call.call_id,'output':'42'})
                completed(tc.responses.create(model=model,input=history,tools=[tool]))
        check(model+':'+kind+'.roundtrip',roundtrip)

class Result(BaseModel):
    ok: bool

s = client('structured')
check('responses.parse', lambda: require(s.responses.parse(**kwargs(), text_format=Result).output_parsed == Result(ok=True), 'parsed output mismatch'))
def parsed_stream():
    with s.responses.stream(**kwargs(), text_format=Result) as stream:
        require(stream.get_final_response().output_parsed == Result(ok=True), 'parsed stream mismatch')
check('responses.stream_parse', parsed_stream)

compact_client = client('compact')
def compact():
    response = compact_client.responses.compact(**kwargs(), extra_body={'reasoning':{'context':'all_turns'}})
    require(response.object == 'response.compaction', 'wrong compaction object')
    require(response.output[0].encrypted_content == 'opaque-fixture', 'compaction lost')
check('responses.compact', compact)

f = client('files')
check('files.create', lambda: require(f.files.create(file=('fixture.txt', b'fixture data', 'text/plain'),purpose='user_data').id == 'file_test', 'file upload failed'))
check('images.generate', lambda: require(c.images.generate(model='gpt-image-2',prompt='A square').data[0].b64_json == config['png'], 'image data lost'))
def edit():
    image=io.BytesIO(base64.b64decode(config['png']));image.name='fixture.png'
    require(c.images.edit(model='gpt-image-2',prompt='Blue',image=image).data[0].b64_json == config['png'], 'image edit failed')
check('images.edit', edit)

for name, params in [('store',{'store':True}), ('unknown_field',{'future_option':True}), ('unknown_reasoning',{'reasoning':{'future_option':True}}), ('max_output_tokens',{'max_output_tokens':32}), ('previous_response_id_http',{'previous_response_id':'resp_1'}), ('background',{'background':True}), ('tool_choice',{'tool_choice':'required'}), ('json_object',{'text':{'format':{'type':'json_object'}}})]:
    check('unsupported.'+name, lambda params=params: rejected(lambda:c.responses.create(**kwargs(),extra_body=params),openai.APIStatusError,[400,422]))
for name, fn in [('responses.retrieve',lambda:c.responses.retrieve('resp_missing')),('responses.delete',lambda:c.responses.delete('resp_missing')),('models.retrieve',lambda:c.models.retrieve('missing')),('chat.completions',lambda:c.chat.completions.create(model=config['models'][0],messages=[{'role':'user','content':'Hi'}]))]:
    check('unsupported.'+name, lambda fn=fn: rejected(fn,openai.NotFoundError,[404]))
check('unsupported.files.list', lambda: rejected(lambda:c.files.list(),openai.APIStatusError,[405]))
# Every public SDK create parameter is classified. New SDK parameters cannot
# silently become untested when the dependency is upgraded.
supported_fields = {'model','input','instructions','stream','store','tools','tool_choice',
                    'parallel_tool_calls','reasoning','text','service_tier','prompt_cache_key','include'}
transport_fields = {'extra_headers','extra_query','extra_body','timeout'}
sdk_fields = set(inspect.signature(c.responses.create).parameters) - transport_fields
for field in sorted(sdk_fields - supported_fields):
    check('unsupported.create_parameter.'+field, lambda field=field: rejected(
        lambda:c.responses.create(**kwargs(),extra_body={field:None}),openai.APIStatusError,[400,422]))
check('responses.transport_options', lambda: completed(c.responses.create(**kwargs(),timeout=3,extra_headers={'session-id':'sdk-session'})))

check('upstream.rate_limit',lambda:rejected(lambda:client('limited').responses.create(**kwargs()),openai.RateLimitError,[429]))
check('upstream.failed_status',lambda:require(client('failed').responses.create(**kwargs()).status=='failed','failure became success'))
check('upstream.incomplete_status',lambda:require(client('incomplete').responses.create(**kwargs()).status=='incomplete','incomplete became success'))

async def acheck(name, fn):
    try:
        await fn()
        results.append({'case':name,'status':'passed'})
    except Exception as error:
        results.append({'case':name,'status':'failed','error':type(error).__name__,'detail':str(error)})

async def async_checks():
    async with AsyncOpenAI(base_url=config['text']+'/v1',api_key='client-key',timeout=5,max_retries=0) as ac:
        async def models():
            require(set(config['models']) <= {m.id async for m in await ac.models.list()}, 'async models missing')
        await acheck('async.models',models)
        for model in config['models']:
            async def create():
                completed(await ac.responses.create(**kwargs(model,reasoning={'context':'all_turns'})))
            async def sse():
                async with await ac.responses.create(**kwargs(model,stream=True)) as stream:
                    events=[event async for event in stream]
                completed(events[-1].response)
            async def stream_helper():
                async with ac.responses.stream(**kwargs(model)) as stream:
                    completed(await stream.get_final_response())
            async def raw():
                response = await ac.responses.with_raw_response.create(**kwargs(model))
                completed(response.parse())
            async def wrapper():
                async with ac.responses.with_streaming_response.create(**kwargs(model)) as response:
                    completed(await response.parse())
            async def websocket():
                async with ac.responses.connect(max_retries=0,websocket_connection_options={'open_timeout':3,'close_timeout':1}) as connection:
                    await connection.response.create(**kwargs(model,reasoning={'context':'current_turn'}))
                    async for event in connection:
                        require(event.type!='error','async WebSocket error')
                        if event.type=='response.completed':
                            completed(event.response);break
                    else:raise AssertionError('async WebSocket ended early')
            for name,fn in [('create',create),('sse',sse),('stream_helper',stream_helper),('raw_response',raw),('streaming_wrapper',wrapper),('websocket',websocket)]:
                await acheck('async.'+model+':'+name,fn)
        async def invalid():
            try:await ac.responses.create(**kwargs(),extra_body={'reasoning':{'context':'invalid'}})
            except openai.UnprocessableEntityError:return
            raise AssertionError('async invalid enum was accepted')
        await acheck('async.invalid_context',invalid)
    async with AsyncOpenAI(base_url=config['structured']+'/v1',api_key='client-key',timeout=5,max_retries=0) as ac:
        async def parse():
            require((await ac.responses.parse(**kwargs(),text_format=Result)).output_parsed==Result(ok=True),'async parse mismatch')
        async def parse_stream():
            async with ac.responses.stream(**kwargs(),text_format=Result) as stream:
                require((await stream.get_final_response()).output_parsed==Result(ok=True),'async stream parse mismatch')
        await acheck('async.parse',parse)
        await acheck('async.stream_parse',parse_stream)
    async with AsyncOpenAI(base_url=config['files']+'/v1',api_key='client-key',timeout=5,max_retries=0) as ac:
        async def upload():
            require((await ac.files.create(file=('async.txt',b'async fixture','text/plain'),purpose='user_data')).id=='file_test','async upload failed')
        await acheck('async.files.create',upload)
asyncio.run(async_checks())
for x in [c,s,compact_client,f]:x.close()
print(json.dumps({'sdk_version':openai.__version__,'synthetic':True,'create_parameters':{'supported':sorted(supported_fields),'rejected':sorted(sdk_fields-supported_fields)},'results':results},ensure_ascii=False))
raise SystemExit(any(row['status']!='passed' for row in results))
