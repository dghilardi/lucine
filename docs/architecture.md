# Architecture

This document explains the maintained implementation and its trust boundaries. It does not reproduce the manufacturer's source code.

## Components

- `src/main.ts`: Italian UI, dynamic lamp rows, command debounce, error handling and one-minute polling only while visible and idle. Static templates contain no account data; dynamic text is assigned with `textContent`.
- `src-tauri/src/lib.rs`: Tauri commands, configuration location, CLI and tray lifecycle.
- `src-tauri/src/protocol.rs`: credential validation and persistence, REST enumeration, MQTT TLS, state normalization and command confirmation.
- `src-tauri/src/zones.rs`: validated local zone membership and atomic private persistence.
- `src-tauri/src/local_store.rs`: bounded reads and shared atomic private persistence for zones and scenes.
- `src-tauri/src/scenes.rs`: validated local scenes, per-bulb targets and persistence.
- `src/scenes.ts` and `src/shortcuts.ts`: scene editing and explicit zone / scene activation.
- `src/settings.ts`: independent startup, zones and session settings, with synthetic interaction tests.
- `public/preview-state.json`: invented browser-only demonstration data. Native builds never use it for device discovery.

REST HTTPS returns the authorized account's bulbs and per-device MQTT endpoints and credentials. Only product ID `12` enters the control path. Account credentials stay in Rust and the local session file; only display data crosses the Tauri IPC boundary. The CSP restricts the webview to local resources and IPC, without remote page navigation or webview HTTP access to the provider.

## Connection and command lifecycle

REST device enumeration uses `/v2/user/device/list`, home identifiers in the query and the session token in a header. No Android app identity, brand or OS metadata is sent. Regional servers are supplied by the imported session, not embedded account configuration.

Each state read or command creates a short-lived MQTT 3.1.1 TLS connection with a unique client ID and device-specific credentials. It subscribes to the target device's response topic before publishing. Topic fragments, product ID and vendor DNS names are validated. Connections have a timeout; there is no endless retry loop.

The request envelope is `m.req`; supported state responses are `m.res` with action `bulb_conf`. Mutations use action `value_set`. Numeric modes describe power and the white presets; these are protocol constants, not installation settings. On power-up, the bulb restores a previous mode rather than echoing the power-on command code. Brightness commands preserve a supported scene or valid custom white coordinates. Unknown or incomplete states are not actionable.

A mutation is successful only after the bulb reports a matching state, including white coordinates when supplied. Publishing alone is not a success acknowledgment. The backend serializes refresh, import and control; state reads across bulbs run concurrently. UI controls are disabled during their pending operations, and a command failure requires a fresh read before that bulb can be controlled again.

State queries and command confirmations reject retained MQTT messages and responses received before the query has been sent. An explicit power command is always published, even when the preceding state reports the same power setting. White preset requests include both mode and brightness. Mode `161` is not exposed or emitted: previous neutral scenes are preserved for editing but rejected before activation; brightness commands cannot propagate that legacy mode.

## Review boundaries

Unit tests cover protocol edge cases, endpoint validation and credential file permissions. UI tests cover preview isolation, command debounce, error handling, text injection and dynamic names. Tests and CI are hardware-free. No telemetry, analytics, background login automation or public credential collection is implemented.

The session file is plaintext. TLS still depends on the operating system's trust store and network environment. Cloud traffic reveals authenticated activity to the provider, and the integration remains dependent on an undocumented service. See [session import](session.md) and [publication notes](publication.md).

## Desktop actions

Zones contain generated UUIDs, user-chosen names and device IDs, without credential copies. Corrupt zone configuration is reported rather than overwritten. The tray rechecks membership against the current authenticated account and supported product type; stale IDs are reported, never contacted. Each action is serialized against other backend work, deduplicates members, limits concurrency to four and returns per-bulb confirmations / errors. Pending tray actions disable individual UI controls and cancel queued brightness changes. Results reach the webview through local Tauri events; partial failures also reveal the window.

Startup registration uses the official Tauri autostart plugin through narrow Rust commands with sanitized errors. Plugin webview permissions are not granted. Registration is off until explicitly requested and launches with `--background`. The native window starts hidden to avoid a login-time flash, then is shown for manual launches. The single-instance plugin is registered first and brings forward an existing manual session; background repeat launches leave it hidden. Linux desktop-session startup is separate from bulb power and does not schedule hardware commands.

Scene targets are strictly validated before network access: 1–128 unique supported-format device IDs; on targets require brightness 1–100 and white mode 160 or 162, while off targets carry neither field. Local scene IDs are generated UUIDs and names are bounded; unknown JSON fields are rejected. Zone brightness / white actions check fresh device state before mutation and report off bulbs without turning them on.

A shared native action path coordinates tray and window requests, blocks duplicate group actions, emits pending / result events, and updates the tray. Saved membership is resolved against fresh account discovery for every activation. Scenes confirm power, white, then brightness in order per bulb; failure stops remaining steps for that bulb. There is no automatic retry or rollback of earlier confirmed steps. No scene runs merely by saving it, launching the app or reading configuration. Local stores use bounded 1 MiB reads, atomic replacements and private file permissions, and corrupt files are not silently overwritten.
