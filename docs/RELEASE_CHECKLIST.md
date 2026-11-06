# Release verification

[Documentation](README.md) · [Installation](INSTALLATION.md) · [Reponiq v1](https://github.com/Louguerrier22/Reponiq/releases/tag/v1.0.0)

Reponiq 1.0.0 is one consolidated source commit, identified by tag v1.0.0. The release notes link the exact packaging run and source identity. SHA256SUMS.txt on that same release identifies the distributed Windows EXE and Linux deb/rpm/AppImage files. Earlier experimental packages and history are superseded.

## Source and local runtime checks

| Surface | Observed checks |
| --- | --- |
| Frontend | 18 tests; Svelte/TypeScript production build with zero errors and warnings. |
| Windows native core | 34 tests covering real local Git operations, separate-file commits, merge/revert conflicts, deleted upstream guards, literal paths, catalog init/clone, retained directory authority, credential identity and bounded provider transport. |
| Native interface | Real disposable-repository creation, per-file staging and commits, history/diff selection, browser device-code start/open/cancel, native folder selection, persisted walkthrough skip and replay, first-run setup, saved folder reload and reviewed discovery/import of four disposable repositories. |
| Compact layout | Create/Clone/PR/conflict screens checked at 760px, without page-level horizontal overflow. Checked controls use visible lavender fill and a dark check. |
| Documentation | Actual native application captures with disposable repositories, cropped to exclude private paths. No generated UI images. |

Repository-local Git executable selection and filesystem handling have dedicated regression checks. Linux adds tests for alias substitution, fd-bound subprocess directories and nonblocking rejection of FIFOs. The linked packaging run records their actual target-platform outcome.

## Release gates

Publication requires the full audit for this exact source and installer digests, current locked dependency/rights review, privacy inspection, source checks and successful Windows/Linux packaging. A previous build's audit does not cover a new candidate. The workflow produces packages; publication is a separate step after the checks.

## Limits of the evidence

PR interaction tests use synthetic provider fixtures and bounded HTTP fixtures; they do not write to real user repositories. GitHub remains authoritative for account permissions, organization policy and branch protection. A real-user credential refresh/expiry lifecycle, full installed-package qualification, broad Linux distribution testing and independent accessibility certification are not claimed.

Installers are unsigned. Checksums verify integrity, not publisher certification. No automatic updater is included. This consolidated baseline has no supported upgrade/rollback contract with experimental builds; ordinary saved-data preservation remains tested.
