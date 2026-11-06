# Performance

[Documentation](README.md) · [Architecture](ARCHITECTURE.md)

Reponiq uses the OS webview with a Rust backend. Fonts, icons and frontend assets are bundled locally; no CDN or external font request is needed to run the app.

## Interaction behavior

Changing a file, branch, collection, tab or checkbox updates component state rather than navigating or reloading the document. Keyed file/commit/PR lists preserve item identity. Re-selecting the same commit or file avoids a duplicate request. A Git mutation refreshes only the selected repository and activity; it does not rescan every registered repository. Manual shelf refresh remains available. PR and history views reject stale asynchronous results.

A Windows browser fixture staging check observed the same application-shell node and one unchanged navigation entry, with no list_repos or refresh_repos call. The fixture interaction completed in 24 ms; this measures UI/mock transport only, not real Git latency. Native disposable-repository checks separately verify real staging and split commits.

## Work limits

Shelf status uses one porcelain-v2 query per repository. Native worker jobs are capped at four; Git subprocess work is serialized. Git output is capped at 4 MiB with local/network deadlines. File diffs display at most 1,500 lines / 200,000 characters, and changed files render at most 200 filtered matches. Repositories page at 50. History/PR limits are documented in the user guide.

Refreshes are coalesced; inactive or hidden work is deferred. Timed local refresh pauses during dialogs, mutations and hidden windows. GitHub monitoring is manual. The contextual tour disables background animation and uses no blur; reduced motion disables interface animation globally.

The local final feature bundle measured164.94kB JavaScript /54.13kB gzip and59.24kB CSS /13.25kB gzip before final copy changes. These are local build observations, not a field latency guarantee. Cold/warm startup, large-workspace latency, idle CPU and memory vary by system and have not been comprehensively benchmarked. No universal fastest-app or zero-delay claim is made.
