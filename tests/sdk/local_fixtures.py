"""Clients for Cargo-managed fake upstreams; no live credentials required."""
from contextlib import ExitStack
import json
import os

from openai import OpenAI
import pytest


def gateway_config():
    value = os.environ.get('CODEX_SDK_TEST_CONFIG')
    if not value:
        raise pytest.UsageError('Local fixtures are supplied by Cargo; run cargo test --locked --test gateway actual_openai_ -- --ignored')
    return json.loads(value)


def pytest_generate_tests(metafunc):
    if 'model' in metafunc.fixturenames:
        metafunc.parametrize('model', gateway_config()['models'], scope='module')


@pytest.fixture(scope='session')
def config():
    return gateway_config()


@pytest.fixture
def client_factory(config):
    with ExitStack() as stack:
        def create(kind='text', key='client-key'):
            return stack.enter_context(OpenAI(base_url=config[kind] + '/v1', api_key=key,
                                              timeout=5, max_retries=0))
        yield create


@pytest.fixture
def client(client_factory):
    return client_factory()


@pytest.fixture
def request_kwargs(config):
    def build(model=None, **extra):
        return dict(model=model or config['models'][0], input='Hello', **extra)
    return build
