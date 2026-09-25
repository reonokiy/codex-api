#!/usr/bin/env python3
"""Remove one adapter behavior at a time in an isolated copy; require test regressions."""
import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent


@dataclass(frozen=True)
class Case:
    name: str
    source: str
    pattern: str
    replacement: str
    test: str
    contract: str


CASES = [
    Case('native_error_bytes', 'transport.rs',
         r'\*self\s*\.failed\s*\.lock\(\).*?\?\s*=\s*Some\(response\);',
         '',
         'native_cases::native_errors_redirects_and_auth_modes_remain_transparent',
         'Native binary error bodies and redirect metadata must survive original typed transport errors.'),
    Case('native_authentication', 'server.rs',
         r'pub\(crate\) fn authorize\(.*?\n\}',
         'pub(crate) fn authorize(_gateway: &Gateway, _headers: &HeaderMap) -> Result<(), GatewayError> { Ok(()) }',
         'native_cases::native_errors_redirects_and_auth_modes_remain_transparent',
         'Gateway authentication is separate from upstream credentials across native protocols.'),
    Case('sdk_headers', 'server.rs',
         r'pub\(crate\) fn forward_request_headers\(.*?\n\}',
         'pub(crate) fn forward_request_headers(headers: &HeaderMap) -> HeaderMap { headers.clone() }',
         'images_match_original_client_and_preserve_complete_responses',
         'SDK headers must not change original Codex upstream headers.'),
    Case('image_defaults', 'images.rs',
         r'\.or\(\(!native\)\.then_some\(ImageQuality::Auto\)\)', '',
         'images_accept_sdk_multipart_without_touching_the_filesystem',
         'Omitted SDK quality uses the original image-extension default.'),
    Case('multipart', 'images.rs', r'multipart_input\(request\)\.await\?',
         'return Err(GatewayError::invalid("multipart adapter removed"))',
         'images_accept_sdk_multipart_without_touching_the_filesystem',
         'SDK binary uploads must reach ImagesClient as native image data URLs.'),
    Case('authentication', 'standalone.rs', r'    authorize\(&gateway, &headers\)\?;', '',
         'images_reject_invalid_requests_and_unauthorized_clients_before_upstream',
         'Unauthenticated image requests must never reach the upstream.'),
    Case('concurrency', 'standalone.rs', r'    let _permit = gateway\n.*?        \}\)\?;\n', '',
         'standalone_tools_share_concurrency_slots',
         'Images and search must share the gateway generation limit.'),
    Case('output_assembly', 'output.rs',
         r'    pub fn observe\(&mut self, event: &mut Value\) -> bool \{.*\n    \}',
         '    pub fn observe(&mut self, _event: &mut Value) -> bool { false }',
         'web_search_preserves_sources_citations_and_stream_events',
         'Public terminal output must include separately emitted search items and citations.'),
    Case('strict_fields', 'standalone.rs', r'    if !ignored.is_empty\(\) \{.*?\n    \}', '',
         'standalone_search_rejects_unknown_fields_and_preserves_failures',
         'Unsupported nested search fields must not silently disappear.'),
    Case('raw_response_capture', 'transport.rs',
         r'        let _ = self\n            \.response\n            \.set\(\(response.headers.clone\(\), response.body.clone\(\)\)\);', '',
         'images_match_original_client_and_preserve_complete_responses',
         'Original typed parsing alone cannot return the full standalone response to callers.'),
]


def test_name(case):
    return case.test if '::' in case.test else f'tool_cases::{case.test}'


