# Changelog

This document records user-visible changes and limitations. Changes follow Keep a Changelog categories; version numbers describe Lucine, not vendor firmware.

## Unreleased

### Added

- Experimental Linux Tauri desktop client with dynamic compact lamp rows and tray controls.
- Power, brightness and warm / neutral / cool white commands for service product ID `12`.
- Session import, read-only CLI probe and per-device command confirmation.
- Invented browser demo, hardware-free protocol and interaction tests, and CI checks.
- Scope, compatibility, security, provenance and contribution documentation.

### Security

- Credential tokens remain in the native backend and private session files.
- TLS endpoint validation, rejected redirects and no tokens in authenticated URLs.
- Atomic credential-file replacement with Linux `0600` permissions.
- Publication guard and full-history secret scan.

### Known limitations

- No standalone login, automatic session renewal, local-LAN control or other product support.
- Requires the provider's undocumented cloud and a valid session; third-party access is not manufacturer-approved.
- Linux only; other OSes, hardware revisions and regions are unverified.
- No public binary release has been audited or published.
