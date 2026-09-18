#!/usr/bin/env python3
"""Fetch the exact release baseline; never resolve 'latest' during reproduction."""
import hashlib
import json
import pathlib
import subprocess
import tarfile
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOCK = json.loads((ROOT / "baseline/codex-release.lock.json").read_text())
CACHE = ROOT / ".cache/codex-baseline" / LOCK["tag"]
CACHE.mkdir(parents=True, exist_ok=True)
source = CACHE / "source"
if not source.exists():
    subprocess.run(["git", "clone", "--depth=1", "--branch", LOCK["tag"], LOCK["repository"], str(source)], check=True)
head = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
if head != LOCK["commit"]:
    raise SystemExit(f"Baseline commit mismatch: {head}")
if subprocess.check_output(["git", "-C", str(source), "status", "--porcelain"], text=True).strip():
    raise SystemExit("Baseline source has local changes")
archive = CACHE / LOCK["binary"]["asset"]
if not archive.exists():
    temporary = archive.with_suffix(".download")
    urllib.request.urlretrieve(LOCK["binary"]["url"], temporary)
    temporary.replace(archive)
with archive.open("rb") as file:
    digest = hashlib.file_digest(file, "sha256").hexdigest()
if digest != LOCK["binary"]["sha256"]:
    raise SystemExit(f"Baseline binary checksum mismatch: {digest}")
with tarfile.open(archive) as tar:
    tar.extractall(CACHE / "bin", filter="data")
binaries = [p for p in (CACHE / "bin").rglob("codex*") if p.is_file() and p.stat().st_mode & 0o111]
if len(binaries) != 1:
    raise SystemExit("Expected one release executable")
version = subprocess.check_output([str(binaries[0]), "--version"], text=True).strip()
if version != f"codex-cli {LOCK['release']}":
    raise SystemExit(f"Baseline version mismatch: {version}")
print(f"Verified {version}\nCommit: {head}\nSource: {source}\nBinary: {binaries[0]}")
