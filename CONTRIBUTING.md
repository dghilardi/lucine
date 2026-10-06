# Contributing

This document defines the development and review workflow. Start with README for setup and AGENTS.md for scope.

Use a branch for each focused change. Commits follow Conventional Commits, for example `fix(protocol): reject malformed white coordinates` or `test(ui): cover stale-state recovery`. Each commit should describe one coherent change; keep formatting-only edits separate from unrelated behavior changes.

Run the checks in README. Add useful regression tests for parsing, credential handling, command semantics and interaction failures. Tests must use invented fixtures and must not connect to the provider or alter hardware. The browser preview has disabled commands by design.

Before opening a pull request, inspect the staged diff and run the publication check and a full-history secret scan. Explain the problem, resulting behavior, tests and limitations. Do not attach sessions, APKs, decompiled source, vendor assets, authenticated logs or screenshots. Report vulnerabilities through SECURITY.md rather than a public issue with private data.

Compatibility expansion requires evidence from an authorized device, a maintained protocol boundary, hardware-free regression tests and updated compatibility documentation. Do not infer support from branding or share real account artifacts as evidence.
