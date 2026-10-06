#!/usr/bin/env python3
"""Check tracked files for private artifacts and accidental installation-derived fixtures.

This complements a full-history secret scanner; it does not prove legal safety
or detect every possible secret. It never prints file contents.
"""
import json
from pathlib import Path
import re
import subprocess
import sys

FORBIDDEN_PARTS = {'.analysis', '.venv', '.impeccable', 'captures', 'secrets',
                   'node_modules', 'target', 'dist', '__pycache__', 'gen'}
FORBIDDEN_SUFFIXES = {'.apk', '.aab', '.dex', '.so', '.java', '.log', '.pyc',
                      '.AppImage', '.deb', '.jks', '.keystore', '.pem'}


def inspect_file(name: str, data: bytes) -> list[str]:
    path = Path(name)
    problems = []
    if FORBIDDEN_PARTS.intersection(path.parts) or path.suffix in FORBIDDEN_SUFFIXES:
        problems.append('private or generated artifact')
    if path.name.startswith('.env') or (path.name.startswith('session') and path.suffix == '.json' and name != 'examples/session.example.json'):
        problems.append('credential file')
    if path.suffix == '.png' and name != 'src-tauri/icons/icon.png':
        problems.append('unreviewed bitmap asset')
    if len(data) > 1_000_000:
        problems.append('unexpected large file')
    text = data.decode('utf-8', errors='ignore')
    if re.search(r'/home/[\w.-]+/|/Users/[\w.-]+/', text):
        problems.append('workstation-specific absolute path')
    if name == 'public/preview-state.json':
        try:
            lamps = json.loads(text)
            if not isinstance(lamps, list) or any(
                not lamp['id'].startswith('demo-') or not lamp['name'].startswith('Lampadina demo ')
                for lamp in lamps
            ):
                problems.append('demo fixture must use invented IDs and names')
        except (ValueError, KeyError, TypeError, AttributeError):
            problems.append('invalid demo fixture')
    if name == 'examples/session.example.json':
        try:
            session = json.loads(text)
            expected = {'AmToken': 'YOUR_TOKEN', 'amDomain': 'region.iotdreamcatcher.net',
                        'amPort': 443, 'UserId': 'YOUR_USER_ID', 'userDB': 'YOUR_USER_DB',
                        'homeID': 1, 'homeDB': 'YOUR_HOME_DB'}
            if session != expected:
                problems.append('session example must contain only reviewed placeholders')
        except ValueError:
            problems.append('invalid session example')
    return problems


def main() -> int:
    names = subprocess.check_output(['git', 'ls-files', '-z']).decode().split('\0')
    findings = [(name, problem) for name in names if name
                for problem in inspect_file(name, Path(name).read_bytes())]
    for name, problem in findings:
        print(f'{name}: {problem}', file=sys.stderr)
    if findings:
        return 1
    print(f'Publication checks passed for {len([name for name in names if name])} tracked files.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
