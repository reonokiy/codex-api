"""Shared pytest options and explicit test-support plugins."""
import os
from pathlib import Path

import pytest

pytest_plugins = [
    "outcome_report",
    "local_fixtures",
    "live_fixtures",
]


def pytest_addoption(parser):
    group = parser.getgroup('gateway SDK')
    group.addoption('--sdk-report', type=Path, help='Write an outcome-only JSON report')
    group.addoption('--live-sdk', action='store_true', help='Opt in to real subscription usage')
    group.addoption('--gateway-url', default=os.environ.get('CODEX_GATEWAY_LIVE_URL'))
    group.addoption('--sdk-model', default='gpt-6-sol')
    group.addoption('--sdk-timeout', type=int, default=330)


def pytest_collection_modifyitems(config, items):
    if any(item.get_closest_marker('live') for item in items) and not config.getoption('--live-sdk'):
        raise pytest.UsageError('Live SDK tests require --live-sdk; normally use cargo e2e real_subscription_openai_sdk')