def run_test(work, selector, log, env, exact=False):
    command = ['cargo', 'test', '--locked', '--test', 'gateway', selector]
    if exact:
        command.extend(['--', '--exact'])
    started = time.monotonic()
    with log.open('w') as output:
        result = subprocess.run(command, cwd=work, env=env, stdout=output,
                                stderr=subprocess.STDOUT, timeout=900)
    return result.returncode, round(time.monotonic() - started, 2), log.read_text()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', action='append', choices=[case.name for case in CASES])
    args = parser.parse_args()
    selected = [case for case in CASES if not args.case or case.name in args.case]
    for case in selected:
        source = (ROOT / 'src' / case.source).read_text()
        count = len(re.findall(case.pattern, source, flags=re.DOTALL))
        if count != 1:
            raise RuntimeError(f'{case.name}: expected one mutation site, found {count}')
    logs = ROOT / 'artifacts/ablation'
    logs.mkdir(parents=True, exist_ok=True)
    cache = ROOT / '.cache'
    cache.mkdir(exist_ok=True)
    env = os.environ.copy()
    env['CARGO_TARGET_DIR'] = str(Path(env.get('CARGO_TARGET_DIR', ROOT / 'target')).resolve())
    sources = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
               for p in (ROOT / 'src').glob('*.rs')}
    report = {'source_sha256': sources, 'cases': [], 'baseline': False, 'restored': False}
    try:
        with tempfile.TemporaryDirectory(prefix='ablation-', dir=cache) as temporary:
            work = Path(temporary)
            for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'clippy.toml'):
                shutil.copy2(ROOT / name, work / name)
            for name in ('src', 'tests', '.cargo', 'baseline', 'xtask'):
                shutil.copytree(ROOT / name, work / name)
            # Dependencies stay unchanged. Mutants can only edit copied gateway source.
            (work / 'vendor').symlink_to(ROOT / 'vendor', target_is_directory=True)
            code, seconds, output = run_test(work, '', logs / 'baseline.log', env)
            report['baseline'] = code == 0 and all(
                f'test {test_name(case)} ... ok' in output for case in selected)
            report['baseline_seconds'] = seconds
            if not report['baseline']:
                raise RuntimeError(f'Baseline failed or a target test was not executed: {logs / "baseline.log"}')
            print('Baseline passed', flush=True)
            for case in selected:
                path = work / 'src' / case.source
                original = path.read_text()
                mutant, count = re.subn(case.pattern, lambda _: case.replacement,
                                        original, flags=re.DOTALL)
                if count != 1:
                    raise RuntimeError(f'{case.name}: expected one mutation site, found {count}')
                try:
                    path.write_text(mutant)
                    test = test_name(case)
                    log = logs / f'{case.name}.log'
                    code, seconds, output = run_test(work, test, log, env, exact=True)
                    # Compilation errors, crashes and missing tests do not count as evidence.
                    killed = code == 101 and f'test {test} ... FAILED' in output and 'error: test failed' in output
                    report['cases'].append({'name': case.name, 'contract': case.contract,
                                            'test': test, 'regression_detected': killed,
                                            'seconds': seconds, 'log': str(log.relative_to(ROOT))})
                    print(f'{case.name}: {"regression detected" if killed else "INVALID / SURVIVED"}', flush=True)
                finally:
                    path.write_text(original)
            code, _, output = run_test(work, '', logs / 'restored.log', env)
            report['restored'] = code == 0 and all(
                f'test {test_name(case)} ... ok' in output for case in selected)
            print(f'Restored baseline: {"passed" if report["restored"] else "FAILED"}', flush=True)
    finally:
        (logs / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        # Cargo can reuse the copied package's embedded CARGO_MANIFEST_DIR in a shared target.
        # Clear only gateway outputs; preserve compiled dependencies and leave source untouched.
        with (logs / 'cleanup.log').open('w') as output:
            subprocess.run(['cargo', 'clean', '--manifest-path', str(ROOT / 'Cargo.toml'),
                            '-p', 'codex-api-gateway'], env=env, stdout=output,
                           stderr=subprocess.STDOUT, check=True)
    if not report['restored'] or not all(case['regression_detected'] for case in report['cases']):
        raise SystemExit('Ablation did not establish all selected contracts; inspect artifacts/ablation/')
    print(f'{len(selected)}/{len(selected)} ablations detected; source was never modified.', flush=True)


if __name__ == '__main__':
    main()
