#!/usr/bin/env python3
"""Audit pinned dependencies and gateway patches against the cached official release."""
import difflib
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parent.parent
lock = json.loads((ROOT / 'baseline/codex-release.lock.json').read_text())
source = ROOT / '.cache/codex-baseline' / lock['tag'] / 'source'
if not source.is_dir():
    raise SystemExit('Run scripts/fetch-codex-baseline.py first')
assert subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip() == lock['commit']
assert not subprocess.check_output(['git', '-C', str(source), 'status', '--porcelain'], text=True).strip()
packages = tomllib.loads((ROOT / 'Cargo.lock').read_text())['package']
upstream = tomllib.loads((source / 'codex-rs/Cargo.lock').read_text())['package']
expected_source = f"git+{lock['repository']}?rev={lock['commit']}#{lock['commit']}"
codex = [p for p in packages if 'github.com/openai/codex' in p.get('source', '')]
assert codex and all(p['source'] == expected_source and p['version'] == lock['release'] for p in codex)
upstream_versions = {}
for package in upstream:
    if package.get('source', '').startswith('registry+'):
        upstream_versions.setdefault(package['name'], set()).add(package['version'])
shared = [p for p in packages if p.get('source', '').startswith('registry+') and p['name'] in upstream_versions]
assert all(p['version'] in upstream_versions[p['name']] for p in shared), 'Registry dependency differs from release lockfile'
assert tomllib.loads((ROOT / 'vendor/codex-api/Cargo.toml').read_text())['package']['version'] == lock['release']
assert tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel'] == tomllib.loads((source / 'codex-rs/rust-toolchain.toml').read_text())['toolchain']['channel']
original = source / 'codex-rs/codex-api'
vendor = ROOT / 'vendor/codex-api'
files = {p.relative_to(original) for p in (original / 'src').rglob('*.rs')}
assert files == {p.relative_to(vendor) for p in (vendor / 'src').rglob('*.rs')}
integration_tests = {p.relative_to(original) for p in (original / 'tests').rglob('*.rs')}
assert integration_tests == {p.relative_to(vendor) for p in (vendor / 'tests').rglob('*.rs')}
assert all((original / p).read_bytes() == (vendor / p).read_bytes() for p in integration_tests)
changed, diff = [], []
for relative in sorted(files):
    before, after = (original / relative).read_text(), (vendor / relative).read_text()
    if before != after:
        changed.append(str(relative))
        diff.extend(difflib.unified_diff(before.splitlines(True), after.splitlines(True), fromfile='a/' + str(relative), tofile='b/' + str(relative)))
assert changed == ['src/endpoint/responses.rs', 'src/endpoint/responses_websocket.rs']
patch = ''.join(diff).encode()
assert patch == (ROOT / 'vendor/raw-transport.patch').read_bytes(), 'Regenerate patch after source changes'
report = {
    'release': lock['release'], 'revision': lock['commit'],
    'rust_toolchain': tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel'],
    'codex_git_packages_verified': len(codex),
    'shared_registry_packages_verified': len(shared),
    'identical_source_files': len(files) - len(changed),
    'identical_integration_test_files': len(integration_tests),
    'changed_source_files': changed,
    'patch_sha256': hashlib.sha256(patch).hexdigest(),
    'sse_parser_unchanged': True,
}
output = ROOT / 'artifacts/protocol/source-audit.json'
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
