# Troubleshooting

[Documentation](README.md) · [User guide](USER_GUIDE.md)

## Git is unavailable

Install Git and restart Reponiq so it receives the updated PATH. Reponiq uses system Git; it does not ship another Git installation.

## Push is disabled or rejected

Commit or ignore every changed and untracked file. Finish or abort paused operations. If the remote is ahead, pull first; if histories diverge, use Merge upstream and resolve conflicts. A deleted tracking branch needs an intentional new branch or tracking configuration through Git. Force-push is not exposed.

## Authentication failed

Git fetch, pull and push use your existing Git credentials, SSH agent or credential helper. Connecting GitHub in Reponiq does not configure those. For in-app PRs, select an account with access, grant the required permissions, and satisfy organization/SSO policy. Reconnect an expired or revoked token.

## Browser sign-in does not finish

Open GitHub from the sign-in dialog and confirm the displayed device code. Codes expire; cancel and start again when needed. Do not share codes or paste tokens into issues. If you cancel, pending device sign-in is discarded. A fine-grained PAT is available as an alternative.

## A conflict cannot be edited

The built-in editor supports bounded UTF-8 text files. Resolve binary, oversized or unsafe-path files with your normal Git tools. If another editor changed the file, reopen the conflict to avoid overwriting that work. Once resolved and staged, choose Continue.

## A pull request cannot merge

Refresh it. New commits, failing/pending checks, incomplete status, draft state, requested changes, conflicts or branch policy can block merging. GitHub is authoritative. A network error after submitting an action can leave the result uncertain: refresh before retrying.

## Find the walkthrough again

Open Settings, scroll to A quick tour, and choose Open walkthrough. Skipping or completing it is saved locally.

## Report a problem safely

Include app version, operating system, the action and a redacted error. Remove repository paths, private code, user names, account details, tokens and device codes from screenshots or logs. Use the private security reporting guidance for suspected credential exposure.
