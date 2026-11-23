# Changelog

## 1.2.3

- Compact windows switch directly to a retractable sidebar with full navigation.

## 1.2.2

- Overview shows collection cards by default when collections exist. Switch to All to see every repository, including unassigned repositories.
- Changes, Commit history, and Activity use flatter surfaces and clearer selection highlights. The Changes diff scrolls in its own panel and keeps its position during refresh.
- Changes highlights the next branch action and groups secondary actions under More actions.
- Settings places A quick tour and Live logs side by side where space allows and improves description contrast.

## 1.2.0

- Reponiq now uses React and shadcn/ui for its sidebar, dialogs and controls, with the existing desktop theme.
- Browse themes in Settings or during setup. Preview and apply external palettes from shadcn/ui, tweakcn or a supported theme link.
- Overview puts latest activity above the repository results, with grid and row controls next to search.
- Choose a repository when creating a collection and dismiss feedback from the bottom-right corner.

## 1.1.0

- Commit all changes by default, or include or exclude checked files. The workbench shows the commit scope and offers direct access to the Git author fields when needed.
- Switch with edits, stash before switching, or stash on the current branch. Restore from Saved changes and use the sync action shown for the current branch.
- Reuse saved drafts, personalize launch fetch and sign-in startup, and keep project pins visible with solid pin icons.
- Windows and Linux AppImage prepare signed updates in the background. Install 1.1.0 manually when upgrading from 1.0.x.

## 1.0.1 — 2026-11-06

The repository shelf loads saved metadata before background Git checks. Pinning and organization update only the affected entry; local status refreshes automatically on focus and during idle use, with manual refresh buttons removed. Native work queues small bursts instead of rejecting the fifth request as busy.

GitHub Desktop import also checks the saved repository folder for entries missing from its retained logs. Failed imports identify each folder and a recovery reason, preserve successful entries, and can retry remaining folders. Git ownership refusals have a fixed explanatory message. Import completion clears an old shelf search so added entries are visible.

## 1.0.0 — 2026-11-06

First consolidated Reponiq release. A native Windows/Linux Git workspace with a dark neumorphic interface, a project shelf, collections and Enter-to-add tags, a native folder picker, first-run default-folder setup and a skippable contextual walkthrough. Review automatic folder discoveries or import local repositories from GitHub Desktop history, VS Code recent folders or a folder used by another Git client. Create local/GitHub repositories under personal or organization owners, add folders, or clone repositories grouped by connected account.

The focused workbench includes per-file staging, separate commits, selectable commit/file diffs, safe revert commits, branch selection/creation, fetch, guarded pull/push, and text conflict resolution with continue/abort.

GitHub browser sign-in supports manual Actions monitoring and in-app pull-request inspection, reviews, same-repository creation and guarded merge. Other Git platforms use installed system Git with provider identification.

Windows x64 EXE and Linux x64 deb/rpm/AppImage packages use the OS webview. There is no telemetry, Reponiq backend, automatic updater or signing certificate. See [release verification](docs/RELEASE_CHECKLIST.md) for observed platform checks and limits.
