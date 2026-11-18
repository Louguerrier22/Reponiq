# Contributing to Reponiq

Thanks for helping make repository management better. This project is at an early MVP stage; smaller, well-tested changes are preferred over broad refactors.

## Workflow

1. Discuss significant feature changes in an issue before implementation.
2. Make a focused branch (`feat/...`, `fix/...`, `docs/...`).
3. Keep commits scoped and explain the user-visible impact.
4. Run the available frontend and Rust checks listed in [development setup](docs/DEVELOPMENT.md).
5. Open a PR with a concise problem statement, screenshots for UI changes, test results, and any security implications.

Review from the current base and record the PR head SHA, the user problem, and test results. Check whether the change already exists or overlaps another PR. Review files, checks, and open threads before deciding whether to approve, request changes, or mark the PR superseded. Hold drafts and changes with unresolved findings, merge conflicts, or incomplete CI. If the branch changes after review, repeat the affected checks on the new head. After an authorized merge, verify CI on the resulting `main` commit and fast-forward clean local checkouts.

## Signed updates

After the release gates pass, use the exact verified Windows and Linux CI artifacts. Place them and their respective checksum files in `.work/release/windows-x64` and `.work/release/linux-x64`. Run `pwsh -File scripts/sign-updates.ps1` under the Windows account that holds the updater key. The script checks the artifact hashes, signs the NSIS and AppImage packages for the exact app version, and creates `.work/release/latest.json`. The encrypted signing key is kept in the operator's Windows profile with account-bound protection. Maintain a recoverable secure backup of the signing identity before distributing update-enabled builds.

For an authorized public release, attach the signed NSIS and AppImage files under their collected names and attach `latest.json` alongside the existing installers and checksums before making the release latest. The CI workflow builds packages with read-only permissions. The signing script never uploads artifacts.

## PR checklist

- [ ] No credentials, `.env` files, or personal data committed
- [ ] New Git operations require explicit user intent and validation
- [ ] No unexpected shell execution or token-bearing arbitrary URLs
- [ ] Added tests for behavioral changes (or explained why not)
- [ ] Docs and roadmap updated for changed functionality
- [ ] Dependencies are justified and licenses reviewed
- [ ] Windows and Linux CI are green for the reviewed candidate
- [ ] Reviewed user journeys include recovery paths and preserve local work

## Coding conventions

Prefer idiomatic Rust and typed TypeScript. Keep UI effects light, ensure error messages are actionable, and preserve local-first/offline behavior. Git mutations should be testable against disposable repositories. Do not change scope merely to rewrite working code.

## Community standards

Be respectful, specific, and constructive. Report security concerns privately (see [security](docs/SECURITY.md)).
