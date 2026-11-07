# Privacy and local data

Reponiq has no hosted backend, analytics, advertising, crash-reporting service, cookies, or marketing signup. The developer does not receive your local repositories, tokens, or journal through the app.

| Data | Purpose | Storage / recipient |
| --- | --- | --- |
| Repository paths, names, remotes, collections, tags, pins | Workspace organization | Local SQLite database |
| Default repository folder, discovery preference and setup completion | Optional folder setup | Local SQLite preferences |
| Local Git actions and observed workflow runs | Activity journal | Local SQLite database |
| Runtime messages, command timings, Git errors and stack information | Local troubleshooting through Live logs | Memory and two bounded local log files next to the database; exported only when requested |
| GitHub login, API host, connection timestamp | Account list | Local SQLite database |
| GitHub access and refresh credentials | Optional GitHub operations and private account-list clones | OS credential manager; access sent to api.github.com and, for private clones, github.com; device/refresh exchange with github.com |
| Repository files and Git credentials | Explicit Git operations | Existing local repositories and configured Git remotes via system Git |

Repository discovery reads Git markers in the folder you choose. GitHub Desktop import reads bounded local history logs for repository paths and also checks your saved default repository folder when configured; VS Code import reads its recent-folder history. These candidate paths are shown locally for review. Other client accounts, tokens and settings are not imported, and discovery does not send paths to a hosted service. Explicit registration then runs system Git on selected folders.

Browser sign-in contacts GitHub device authorization endpoints and opens the fixed GitHub verification page. Device codes stay in native memory until cancellation or expiry. Account catalogs request accessible repositories and organizations. Creating a remote sends its name, owner and selected visibility to GitHub; cloning downloads through system Git without storing the API token in its origin URL. In-app PR actions send the selected repository, branch, review/comment, PR title/body or merge request to GitHub only when requested. Conflict file contents are not written into the activity journal. Adding a GitHub token validates it with GitHub’s `/user` endpoint. Manual sync requests recent workflow runs for registered GitHub.com repositories. GitHub applies its own terms and privacy practices. Git fetch/pull/push use your Git configuration and may contact the remotes you configured. Installing a missing WebView2 runtime may contact Microsoft.

Remove a repository in Reponiq to remove its workspace record; this never deletes the working tree. Existing activity entries retain event text and may retain repository names or paths inside that text. Disconnecting a GitHub account deletes its saved access/refresh credentials and account record. This does not revoke GitHub authorization; revoke it separately on GitHub. Local records remain on your device until removed; there is no developer-operated retention or remote deletion service. See the user guide for exact file locations. Backups you create are your responsibility.

Live logs retain up to 2,000 entries in memory and two 2 MiB files in the database folder’s `logs` directory. Standard credential fields, URL credentials/query strings, recognized token formats, and known account secrets are redacted. Logs may still include local paths, repository identities, and other private details. Review them before sharing. Clear logs in the viewer to remove the saved diagnostic history.

Do not put tokens in remote URLs. Reponiq strips URL userinfo/query/fragment before storing remote metadata and removes those fields from existing stored remote rows on startup. Credentials already present in Git configuration or old backups are outside that cleanup.

This describes the inspected application behavior, not a certification of every operating system, Git helper, dependency, or user-configured service. Report implementation inconsistencies through the private repository’s security process.
