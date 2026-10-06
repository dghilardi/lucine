import json
import unittest
from check_public_tree import inspect_file


class PublicationChecks(unittest.TestCase):
    def test_rejects_captures_and_vendor_binaries(self):
        for path in ('captures/reply.json', '.analysis/source.java', 'vendor/lib.so', 'session.json'):
            self.assertTrue(inspect_file(path, b'{}'))

    def test_rejects_absolute_workstation_paths(self):
        # Build the synthetic path in parts so this test itself passes the scanner.
        self.assertTrue(inspect_file('README.md', ('/' + 'home/' + 'demo/file').encode()))

    def test_rejects_real_looking_preview_identifiers(self):
        value = json.dumps([{'id': 'device-123', 'name': 'Private name'}]).encode()
        self.assertTrue(inspect_file('public/preview-state.json', value))
        value = json.dumps([{'id': 'demo-1', 'name': 'Lampadina demo 1'}]).encode()
        self.assertFalse(inspect_file('public/preview-state.json', value))

    def test_rejects_altered_session_example(self):
        self.assertTrue(inspect_file('examples/session.example.json', b'{"AmToken":"unexpected"}'))

    def test_allows_original_icon_and_source(self):
        self.assertFalse(inspect_file('src-tauri/icons/icon.png', b'PNG'))
        self.assertFalse(inspect_file('src/main.ts', b'const demo = true;'))
        self.assertFalse(inspect_file('docs/session.md', b'Session documentation'))


if __name__ == '__main__':
    unittest.main()
