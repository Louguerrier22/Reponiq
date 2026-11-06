# Development

Use Node.js 22, npm, stable Rust, Git, and the native [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for the host platform. Windows builds need Microsoft C++ Build Tools and WebView2. Linux builds need WebKitGTK 4.1, GTK, appindicator, OpenSSL, D-Bus, and packaging libraries.

Install dependencies with `npm ci`; both npm and Cargo lockfiles are committed. Start with `npm run tauri dev`. A plain `npm run dev` opens a web preview without native Git/database commands; browser tests inject fixtures only in the test harness.

```bash
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml
node scripts/release.mjs verify
```

`npm run build` checks Svelte/TypeScript and creates the frontend bundle. Native installers require `npm run tauri build -- --bundles nsis` on Windows or `npm run tauri build -- --bundles deb,rpm,appimage` on Linux. Rebuild after any source or icon change. `assets/reponiq.svg` owns the original app mark; `npm run tauri icon -- assets/reponiq.svg` regenerates platform artwork.

## Release

Version the npm package/lockfile, Cargo manifest/lockfile, and Tauri configuration together. Build/test the exact candidate. The workflow runs frontend checks and native Windows/Linux packaging, with a Linux startup smoke test and exact installer collection. `node scripts/release.mjs collect windows-x64` (or `linux-x64`) copies only the declared installer formats and writes SHA-256 checksums.

Push the reviewed source before creating the matching version tag (currently `v1.0.1`). Tags must match the application version and descend from `origin/main`. Collect both platform artifacts from the same commit, combine checksums, finish the full candidate-bound security audit and applicable release checks, then publish the GitHub release with those exact files. CI cannot publish a release. Do not advertise unbuilt formats or substitute source ZIPs for installers.

Release signing and automatic updates are not configured. Native builds, installed/runtime evidence, visual review, and publication are tracked separately in [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md).

## Ownership

`src/App.svelte` owns interface state and orchestration; `src/lib/Modal.svelte` owns native dialog focus behavior; `src/lib/api.ts` owns typed IPC and filtering. `src-tauri/src/lib.rs` owns bounded worker commands, Git, SQLite, GitHub, and the tray. `src-tauri/src/storage.rs` owns the rename-compatible storage selection. Keep compatible database contracts and use isolated fixtures for tests.

## Dependency notices

The installer includes LICENSE and THIRD_PARTY_NOTICES.txt. After dependency changes, run `cargo metadata --locked --format-version 1 --manifest-path src-tauri/Cargo.toml > .work/cargo-metadata.json`, then `python scripts/generate-notices.py`. Review the resulting SPDX/source inventory and missing-license-file list; source links do not replace any applicable redistribution obligations. The inventory conservatively includes build-time and target-specific dependencies, so not every listed package ships in each binary.
