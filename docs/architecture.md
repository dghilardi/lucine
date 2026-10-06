# Architecture

This document explains the maintained implementation and its trust boundaries. It does not reproduce the manufacturer's source code.

## Components

- `src/main.ts`: Italian UI, dynamic lamp rows, command debounce, error handling and one-minute polling only while visible and idle. Static templates contain no account data; dynamic text is assigned with `textContent`.
- `src-tauri/src/lib.rs`: Tauri commands, configuration location, CLI and tray lifecycle.
- `src-tauri/src/protocol.rs`: credential validation and persistence, REST enumeration, MQTT TLS, state normalization and command confirmation.
- `public/preview-state.json`: invented browser-only demonstration data. Native builds never use it for device discovery.

REST HTTPS returns the authorized account's bulbs and per-device MQTT endpoints and credentials. Only product ID `12` enters the control path. Account credentials stay in Rust and the local session file; only display data crosses the Tauri IPC boundary. The CSP restricts the webview to local resources and IPC, without remote page navigation or webview HTTP access to the provider.

## Connection and command lifecycle

REST device enumeration uses `/v2/user/device/list`, home identifiers in the query and the session token in a header. No Android app identity, brand or OS metadata is sent. Regional servers are supplied by the imported session, not embedded account configuration.

Each state read or command creates a short-lived MQTT 3.1.1 TLS connection with a unique client ID and device-specific credentials. It subscribes to the target device's response topic before publishing. Topic fragments, product ID and vendor DNS names are validated. Connections have a timeout; there is no endless retry loop.

The request envelope is `m.req`; supported state responses are `m.res` with action `bulb_conf`. Mutations use action `value_set`. Numeric modes describe power and the white presets; these are protocol constants, not installation settings. On power-up, the bulb restores a previous mode rather than echoing the power-on command code. Brightness commands preserve a supported scene or valid custom white coordinates. Unknown or incomplete states are not actionable.

A mutation is successful only after the bulb reports a matching state, including white coordinates when supplied. Publishing alone is not a success acknowledgment. The backend serializes refresh, import and control; state reads across bulbs run concurrently. UI controls are disabled during their pending operations, and a command failure requires a fresh read before that bulb can be controlled again.

## Review boundaries

Unit tests cover protocol edge cases, endpoint validation and credential file permissions. UI tests cover preview isolation, command debounce, error handling, text injection and dynamic names. Tests and CI are hardware-free. No telemetry, analytics, background login automation or public credential collection is implemented.

The session file is plaintext. TLS still depends on the operating system's trust store and network environment. Cloud traffic reveals authenticated activity to the provider, and the integration remains dependent on an undocumented service. See [session import](session.md) and [publication notes](publication.md).
