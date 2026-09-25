"""Real gateway clients with function-scoped SDK lifetimes."""
import os
from pathlib import Path

from openai import OpenAI, AsyncOpenAI
import pytest
import pytest_asyncio


@pytest.fixture
def live_options(pytestconfig):
    url = pytestconfig.getoption('--gateway-url')
    key = os.environ.get('CODEX_GATEWAY_API_KEY')
    if not url or not key:
        raise pytest.UsageError('Live SDK tests require a gateway URL and CODEX_GATEWAY_API_KEY')
    return dict(base_url=url.rstrip('/') + '/v1', api_key=key, max_retries=0,
                timeout=pytestconfig.getoption('--sdk-timeout'))


@pytest.fixture
def live_client(live_options):
    with OpenAI(**live_options) as client:
        yield client


@pytest_asyncio.fixture
async def live_async_client(live_options):
    async with AsyncOpenAI(**live_options) as client:
        yield client


@pytest.fixture
def live_model(pytestconfig):
    return pytestconfig.getoption('--sdk-model')


@pytest.fixture
def sdk_artifacts(pytestconfig):
    report = pytestconfig.getoption('--sdk-report') or Path('artifacts/e2e/sdk/live-results.json')
    report.parent.mkdir(parents=True, exist_ok=True)
    return report.parent
