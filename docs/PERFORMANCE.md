# Performance

[Documentation](README.md) · [Architecture](ARCHITECTURE.md)

Reponiq uses the OS webview with a Rust backend. Fonts, icons and frontend assets are bundled locally; no CDN or external font request is needed to run the app.

## Interaction behavior

Changing a file, branch, collection, tab or checkbox updates component state rather than navigating or reloading the document. Keyed file/commit/PR lists preserve item identity. Re-selecting the same commit or file avoids a duplicate request. A Git mutation refreshes only the selected repository and activity; it does not rescan every registered repository. The shelf reads saved metadata first and checks Git status separately. There is no manual refresh button; status updates on focus and during visible idle polling. Pinning and organization update only the affected repository. PR and history views reject stale asynchronous results.

A Windows browser fixture staging check observed the same application-shell node and one unchanged navigation entry, with no list_repos or refresh_repos call. The fixture interaction completed in 24 ms; this measures UI/mock transport only, not real Git latency. Native disposable-repository checks separately verify real staging and split commits.

## Work limits

The initial shelf query reads SQLite metadata without invoking Git. At most two frontend status requests run together, leaving capacity for interactive operations. Native work admits up to 32 pending requests and executes at most four jobs at once; ordinary bursts wait asynchronously rather than immediately failing as busy. Git mutations retain their operation lock; independent Git reads can run within the worker limit. Git output is capped at 4 MiB with local/network deadlines. File diffs display at most 1,500 lines / 200,000 characters, and changed files render at most 200 filtered matches. Repositories page at 50. History/PR limits are documented in the user guide.

Refreshes are coalesced; inactive or hidden work is deferred. Timed local refresh pauses during dialogs, mutations and hidden windows. GitHub monitoring is manual. The contextual tour disables background animation and uses no blur; reduced motion disables interface animation globally.

A local warm Windows native check against 48 registered paths returned shelf metadata in 4 ms and persisted a pin in 9 ms while status checks ran. Twelve concurrent metadata requests completed without busy errors. A browser fixture with four unrelated 2.5-second requests still pending displayed cards in 191 ms; pinning used only pin_repo and retained the same application-shell node. These are single-run local/mock observations, not cold-start or field percentiles. These are local build observations, not a field latency guarantee. Cold/warm startup, large-workspace latency, idle CPU and memory vary by system and have not been comprehensively benchmarked. No universal fastest-app or zero-delay claim is made.
