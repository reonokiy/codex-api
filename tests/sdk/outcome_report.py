"""Pytest outcome reporting and private live-error redaction."""
from datetime import datetime, timezone
import json
from pathlib import Path

import openai
import pytest

from sdk_contract import SUPPORTED_FIELDS, REJECTED_FIELDS


class OutcomeReport:
    """Record pytest outcomes without persisting SDK exception bodies or secrets."""
    def __init__(self, config):
        self.config = config
        self.path = config.getoption('--sdk-report')
        self.results = {}
        self.started = datetime.now(timezone.utc).isoformat()
        self.deselected = 0
        self.selected = 0

    def save(self, finished=False, exitstatus=None):
        if self.path is None:
            return
        rows = list(self.results.values())
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(json.dumps({
            'started_at': self.started, 'runner': 'pytest', 'sdk_version': openai.__version__,
            'synthetic': not self.config.getoption('--live-sdk'), 'finished': finished,
            'selected': self.selected, 'deselected': self.deselected,
            'selection': self.config.option.keyword or 'all',
            'full_suite': self.deselected == 0,
            'complete': finished and exitstatus == 0 and bool(rows)
                        and all(row['status'] == 'passed' for row in rows),
            'create_parameters': {'supported': sorted(SUPPORTED_FIELDS), 'rejected': REJECTED_FIELDS},
            'results': rows,
        }, indent=2) + '\n')

    def pytest_sessionstart(self, session):
        self.save()

    def pytest_deselected(self, items):
        self.deselected += len(items)

    def pytest_collection_finish(self, session):
        self.selected = len(session.items)
        self.save()

    def pytest_runtest_logreport(self, report):
        if report.when == 'call' or report.failed or report.skipped:
            row = self.results.setdefault(report.nodeid, {'case': report.nodeid})
            if row.get('status') != 'failed':
                row.update(status=report.outcome, phase=report.when, seconds=round(report.duration, 3))
            row.update(dict(report.user_properties))
            self.save()

    def pytest_sessionfinish(self, session, exitstatus):
        self.save(finished=True, exitstatus=int(exitstatus))


def pytest_configure(config):
    config.pluginmanager.register(OutcomeReport(config), 'sdk-outcome-report')


@pytest.hookimpl(wrapper=True)
def pytest_runtest_makereport(item, call):
    report = yield
    if report.failed and item.get_closest_marker('live') and call.excinfo:
        # Live SDK exceptions/traceback locals can contain account response bodies.
        location = call.excinfo.traceback[-1]
        failure_location = f'{Path(str(location.path)).name}:{location.lineno + 1}'
        report.user_properties.extend([('error_type', call.excinfo.typename), ('failure_location', failure_location)])
        report.longrepr = f'{item.nodeid}: {call.when} failed ({call.excinfo.typename}, {failure_location}); private details omitted'
    return report
