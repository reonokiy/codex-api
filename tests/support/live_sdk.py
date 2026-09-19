#!/usr/bin/env python3
"""Official OpenAI SDK checks, launched by `cargo e2e` against its test gateway.

Never substitutes a mock, retries generations, or saves account response bodies.
The Rust live suite covers native Responses/WebSocket separately.
"""
import argparse
import base64
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import struct
import time
import urllib.error
import urllib.request
import urllib.parse
import uuid
import zlib


def require(condition, message):
    if not condition:
        raise AssertionError(message)


class Suite:
    def __init__(self, args):
        self.args = args
        self.key = os.environ.get('CODEX_GATEWAY_API_KEY', '')
        self.results = []
        self.started = datetime.now(timezone.utc).isoformat()
        self.finished = False
        self.search_session = None
        self.search_reference = None

    def save(self):
        self.args.report.parent.mkdir(parents=True, exist_ok=True)
        self.args.report.write_text(json.dumps({
            'started_at': self.started, 'synthetic': False,
            'model': self.args.model, 'results': self.results,
            'finished': self.finished,
            'complete': self.finished and all(r['status'] == 'passed' for r in self.results),
        }, indent=2) + '\n')

    def record(self, name, status, **details):
        self.results.append(dict(name=name, status=status, **details))
        self.save()
        print(f'{name}: {status}', flush=True)

    def run(self, name, fn):
        start = time.monotonic()
        try:
            details = fn() or {}
            self.record(name, 'passed', seconds=round(time.monotonic()-start, 2), **details)
        except urllib.error.HTTPError as error:
            # Do not persist upstream bodies, which can include private account data.
            self.record(name, 'failed', http_status=error.code,
                        seconds=round(time.monotonic()-start, 2))
        except Exception as error:
            self.record(name, 'failed', error_type=type(error).__name__,
                        http_status=getattr(error, 'status_code', None),
                        reason=str(error) if isinstance(error, AssertionError) else 'transport or decoding failure',
                        seconds=round(time.monotonic()-start, 2))

    def request(self, path, body=None, headers=None, auth=True):
        h = {'Authorization': f'Bearer {self.key}'} if auth and self.key else {}
        if isinstance(body, dict):
            body = json.dumps(body).encode()
            h['Content-Type'] = 'application/json'
        h.update(headers or {})
        return urllib.request.urlopen(urllib.request.Request(
            self.args.url.rstrip('/') + path, data=body, headers=h), timeout=self.args.timeout)

    def json(self, path, body=None):
        with self.request(path, body) as response:
            return json.load(response)

    def prompt(self, **extra):
        return dict(model=self.args.model, input='Reply exactly OK.',
                    instructions='Follow the user instruction precisely.',
                    reasoning={'effort': 'low'}, **extra)

    def health(self):
        v = self.json('/healthz')
        require(v.get('status') == 'ok', 'health must be ok')
        return {'codex_release': v.get('codex_release'), 'codex_revision': v.get('codex_revision')}

    def auth(self):
        for headers in ({}, {'Authorization': 'Bearer intentionally-wrong-e2e-key'}):
            try:
                with self.request('/v1/models', headers=headers, auth=False):
                    raise AssertionError('gateway accepted missing or incorrect key')
            except urllib.error.HTTPError as error:
                require(error.code == 401, 'expected HTTP 401')

    def models(self, native=False):
        v = self.json('/codex/models' if native else '/v1/models')
        items = v.get('models' if native else 'data', [])
        require(any(i.get('slug' if native else 'id') == self.args.model for i in items),
                'test model absent from catalog')
        return {'count': len(items), 'upstream': native}

    @staticmethod
    def text(v):
        return ''.join(c.get('text', '') for i in v.get('output', [])
                       for c in i.get('content', []) if c.get('type') == 'output_text')

    def responses(self):
        v = self.json('/v1/responses', self.prompt())
        require(v.get('status') == 'completed' and self.text(v).strip() == 'OK',
                'expected completed response with exact OK text')


    def compact(self):
        v = self.json('/v1/responses/compact', self.prompt())
        require(v.get('object') == 'response.compaction' and
                any(i.get('type') == 'compaction' and i.get('encrypted_content') for i in v.get('output', [])),
                'missing usable compaction output')

    def function(self):
        body = self.prompt(tools=[{'type': 'function', 'name': 'e2e_value',
                                  'parameters': {'type': 'object', 'properties': {}, 'additionalProperties': False}}])
        body['input'] = [{'role': 'user', 'content': 'Call e2e_value once, then reply with its exact result.'}]
        v = self.json('/v1/responses', body)
        calls = [i for i in v.get('output', []) if i.get('type') == 'function_call']
        require(len(calls) == 1 and calls[0].get('name') == 'e2e_value', 'expected function call')
        require(isinstance(json.loads(calls[0]['arguments']), dict), 'invalid function arguments')
        marker = 'e2e-' + uuid.uuid4().hex
        body['input'] += v['output'] + [{'type': 'function_call_output', 'call_id': calls[0]['call_id'], 'output': marker}]
        v = self.json('/v1/responses', body)
        require(v.get('status') == 'completed' and marker in self.text(v), 'tool result continuation failed')

    def search(self):
        body = self.prompt(tools=[{'type': 'web_search'}], include=['web_search_call.action.sources'])
        body['input'] = 'Use web search to find the official OpenAI Codex page. Cite its URL. You must search.'
        v = self.json('/v1/responses', body)
        require(v.get('status') == 'completed' and self.text(v), 'search response incomplete')
        require(any(i.get('type') == 'web_search_call' for i in v.get('output', [])), 'no web search executed')
        sources = [s for i in v.get('output', []) if i.get('type') == 'web_search_call'
                   for s in (i.get('action') or {}).get('sources', []) if s.get('url', '').startswith('https://')]
        annotations = [a for i in v.get('output', []) for c in i.get('content', [])
                       for a in c.get('annotations', []) if a.get('type') == 'url_citation']
        require(sources or annotations, 'no search sources or citations returned')
        return {'source_count': len(sources), 'citation_count': len(annotations)}

    def standalone(self):
        self.search_session = 'e2e-' + uuid.uuid4().hex
        v = self.json('/codex/alpha/search', {'id': self.search_session, 'model': self.args.model,
                      'commands': {'search_query': [{'q': 'OpenAI Codex', 'domains': ['openai.com']}],
                                   'response_length': 'short'}})
        results = [r for r in v.get('results', []) if r.get('type') == 'text_result'
                   and r.get('url', '').startswith('https://') and r.get('ref_id')]
        require(v.get('output') and results, 'missing standalone search text or structured results')
        self.search_reference = results[0]['ref_id']
        return {'result_count': len(results), 'first_url': results[0]['url']}

    def standalone_open(self):
        if not self.search_reference:
            self.standalone()
        v = self.json('/backend-api/codex/alpha/search', {'id': self.search_session, 'model': self.args.model,
                      'commands': {'open': [{'ref_id': self.search_reference}], 'response_length': 'short'}})
        require(bool(v.get('output')), 'empty opened page')
        require(bool(v.get('results')), 'missing structured opened-page result')
        return {'output_characters': len(v['output']), 'result_count': len(v['results'])}

    @staticmethod
    def png():
        def chunk(kind, data):
            return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind+data))
        return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', 32, 32, 8, 2, 0, 0, 0))
                + chunk(b'IDAT', zlib.compress((b'\0' + b'\xff\0\0'*32)*32)) + chunk(b'IEND', b''))



    def save_image(self, v, edit):
        require(bool(v.get('data')), 'no image returned')
        data = base64.b64decode(v['data'][0]['b64_json'], validate=True)
        require(data.startswith(b'\x89PNG\r\n\x1a\n') and b'IEND' in data[-12:], 'invalid PNG output')
        width, height = struct.unpack('!II', data[16:24])
        require(width > 0 and height > 0, 'invalid image dimensions')
        path = self.args.report.parent / ('edit.png' if edit else 'generation.png')
        path.write_bytes(data)
        return {'bytes': len(data), 'width': width, 'height': height, 'sha256': hashlib.sha256(data).hexdigest()}



