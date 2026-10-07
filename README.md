# Lucine

An experimental Linux desktop client for white smart bulbs managed through DreamCatcher Life. Built with Tauri 2, Rust and TypeScript, with an Italian interface.

Lucine is an independent project, unaffiliated with Chuango or DreamCatcher. Those names identify compatibility only. This is not an official integration or a replacement for the manufacturer's app.

## Features

- Compact rows show the bulbs returned by your account; no device IDs or names are built in.
- Power, brightness and warm / cool white controls.
- Device-confirmed state, per-bulb connection errors and manual refresh.
- Light / dark system theme and keyboard controls.
- Custom zones with power, brightness and warm / cool white controls from the window and tray.
- Local scenes with distinct power, brightness and white settings for each bulb, activated from the window or tray.
- Optional launch at Linux login, initially disabled, opening in the tray.
- System tray with **Apri Lucine** and **Esci**. Closing the window hides it in the tray; repeated launches reopen the existing instance.

Compatibility is currently limited to white bulbs identified by the service as `product_id: "12"` (reported as G95G by the Android app). Other products, RGB, firmware versions and regions have not been validated. See [compatibility](docs/compatibility.md).

## Important limitations

An authenticated session must be imported. Desktop login and automatic session renewal are **not implemented**. You need Internet and a valid account with access to your own bulbs; no emulator is needed during normal use. Lucine cannot create a session for you.

The cloud interface is undocumented and may change. Third-party access has not been approved by the manufacturer. This project cannot promise that using it complies with every applicable account agreement or that an account will never be restricted. Read [publication and interoperability notes](docs/publication.md) before using it.

The previous “Neutro” preset (`161`) was unverified and has been removed. Existing scenes containing it remain editable, but must be changed to **Caldo** or **Freddo** before activation. White preset commands include brightness. MQTT state reads and confirmations ignore retained broker snapshots and require a response after the query is sent; the reported state is a device confirmation, not optical proof that LEDs are emitting light.

## Build and run

Use Rust 1.92 or newer, Node 24.15 or newer (Node 26 is also supported), npm, and the [Linux dependencies for Tauri 2](https://v2.tauri.app/start/prerequisites/#linux). On Ubuntu / Debian these include WebKitGTK 4.1, OpenSSL and Ayatana appindicator development packages. Other distributions use different package names.

```sh
npm ci
npm run tauri dev
```

Build an AppImage:

```sh
npm run tauri build -- --bundles appimage
```

The executable is written under `src-tauri/target/release/bundle/appimage/`. Build on the oldest Linux distribution you intend to support: an AppImage is not a guarantee of compatibility with older glibc or graphics stacks. A desktop tray host is required to reopen a hidden window.

`npm run dev` opens only the browser preview, with invented demo data and disabled hardware controls. It does not authenticate or contact the manufacturer's cloud.

## Import a session

Use **Impostazioni** in the native app to enter the path to a session JSON file you are authorized to use. The format is described in [session import](docs/session.md); the example contains placeholders, not working credentials. Session extraction, device rooting and proprietary app distribution are outside the public project's scope.

The CLI also supports import and a read-only check:

```sh
./Lucine_0.1.0_amd64.AppImage --import-session /path/to/session.json --probe
```

The local session is stored in `$XDG_CONFIG_HOME/it.local.lucine/session.json`, or `~/.config/it.local.lucine/session.json` when `XDG_CONFIG_HOME` is unset. On Linux the file has `0600` permissions. It is a plaintext credential file, not an encrypted vault: protect it and your backups. No session is included in the source or application bundle.

While visible and idle, Lucine polls once per minute. Refresh and import do not change bulb settings. Hardware commands occur only on user interaction. Brightness and white controls are disabled while a bulb is off.

## Zones and automatic startup

Open **Impostazioni → Zone nella tray**, enter a name and select at least one bulb, then choose **Crea zona**. Existing zones can be edited or deleted. Membership is stored locally in `zones.json` next to the session, with private filesystem permissions; it is not a cloud room or a shared configuration.

Expand **Zone e scene** in the window to choose a zone and apply power, brightness (1–100%) or a white preset. Each tray zone also offers **Accendi**, **Spegni**, brightness shortcuts (25 / 50 / 75 / 100%) and white presets. Brightness and white adjustments require bulbs to be on: off bulbs stay off and are reported individually. Lucine rechecks the current account's supported devices before sending commands. Missing or offline bulbs produce partial failures without preventing the remaining bulbs from completing. Explicit power requests are always sent, even if the reported power state already matches. Up to four bulbs run concurrently; repeated tray clicks are blocked while a zone action is pending. The tray reports confirmed results, and failures open the window for review. Changing a zone never sends commands.

Open **Impostazioni → Scene locali** to name a scene and select its bulbs. For each selected bulb choose **Accesa** with brightness and white, or **Spenta**. Saving, editing or deleting a scene never sends hardware commands. Activate it explicitly from **Zone e scene** or **Scene** in the tray. Scenes are stored privately in local `scenes.json`, next to the session; they do not import, modify or synchronize the manufacturer's cloud scenes or rooms.

Scenes use the same account checks and concurrency limit as zones. For an on target, Lucine confirms power, then white, then brightness; an off target only switches off. A failure stops subsequent steps for that bulb and leaves other bulbs running. There is no atomic multi-device transaction or rollback: an earlier step may have succeeded before a later failure. Review errors and refresh before retrying. Scenes do not run at startup, and timers / schedules are not implemented.

Enable **Avvia Lucine all’accesso a Linux** only if wanted. This starts the app after logging into the desktop, not before login or as a system service. Startup opens in the tray and never switches bulbs automatically. It uses the executable / AppImage path from which the setting is enabled: move the AppImage to a permanent location first, and disable then re-enable this option if that path changes. Development builds also register their own build path if you enable the setting there.

## Checks

```sh
npm run format:check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
python3 scripts/check_public_tree.py
```

Tests use synthetic fixtures and never connect to a live account or change hardware. CI runs the checks and scans Git history for secrets. See [contributing](CONTRIBUTING.md) for review requirements and atomic Conventional Commits.

## Documentation

| Document | Purpose |
|---|---|
| [Compatibility](docs/compatibility.md) | Narrow support claims and unsupported products |
| [Architecture](docs/architecture.md) | Data flow, protocol boundaries and security decisions |
| [Session import](docs/session.md) | Credential format, storage and renewal limitations |
| [Publication notes](docs/publication.md) | Provenance, excluded material, legal uncertainty and review findings |
| [Security policy](SECURITY.md) | Safe vulnerability reporting and credential handling |
| [Contributing](CONTRIBUTING.md) | Development and review workflow |
| [Agent instructions](AGENTS.md) | Repository scope and instructions for coding agents |
| [Changelog](CHANGELOG.md) | User-visible changes and known limitations |

## License

Original project code and documentation are licensed under [MIT](LICENSE). This license does not cover the manufacturer's software, services or trademarks, and grants no access to its cloud. Dependencies retain their own licenses; see [third-party notices](THIRD_PARTY_NOTICES.md).
