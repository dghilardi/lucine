# Third-party notices

This document identifies direct open-source dependencies and the public project's asset provenance. It is not a complete binary-distribution license inventory.

| Dependency | Role | Declared license |
|---|---|---|
| Tauri, tauri-build, @tauri-apps/api, @tauri-apps/cli | Native shell / build tools | MIT OR Apache-2.0 |
| Lucide | Interface icons | ISC, with included Feather notices |
| serde, serde_json, reqwest, tempfile | Native implementation | MIT OR Apache-2.0 |
| Tokio | Async runtime | MIT |
| rumqttc | MQTT client | Apache-2.0 |
| uuid | Unique MQTT client IDs | Apache-2.0 OR MIT |
| Vite, TypeScript, Prettier, Vitest, jsdom | Development / test tools | MIT, except TypeScript: Apache-2.0 |

The original Lucide notice is retained in [licenses/lucide.txt](licenses/lucide.txt). Tauri API's MIT notice and the Apache-2.0 text are retained in `licenses/`. Exact dependency versions and transitive dependencies are resolved by `package-lock.json` and `src-tauri/Cargo.lock`; consult each upstream package for its own notices and license text.

The Lucine application icon is an original SVG created for this project. There are no manufacturer logos, Android icons, fonts, photographs, screenshots or proprietary binaries in the repository.

Linux builds also depend on system libraries such as GTK, WebKitGTK, OpenSSL and appindicator, which have their own terms. Before distributing an AppImage or other binary, inventory its complete bundled / linked dependencies, provide the required notices and satisfy any applicable source / relinking obligations. The initial public repository distributes source only; local build artifacts are excluded.