class SDKSuite(Suite):
    def __init__(self, args):
        super().__init__(args)
        import openai
        self.client = openai.OpenAI(base_url=args.url.rstrip('/') + '/v1',
                                   api_key=self.key or 'unused-local-key',
                                   timeout=args.timeout, max_retries=0)
        self.record('sdk_version', 'passed', version=openai.__version__)

    def json(self, path, body=None):
        if path in ('/codex/alpha/search', '/backend-api/codex/alpha/search'):
            # Native search has no typed OpenAI SDK resource. Exercise its
            # supported custom-request API with the SDK's real default headers.
            return self.client.post(self.args.url.rstrip('/') + path, body=body, cast_to=dict)
        if path == '/v1/models':
            return self.client.models.list().model_dump()
        if path == '/v1/responses':
            return self.client.responses.create(**body).model_dump(exclude_none=True)
        if path == '/v1/responses/compact':
            # Compact has a distinct public schema without reasoning.
            return self.client.responses.compact(**{k: v for k, v in body.items() if k != 'reasoning'}).model_dump()
        return super().json(path, body)

    def sse(self):
        with self.client.responses.create(**self.prompt(stream=True)) as stream:
            events = list(stream)
        require(not any(e.type in ('error', 'response.failed', 'response.incomplete') for e in events),
                'SDK stream contained an error')
        require(sum(e.type == 'response.completed' for e in events) == 1, 'SDK missing completion')
        require(''.join(e.delta for e in events if e.type == 'response.output_text.delta').strip() == 'OK',
                'SDK streamed text did not equal OK')
        return {'event_count': len(events)}

    def image(self, edit=False):
        fields = {'model': 'gpt-image-2', 'prompt': 'A plain blue square on a white background.', 'quality': 'low'}
        v = (self.client.images.edit(image=('e2e.png', self.png(), 'image/png'), **fields) if edit
             else self.client.images.generate(**fields))
        return self.save_image(v.model_dump(), edit)

    def upload(self):
        data = self.png()
        v = self.client.files.create(file=('codex-sdk-e2e.png', data, 'image/png'), purpose='user_data')
        require(v.object == 'file' and v.id and v.bytes == len(data) and v.status == 'processed',
                'SDK upload did not finalize')
        return {'file_id': v.id, 'bytes': v.bytes, 'cleanup': 'subscription adapter has no delete endpoint'}

    def realtime(self):
        import asyncio
        from aiortc import RTCPeerConnection, RTCConfiguration, RTCSessionDescription

        async def call():
            peer = RTCPeerConnection(RTCConfiguration(iceServers=[]))
            channel = peer.createDataChannel('oai-events')
            received_audio = asyncio.Event()
            completed = asyncio.Event()
            errors = []
            tasks = []

            @peer.on('track')
            def track_received(track):
                async def receive():
                    await track.recv()
                    received_audio.set()
                tasks.append(asyncio.create_task(receive()))

            @channel.on('open')
            def opened():
                channel.send(json.dumps({'type': 'response.create', 'response': {
                    'instructions': 'Say hello briefly.', 'output_modalities': ['audio']}}))

            @channel.on('message')
            def message(raw):
                event = json.loads(raw)
                if event.get('type') == 'error':
                    errors.append('Realtime error event')
                    completed.set()
                if event.get('type') == 'response.done':
                    if event.get('response', {}).get('status') != 'completed':
                        errors.append('Realtime response not completed')
                    completed.set()

            try:
                peer.addTransceiver('audio', direction='recvonly')
                await peer.setLocalDescription(await peer.createOffer())
                result = await asyncio.to_thread(self.client.realtime.calls.create,
                    sdp=peer.localDescription.sdp,
                    session={'type': 'realtime', 'model': 'gpt-realtime'}, timeout=60)
                answer = result.read().decode()
                require(answer.startswith('v=0'), 'expected SDP answer')
                require(result.response.headers.get('location'), 'missing call Location')
                await peer.setRemoteDescription(RTCSessionDescription(sdp=answer, type='answer'))
                await asyncio.wait_for(completed.wait(), 45)
                require(not errors, 'Realtime session returned an error')
                await asyncio.wait_for(received_audio.wait(), 10)
                self.record('realtime_webrtc_media', 'passed', sdp_answer=True,
                            response_completed=True, audio_frame_received=True)
                from openai import AsyncOpenAI
                call_id = urllib.parse.urlsplit(result.response.headers['location']).path.rsplit('/', 1)[-1]
                async with AsyncOpenAI(base_url=self.args.url.rstrip('/') + '/v1', api_key=self.key,
                                       max_retries=0, timeout=30) as client:
                    async with client.realtime.connect(call_id=call_id, max_retries=0,
                            websocket_connection_options={'open_timeout': 20, 'close_timeout': 5}) as connection:
                        # This joins an existing session: buffered media events may
                        # precede the update; session.created is not required here.
                        instructions = 'Reply briefly. E2E session marker: ' + uuid.uuid4().hex
                        await connection.session.update(session={'type': 'realtime', 'instructions': instructions})
                        async def updated():
                            while True:
                                event = await connection.recv()
                                require(event.type != 'error', 'sideband returned error')
                                if event.type == 'session.updated':
                                    require(event.session.instructions == instructions, 'sideband update did not roundtrip')
                                    return
                        await asyncio.wait_for(updated(), 15)
                return {'sdp_answer': True, 'response_completed': True, 'audio_frame_received': True,
                        'sdk_sideband_session_update': True}
            finally:
                for task in tasks:
                    task.cancel()
                await asyncio.gather(*tasks, return_exceptions=True)
                await peer.close()
        return asyncio.run(call())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url', default=os.environ.get('CODEX_GATEWAY_LIVE_URL', 'http://127.0.0.1:8080'))
    parser.add_argument('--model', default='gpt-5.5')
    parser.add_argument('--timeout', type=int, default=330)
    parser.add_argument('--report', type=Path, default=Path('artifacts/e2e/live-results.json'))
    parser.add_argument('--only', help='Comma-separated check names for targeted diagnostics')
    args = parser.parse_args()
    suite = SDKSuite(args)
    checks = [('health', suite.health), ('gateway_auth', suite.auth), ('public_models', suite.models),
              ('native_models', lambda: suite.models(True)), ('responses_http', suite.responses),
              ('responses_sse', suite.sse), ('compaction', suite.compact),
              ('function_roundtrip', suite.function), ('hosted_search', suite.search),
              ('standalone_search', suite.standalone), ('standalone_open', suite.standalone_open),
              ('file_upload', suite.upload),
              ('image_generation', suite.image), ('image_edit_multipart', lambda: suite.image(True))]
    checks.append(('realtime_webrtc', suite.realtime))
    if args.only:
        selected = set(args.only.split(','))
        require(selected <= {name for name, _ in checks}, 'unknown --only check name')
        checks = [(name, fn) for name, fn in checks if name in selected]
    for name, fn in checks:
        suite.run(name, fn)
    suite.record('realtime_direct_and_platform', 'not_tested',
                 reason='Requires independent Platform credentials and a separate session test; subscription is not sufficient')
    suite.record('native_account_mutations', 'not_tested',
                 reason='Task creation, notes writes, plugin installation, sharing, email and remote enrollment excluded')
    suite.client.close()
    suite.finished = True
    suite.save()
    return 1 if any(r['status'] == 'failed' for r in suite.results) else 0


if __name__ == '__main__':
    raise SystemExit(main())
