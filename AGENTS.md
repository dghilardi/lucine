# Agent instructions

This file defines repository scope and working rules for coding agents. User instructions take precedence. It is not end-user documentation or a legal opinion.

## Scope

Maintain an independent, experimental Linux Tauri utility for product-ID-12 white bulbs. Device discovery must remain account-driven; never hard-code a user's installation. Keep support claims narrow and evidence-based. Cloud room CRUD and manual power-scene CRUD are within scope; other cloud automations and task formats must remain read-only. Desktop login, pairing, other device categories, extraction tools and local-LAN control are outside the current implementation.

## Privacy and provenance

- Never read private account artifacts unless the user explicitly authorizes the particular task. Never print tokens or raw authenticated requests.
- Never stage `.analysis/`, `captures/`, `secrets/`, session files, emulator artifacts, vendor APKs, decompiled source, native vendor libraries or authenticated screenshots.
- Use invented fixtures. Do not copy device names, states, IDs, account details or local paths into tests, docs or demos.
- Keep credential-bearing network requests in Rust, validate destinations, preserve TLS verification and sanitized errors, and never log tokens.
- The only query-token exception is the verified `PUT /v2/user/device/account` membership endpoint. Never log its URL, extend it to other endpoints or expose raw cloud configurations over IPC.
- Do not weaken domain validation or add manufacturer identity headers without a documented reason and review.
- Do not claim clean-room analysis, official approval, guaranteed legal safety or account safety. Do not copy vendor code or assets.

## Workflow

Read README, relevant docs and tests before editing. Keep changes focused. Use atomic Conventional Commits, stage explicit paths, inspect the diff and run relevant checks before committing. Review all public files and Git history before publishing. Do not create a release from workstation build artifacts; binaries need a separate dependency-license and reproducibility review.

Default checks are listed in README and run in CI. Tests must be hardware-free, use no cloud credentials and send no live commands. Live hardware testing requires explicit user authorization for that test; never make it an automatic fallback for failing unit tests.

## Documentation ownership

README covers setup and limitations; `docs/compatibility.md` defines supported products; `docs/architecture.md` covers implementation; `docs/session.md` defines credential import; `docs/publication.md` records provenance and review limits. SECURITY governs reporting. CONTRIBUTING governs contribution checks and commits. CHANGELOG records user-visible changes, with new changes under Unreleased. Update the relevant document when behavior or scope changes.
