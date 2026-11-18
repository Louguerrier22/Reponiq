# Troubleshooting

[Documentation](README.md) · [User guide](USER_GUIDE.md)

## Blank Linux AppImage window

Use the updated AppImage. Its bundled Fontconfig uses an app-specific cache (`reponiq/fontconfig-v1` under the normal XDG cache directory), while retaining system/user fonts and aliases. This avoids reusing an incompatible shared font cache, which can prevent the frontend from starting. Repository settings, SQLite data, and credentials are not reset. An explicit `FONTCONFIG_FILE` override is respected.

## Inspect live logs

Open **Settings > Live logs**, below **A quick tour**. Reproduce the problem, filter to Errors or search for the relevant command, and inspect the Git diagnostic or stack details. Pause the view when needed. Copy logs or download the visible entries through the save dialog. Review paths and private details before sharing. Recent logs survive restart; the viewer shows their local file path.

## Git is unavailable

Install Git and restart Reponiq so it receives the updated PATH. Reponiq uses system Git; it does not ship another Git installation.

## Push is disabled or rejected

Commit or ignore every changed and untracked file. Finish or abort paused operations. If the remote is ahead, pull first; if histories diverge, use Merge upstream and resolve conflicts. A deleted tracking branch needs an intentional new branch or tracking configuration through Git. Force-push is not exposed.

## Authentication failed

Git fetch, pull and push use your existing Git credentials, SSH agent or credential helper. Connecting GitHub in Reponiq does not configure those. For in-app PRs, select an account with access, grant the required permissions, and satisfy organization/SSO policy. Reconnect an expired or revoked token.

In **GitHub accounts**, public repositories clone without authentication, and private repositories use the selected connected account, including when your system Git is signed into a different account. Reconnect that account if GitHub refuses access. **Repository URL** uses your system Git credentials or SSH key and supports public repositories without an app account. A destination permission error means Git could not write locally; choose a writable parent folder. Failed clones preserve their destination, so inspect it and choose a new folder name before retrying.

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
## Branch and startup recovery

If GitHub shows a branch that Reponiq does not list, open the branch picker, select **Fetch branches**, then search again. Check the remote label if several remotes have the same branch. For a local branch in use by another worktree, open that worktree or switch it away from the branch using Git. If Git reports that switching would overwrite a file, stay on the current branch and commit or stash your edits. Move an obstructing ignored file yourself before retrying.

If switching fails after stashing, return to the original branch and look under **Saved changes on this branch**. If restoring reports conflicts, inspect and resolve them in **Changes** before continuing work. The original stash stays available in the branch picker and in `git stash list`. Inspect any partially restored files before trying again.

If **Fetch on launch** fails, verify network access and system Git credentials, then use **Retry fetch**. Repositories configured to fetch into local branches or tags require explicit manual Fetch. If sign-in startup is unavailable, check OS startup permissions and open the installed app or the original AppImage. Before rolling back to an older build, disable **Start Reponiq at sign-in** in Settings or remove the Reponiq entry using your OS startup settings. The older app can read and write its existing settings while the newer fetch preferences remain saved.

Use a repo's **Manual only** override if the remote needs a VPN or an interactive credential prompt. If Undo cannot verify publication, fetch explicitly or use Revert for shared history. If **Locate folder** reports a different remote, review the folder and add it as a separate repo. A saved editor that moved or was uninstalled can be replaced in Settings.
