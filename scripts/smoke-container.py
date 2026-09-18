#!/usr/bin/env python3
"""Check the runtime image using synthetic credentials; never call the model API."""
import base64
from datetime import datetime, timezone
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

image = sys.argv[1]

def docker(*args):
    return subprocess.check_output(['docker', *args], text=True).strip()

config = json.loads(docker('image', 'inspect', image))[0]
assert config['Config']['User'] == '65532:65532'
assert 'codex-api-gateway' in docker('run', '--rm', '--network', 'none', image, '--version')

def encode(value):
    return base64.urlsafe_b64encode(json.dumps(value).encode()).decode().rstrip('=')

with tempfile.TemporaryDirectory() as home:
    path = Path(home)
    path.chmod(0o755)
    claims = {'email': 'container-test@example.invalid', 'https://api.openai.com/auth': {
        'chatgpt_account_id': 'container-test', 'chatgpt_user_id': 'container-test', 'chatgpt_plan_type': 'plus'}}
    token = encode({'alg': 'none'}) + '.' + encode(claims) + '.test'
    (path / 'auth.json').write_text(json.dumps({
        'auth_mode': 'chatgpt', 'OPENAI_API_KEY': None,
        'tokens': {'id_token': token, 'access_token': 'synthetic-access',
                   'refresh_token': 'synthetic-refresh', 'account_id': 'container-test'},
        'last_refresh': datetime.now(timezone.utc).isoformat(),
    }))
    (path / 'config.toml').write_text('cli_auth_credentials_store = "file"\n')
    container = docker('run', '-d', '--read-only', '--tmpfs', '/tmp',
                       '-p', '127.0.0.1::8080', '-v', f'{home}:/data:ro',
                       '-e', 'CODEX_GATEWAY_API_KEY=container-test-key', image)
    try:
        port = docker('port', container, '8080/tcp').rsplit(':', 1)[1]
        base = f'http://127.0.0.1:{port}'
        for attempt in range(30):
            try:
                health = json.load(urllib.request.urlopen(base + '/healthz', timeout=2))
                break
            except (urllib.error.URLError, TimeoutError, ConnectionError):
                if docker('inspect', '-f', '{{.State.Running}}', container) != 'true':
                    raise RuntimeError(docker('logs', container))
                time.sleep(1)
        else:
            raise RuntimeError('Container health timeout')
        lock = json.loads(Path('baseline/codex-release.lock.json').read_text())
        assert health['codex_release'] == lock['release'] and health['codex_revision'] == lock['commit']
        try:
            urllib.request.urlopen(base + '/v1/models', timeout=2)
            raise AssertionError('Unauthenticated request accepted')
        except urllib.error.HTTPError as error:
            assert error.code == 401
        request = urllib.request.Request(base + '/v1/models', headers={'Authorization': 'Bearer container-test-key'})
        assert json.load(urllib.request.urlopen(request, timeout=2))['data']
        for endpoint in ('/v1/images/generations', '/v1/images/edits',
                         '/backend-api/codex/images/generations', '/codex/images/edits',
                         '/codex/alpha/search', '/backend-api/codex/alpha/search'):
            request = urllib.request.Request(base + endpoint, data=b'{"prompt":""}', headers={
                'Authorization': 'Bearer container-test-key', 'Content-Type': 'application/json'})
            try:
                urllib.request.urlopen(request, timeout=2)
                raise AssertionError('Invalid image request accepted')
            except urllib.error.HTTPError as error:
                assert error.code == 400
        print(f'Container startup, health, authentication, model catalog and standalone tool routes passed; {config["Size"] / 1024**2:.1f} MiB')
    finally:
        docker('rm', '-f', container)
