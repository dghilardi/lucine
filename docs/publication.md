# Publication and interoperability review

This document records the public-source boundary and the limits of the review. It is not a legal opinion or permission from the service provider.

## Provenance and exclusions

Lucine's Rust / TypeScript implementation, UI and application icon were written for this project. Protocol observations were informed by examination of a lawfully obtained Android app and authorized tests with owned bulbs. This was **not** a separated clean-room process, and no claim of clean-room provenance is made.

The public repository contains original implementation code, synthetic fixtures and the minimum descriptions needed to maintain its interoperability. It does not include APKs, decompiled Java, native vendor libraries, vendor logos, screenshots from an authenticated account, traffic captures, device identifiers, account data, session tokens, emulator images, app patches or extraction / rooting tools. Local analysis material and exploratory clients are outside the published tree and Git history.

Names and protocol field / action names identify the third-party system being interoperated with. They are not a grant of rights in the manufacturer's application or service. The MIT license applies to the original project only; open-source dependencies retain their licenses.

## Findings and changes for publication

- Replaced installation-derived demo names and states with invented fixtures.
- Removed an installation-specific live test and exploratory / credential extraction tools from the public source.
- Replaced workstation-specific documentation with reproducible setup instructions.
- Removed unnecessary Android identity headers and authenticated URL token parameters; a read-only request confirmed the simpler authentication path.
- Restricted account / broker endpoints to vendor DNS suffixes, disabled REST redirects, validated MQTT topic fragments and kept TLS verification enabled.
- Made credential persistence atomic and private; unknown credential fields are discarded.
- Added hardware-free backend / UI regression tests, formatting checks, publication checks and Git-history secret scanning.

## Service terms and legal uncertainty

On 2026-10-06 the English terms page served at the URL selected by the Android app was reviewed: [DreamCatcher terms and conditions](https://psb1.iotdreamcatcher.net:12080/dreamcatcher/platform/termsAndConditions/normal/en.html). It addresses intellectual property, network security, interference with the service and suspension / termination for breaches. The reviewed text is not an explicit authorization of third-party clients. Terms can differ by region or change; the agreement accepted by an account holder must be checked directly. The provider's [app privacy policy](https://chuango.de/pages/datenschutzrichtlinie-dreamcatcher-life-app) describes its handling of account and device information; a privacy policy is not an API license.

The EU [Directive 2009/24/EC](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32009L0024) distinguishes ideas / principles from protected expression in Article 1(2), permits certain authorized observation in Article 5(3), and sets conditional interoperability provisions in Article 6. Those provisions are limited; they do not establish that this particular analysis, publication or cloud usage is lawful in every jurisdiction. Publishing decompiled source or copying protected expression is outside this project's scope.

No manufacturer approval or jurisdiction-specific legal review has been obtained. Removing proprietary artifacts and account data reduces exposure but cannot guarantee freedom from complaints, enforcement or account restrictions. Obtain professional advice or written provider permission if that assurance is required. There is no instruction here to evade service controls, bypass authentication, disrupt the cloud or access another person's devices.
