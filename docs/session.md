# Session import

This document describes the minimum credential interface accepted by Lucine. It is not a login, extraction or device modification guide.

Obtain a valid session through means you are authorized to use, and review the applicable service terms. Lucine does not contain a login implementation or tools for exporting credentials from another application. Never attach a working session to an issue, pull request or support message.

[The example](../examples/session.example.json) uses deliberately nonfunctional placeholders. Required fields:

| Field | Meaning |
|---|---|
| `AmToken` | Account session token |
| `amDomain`, `amPort` | Regional HTTPS endpoint; port can be a JSON number or numeric string |
| `UserId`, `userDB` | Account identifiers used for per-device broker authentication |
| `homeID`, `homeDB` | Home identifiers used in device enumeration |

Additional fields are ignored and are not saved. The import accepts a regular JSON file up to 64 KiB. The file is parsed and validated, then a read-only device-list request checks authentication before it replaces an existing session. Redirects are rejected, TLS certificate validation stays enabled, and tokens are sent in an HTTP header rather than the URL.

The server name must be a DNS hostname under the supported vendor domains. Import cannot direct account tokens to arbitrary hosts. This does not protect against compromise of a vendor domain, a trusted TLS proxy or the local user account.

The saved file is replaced atomically in the configuration directory and has Linux permissions `0600`; new configuration directories use `0700`. It is not encrypted. The frontend receives names, device IDs and normalized states, but no account or broker tokens. Error messages omit raw authenticated URLs and server payloads.

When authentication fails or the session expires, import an updated file. There is no automatic renewal. To disconnect permanently, exit Lucine from its tray menu and remove its saved session file; also remove any imported copies and protect backups. Session invalidation on the service side remains the account provider's responsibility.
