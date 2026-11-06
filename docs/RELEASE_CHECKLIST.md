# Release verification

[Documentation](README.md) · [Installation](INSTALLATION.md) · [Reponiq v1](https://github.com/Louguerrier22/Reponiq/releases/tag/v1.0.1)

Reponiq 1.0.1 is the follow-up to the consolidated v1 baseline, identified by tag v1.0.1. The release notes link the exact packaging run and source identity. SHA256SUMS.txt on that same release identifies the distributed Windows EXE and Linux deb/rpm/AppImage files. The published v1.0.0 baseline remains available.

## Source and local runtime checks

| Surface | Observed checks |
| --- | --- |
| Frontend | 18 tests; Svelte/TypeScript production build with zero errors and warnings. |
| Windows native core | 37 local Windows tests covering real local Git operations, separate-file commits, merge/revert conflicts, deleted upstream guards, literal paths, catalog init/clone, retained directory authority, credential identity and bounded provider transport. |
| Native interface | Real disposable-repository creation, per-file staging and commits, history/diff selection, browser device-code start/open/cancel, native folder selection, persisted walkthrough skip and replay, first-run setup, saved folder reload and reviewed discovery/import of four disposable repositories. |
| Compact layout | Create/Clone/PR/conflict screens checked at 760px, without page-level horizontal overflow. Checked controls use visible lavender fill and a dark check. |
| Documentation | Actual native application captures with disposable repositories, cropped to exclude private paths. No generated UI images. |

Repository-local Git executable selection and filesystem handling have dedicated regression checks. Linux adds tests for alias substitution, fd-bound subprocess directories and nonblocking rejection of FIFOs. The linked packaging run records their actual target-platform outcome.

## Release gates

Publication requires the full audit for this exact source and installer digests, current locked dependency/rights review, privacy inspection, source checks and successful Windows/Linux packaging. A previous build's audit does not cover a new candidate. The workflow produces packages; publication is a separate step after the checks.

## Limits of the evidence

PR interaction tests use synthetic provider fixtures and bounded HTTP fixtures; they do not write to real user repositories. GitHub remains authoritative for account permissions, organization policy and branch protection. A real-user credential refresh/expiry lifecycle, full installed-package qualification, broad Linux distribution testing and independent accessibility certification are not claimed.

Installers are unsigned. Checksums verify integrity, not publisher certification. No automatic updater is included. The update keeps the same SQLite tables, preference keys and credential format as v1.0.0. Reader/writer preservation tests retain unknown data and current records. Runtime status_pending and per-import failures are additive IPC fields; matched UI/native packages ship together. Experimental pre-v1 builds have no supported coexistence contract.
