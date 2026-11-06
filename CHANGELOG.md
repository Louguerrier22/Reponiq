# Changelog

## 1.0.1 — 2026-11-06

The repository shelf loads saved metadata before background Git checks. Pinning and organization update only the affected entry; local status refreshes automatically on focus and during idle use, with manual refresh buttons removed. Native work queues small bursts instead of rejecting the fifth request as busy.

GitHub Desktop import also checks the saved repository folder for entries missing from its retained logs. Failed imports identify each folder and a recovery reason, preserve successful entries, and can retry remaining folders. Git ownership refusals have a fixed explanatory message. Import completion clears an old shelf search so added entries are visible.

## 1.0.0 — 2026-11-06

First consolidated Reponiq release. A native Windows/Linux Git workspace with a dark neumorphic interface, a project shelf, collections and Enter-to-add tags, a native folder picker, first-run default-folder setup and a skippable contextual walkthrough. Review automatic folder discoveries or import local repositories from GitHub Desktop history, VS Code recent folders or a folder used by another Git client. Create local/GitHub repositories under personal or organization owners, add folders, or clone repositories grouped by connected account.

The focused workbench includes per-file staging, separate commits, selectable commit/file diffs, safe revert commits, branch selection/creation, fetch, guarded pull/push, and text conflict resolution with continue/abort.

GitHub browser sign-in supports manual Actions monitoring and in-app pull-request inspection, reviews, same-repository creation and guarded merge. Other Git platforms use installed system Git with provider identification.

Windows x64 EXE and Linux x64 deb/rpm/AppImage packages use the OS webview. There is no telemetry, Reponiq backend, automatic updater or signing certificate. See [release verification](docs/RELEASE_CHECKLIST.md) for observed platform checks and limits.
