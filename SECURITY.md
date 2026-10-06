# Security policy

This document covers vulnerability reporting and credential handling. The current development branch is the only maintained version; there is no response-time guarantee.

Use GitHub's private vulnerability reporting feature on this repository when available. Do not open a public issue containing credentials, device / account identifiers, authenticated captures or exploit instructions against the provider's cloud. If private reporting is unavailable, open only a generic request for a private contact channel, without sensitive details.

Reports should describe the affected Lucine behavior, impact and reproduction with synthetic data. Provider-specific vulnerabilities should be reported through the provider's own security / support channel; this project cannot authorize testing of its infrastructure.

Imported sessions are plaintext local credentials protected by filesystem permissions, not encryption. Protect the machine, imports and backups. If a credential is disclosed, treat it as compromised and use the account provider's session / account controls to invalidate it. Deleting it from a Git commit is not sufficient.

Do not turn off certificate validation or widen network destinations to work around an error. Lucine has no telemetry or update service; do not submit a session file for troubleshooting.
