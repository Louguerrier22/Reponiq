# Security

Reponiq is a local native application. The current candidate’s full audit, platform verification, and publication status are recorded in RELEASE_CHECKLIST.md; a successful build is not a security certification.

## Credentials and local data

GitHub personal access tokens are validated with GitHub.com and stored in the OS credential store. The database stores login/host metadata, repository organization, and activity, not the token. URL userinfo, query strings, and fragments are stripped before remote metadata is persisted and existing stored remotes are sanitized on startup. Git configuration and old backups may still contain credentials; Reponiq does not rewrite those files.

Normal Git error messages expose a generic status. Live logs additionally store bounded, redacted Git stderr and application diagnostics locally. Log redaction handles common credential fields and known account secrets, but is not a substitute for reviewing private details before sharing. API errors do not echo response bodies. Never paste tokens into logs, issues, screenshots, or pull requests. Revoke compromised tokens through GitHub. Use fine-grained read-only Actions permissions for monitoring alone. PR review and merge require additional write permissions; browser public_repo/repo scopes are broader and are disclosed before sign-in.

## Trust boundaries

Tauri exposes an explicit IPC command allowlist and minimal desktop capabilities. Git receives argument arrays without a shell. Register only repositories you trust: Git hooks, filters, helpers, signing tools, and configured remotes remain executable behavior under your local account. Reponiq is not a sandbox for hostile repositories.

Local Git work runs in bounded background workers. Git output and command time are limited; a timeout or interrupted mutation is not proof that no changes occurred. Check repository state before retrying. Device access and database backups can reveal private paths, project names, activity, and account metadata.

Token-bearing REST requests are restricted to `api.github.com` over HTTPS. Device authorization and credential refresh use fixed `github.com` HTTPS endpoints. Redirects are refused, responses are bounded, and no client secret is embedded. OAuth tokens never enter webview state. Public account-list clones run without authentication. Private account-list cloning supplies the selected account token through a command-local Git credential helper, scoped to the exact HTTPS repository on `github.com`, with redirects disabled. The token is passed in the child environment and is not saved to Git configuration, remote URLs, or system credential helpers. URL cloning uses system Git authentication and rejects embedded credentials and executable remote-helper URLs. PR merge revalidates the reviewed SHA and provider policy; system Git operations separately validate current branch/HEAD and working state. No telemetry or Reponiq-hosted server is configured. Other system Git operations use their own credentials and network settings independently.

## Reporting and distribution

Use the repository’s private reporting channel if available; otherwise contact its owner through the private repository without including credentials or private exploit details. Do not publish sensitive reports as public issues. This document does not claim GitHub private vulnerability reporting is enabled.

v1 binaries are unsigned. Release checksums provide integrity verification, not a signing certificate. There is no automatic updater. The build workflow has read-only repository contents permission and produces artifacts only; it cannot publish a release. Publication requires exact-candidate checks, both platform packages, and the mandatory full security audit.
