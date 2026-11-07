use super::{
    background, branch_name, err, full_name, gh_client, git, gitflow, oauth, repo_path, State,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    login: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct PullRef {
    label: String,
    sha: String,
    #[serde(rename = "ref")]
    name: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Pull {
    number: u64,
    title: String,
    body: Option<String>,
    state: String,
    draft: bool,
    merged: Option<bool>,
    mergeable: Option<bool>,
    mergeable_state: Option<String>,
    user: User,
    head: PullRef,
    base: PullRef,
}
#[derive(Serialize, Deserialize)]
pub struct Review {
    id: u64,
    user: User,
    state: String,
    body: Option<String>,
    commit_id: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct PullFile {
    filename: String,
    status: String,
    additions: u64,
    deletions: u64,
    patch: Option<String>,
    previous_filename: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Check {
    name: String,
    status: String,
    conclusion: Option<String>,
}
#[derive(Deserialize)]
struct CheckRuns {
    total_count: usize,
    check_runs: Vec<Check>,
}
#[derive(Deserialize)]
struct Statuses {
    state: String,
    total_count: usize,
}
#[derive(Serialize)]
pub struct Checks {
    runs: Vec<Check>,
    legacy_state: String,
    complete: bool,
    ready: bool,
}
#[derive(Deserialize)]
struct Repository {
    full_name: String,
    default_branch: String,
    allow_merge_commit: Option<bool>,
    allow_squash_merge: Option<bool>,
    allow_rebase_merge: Option<bool>,
}
#[derive(Serialize)]
pub struct PullContext {
    full_name: String,
    default_branch: String,
    branches: Vec<String>,
    merge_methods: Vec<String>,
}
#[derive(Serialize)]
pub struct Page<T> {
    items: Vec<T>,
    has_more: bool,
}
#[derive(Serialize)]
pub struct PullDetails {
    pull: Pull,
    reviews: Vec<Review>,
    reviews_complete: bool,
    checks: Checks,
    merge_methods: Vec<String>,
}
#[derive(Deserialize)]
struct Branch {
    name: String,
}
#[derive(Serialize, Deserialize)]
pub struct PullCommit {
    sha: String,
    commit: CommitInfo,
}
#[derive(Serialize, Deserialize)]
struct CommitInfo {
    message: String,
}
#[derive(Deserialize)]
struct CommitFiles {
    files: Vec<PullFile>,
}

pub(super) async fn json_response<T: DeserializeOwned>(
    request: reqwest::RequestBuilder,
) -> Result<(T, bool), String> {
    let mut response = request.send().await.map_err(|error| {
        log::error!(target: "reponiq::github", "Request failed: {error}");
        "Could not reach GitHub. Check your connection and refresh before retrying an action."
            .to_string()
    })?;
    log::debug!(target: "reponiq::github", "HTTP {} · {}", response.status(), response.url());
    if !response.status().is_success() {
        return Err(match response.status().as_u16(){401=>"GitHub sign-in expired. Reconnect this account.",403=>"GitHub denied this action. Check token permissions, organization policy, branch protection, or API rate limits.",404=>"GitHub could not find this repository or pull request for the selected account.",405=>"GitHub cannot merge this pull request in its current state.",409=>"The pull request changed or has conflicts. Refresh and review it again.",422=>"GitHub rejected this action. Check the branch, review permissions, and whether a pull request already exists.",429=>"GitHub rate limit reached. Wait before trying again.",_=>"GitHub could not complete the action. Refresh before retrying."}.into());
    }
    let more = response
        .headers()
        .get("link")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.contains("rel=\"next\""));
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Could not read GitHub's response.".to_string())?
    {
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err("GitHub's response is too large to display. Narrow the request or use GitHub's website.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((
        serde_json::from_slice(&bytes).map_err(|_| {
            "GitHub returned an unexpected response. Refresh and try again.".to_string()
        })?,
        more,
    ))
}

struct Api {
    client: reqwest::Client,
    token: String,
    full: String,
    #[cfg(test)]
    test_base: Option<String>,
}
impl Api {
    async fn request<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        route: &str,
        body: Option<serde_json::Value>,
    ) -> Result<(T, bool), String> {
        let url = format!("https://api.github.com/repos/{}{route}", self.full);
        #[cfg(test)]
        let url = self
            .test_base
            .as_ref()
            .map(|base| format!("{base}{route}"))
            .unwrap_or(url);
        let mut request = self
            .client
            .request(method, url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .bearer_auth(&self.token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        json_response(request).await
    }
    async fn get<T: DeserializeOwned>(&self, route: &str) -> Result<T, String> {
        self.request(reqwest::Method::GET, route, None)
            .await
            .map(|r| r.0)
    }
    async fn pull(&self, number: u64) -> Result<Pull, String> {
        number_valid(number)?;
        self.get(&format!("/pulls/{number}")).await
    }
    async fn checks(&self, sha: &str) -> Result<Checks, String> {
        hash_valid(sha)?;
        let runs_route = format!("/commits/{sha}/check-runs?per_page=100");
        let status_route = format!("/commits/{sha}/status?per_page=100");
        let (runs, status) = tokio::try_join!(
            self.request::<CheckRuns>(reqwest::Method::GET, &runs_route, None),
            self.request::<Statuses>(reqwest::Method::GET, &status_route, None)
        )?;
        let complete = !runs.1
            && !status.1
            && runs.0.total_count <= runs.0.check_runs.len()
            && status.0.total_count <= 100;
        let ready = complete
            && runs.0.check_runs.iter().all(|c| {
                c.status == "completed"
                    && matches!(
                        c.conclusion.as_deref(),
                        Some("success" | "neutral" | "skipped")
                    )
            })
            && (status.0.total_count == 0 || status.0.state == "success");
        Ok(Checks {
            runs: runs.0.check_runs,
            legacy_state: if status.0.total_count == 0 {
                "none".into()
            } else {
                status.0.state
            },
            complete,
            ready,
        })
    }
    async fn details(&self, number: u64) -> Result<PullDetails, String> {
        let pull = self.pull(number).await?;
        let reviews_route = format!("/pulls/{number}/reviews?per_page=100");
        let (reviews, checks, repo) = tokio::try_join!(
            self.request::<Vec<Review>>(reqwest::Method::GET, &reviews_route, None),
            self.checks(&pull.head.sha),
            self.get::<Repository>("")
        )?;
        Ok(PullDetails {
            pull,
            reviews: reviews.0,
            reviews_complete: !reviews.1,
            checks,
            merge_methods: methods(&repo),
        })
    }
    async fn merge(&self, number: u64, head: &str, method: &str) -> Result<String, String> {
        let details = self.details(number).await?;
        merge_allowed(&details, head, method)?;
        #[derive(Deserialize)]
        struct ResultBody {
            merged: bool,
            sha: String,
        }
        let (result, _) = self
            .request::<ResultBody>(
                reqwest::Method::PUT,
                &format!("/pulls/{number}/merge"),
                Some(json!({"sha":head,"merge_method":method})),
            )
            .await?;
        if !result.merged {
            return Err(
                "GitHub did not merge this pull request. Refresh its status before retrying."
                    .into(),
            );
        }
        Ok(format!("Merged pull request #{number} at {}.", result.sha))
    }
}
fn hash_valid(hash: &str) -> Result<(), String> {
    if matches!(hash.len(), 40 | 64) && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("The pull request has an invalid commit identity. Refresh it.".into())
    }
}
fn number_valid(number: u64) -> Result<(), String> {
    if number == 0 || number > 2_147_483_647 {
        Err("Invalid pull request number.".into())
    } else {
        Ok(())
    }
}
fn methods(repo: &Repository) -> Vec<String> {
    [
        ("merge", repo.allow_merge_commit),
        ("squash", repo.allow_squash_merge),
        ("rebase", repo.allow_rebase_merge),
    ]
    .into_iter()
    .filter(|(_, allowed)| *allowed == Some(true))
    .map(|(method, _)| method.into())
    .collect()
}
async fn api(id: i64, account_id: i64, state: tauri::State<'_, State>) -> Result<Api, String> {
    let full = background(state.clone(), move |s| {
        let path = repo_path(&*s.database.lock().map_err(err)?, id)?;
        let host: String = s
            .database
            .lock()
            .map_err(err)?
            .query_row(
                "SELECT api_host FROM accounts WHERE id=?1",
                [account_id],
                |r| r.get(0),
            )
            .map_err(|_| "Select a connected GitHub account.".to_string())?;
        super::validate_host(&host)?;
        let remote = git(&path, &["remote", "get-url", "origin"])?;
        full_name(&remote)
            .ok_or("In-app pull requests currently support GitHub.com origin remotes.".into())
    })
    .await?;
    let token = oauth::account_token(account_id, state).await?;
    Ok(Api {
        client: gh_client()?,
        token,
        full,
        #[cfg(test)]
        test_base: None,
    })
}

#[tauri::command]
pub async fn pull_context(
    id: i64,
    account_id: i64,
    state: tauri::State<'_, State>,
) -> Result<PullContext, String> {
    let _guard = oauth::ACCOUNT_LOCK.lock().await;
    let api = api(id, account_id, state).await?;
    let (repo, branches) = tokio::try_join!(
        api.get::<Repository>(""),
        api.get::<Vec<Branch>>("/branches?per_page=100")
    )?;
    Ok(PullContext {
        full_name: repo.full_name.clone(),
        default_branch: repo.default_branch.clone(),
        branches: branches.into_iter().map(|b| b.name).collect(),
        merge_methods: methods(&repo),
    })
}
#[tauri::command]
pub async fn list_pull_requests(
    id: i64,
    account_id: i64,
    page: u32,
    state: tauri::State<'_, State>,
) -> Result<Page<Pull>, String> {
    if !(1..=400).contains(&page) {
        return Err("Pull-request paging limit reached.".into());
    }
    let _guard = oauth::ACCOUNT_LOCK.lock().await;
    let api = api(id, account_id, state).await?;
    let (items, has_more) = api
        .request(
            reqwest::Method::GET,
            &format!("/pulls?state=open&sort=updated&per_page=25&page={page}"),
            None,
        )
        .await?;
    Ok(Page { items, has_more })
}
#[tauri::command]
pub async fn pull_request_details(
    id: i64,
    account_id: i64,
    number: u64,
    state: tauri::State<'_, State>,
) -> Result<PullDetails, String> {
    let _guard = oauth::ACCOUNT_LOCK.lock().await;
    api(id, account_id, state).await?.details(number).await
}
#[tauri::command]
pub async fn pull_request_files(
    id: i64,
    account_id: i64,
    number: u64,
    page: u32,
    commit: Option<String>,
    state: tauri::State<'_, State>,
) -> Result<Page<PullFile>, String> {
    number_valid(number)?;
    if !(1..=30).contains(&page) {
        return Err("File paging limit reached. Use GitHub for the remaining files.".into());
    }
    let _guard = oauth::ACCOUNT_LOCK.lock().await;
    let api = api(id, account_id, state).await?;
    let (items, has_more) = if let Some(sha) = commit {
        hash_valid(&sha)?;
        // A commit must belong to this PR before it can be inspected through this surface.
        let (commits, _) = api
            .request::<Vec<PullCommit>>(
                reqwest::Method::GET,
                &format!("/pulls/{number}/commits?per_page=100"),
                None,
            )
            .await?;
        if !commits.iter().any(|c| c.sha == sha) {
            return Err("Choose a commit in the first 100 commits of this pull request.".into());
        }
        let (result, more) = api
            .request::<CommitFiles>(
                reqwest::Method::GET,
                &format!("/commits/{sha}?per_page=100&page={page}"),
                None,
            )
            .await?;
        (result.files, more)
    } else {
        api.request(
            reqwest::Method::GET,
            &format!("/pulls/{number}/files?per_page=100&page={page}"),
            None,
        )
        .await?
    };
    Ok(Page { items, has_more })
}
#[tauri::command]
pub async fn pull_request_commits(
    id: i64,
    account_id: i64,
    number: u64,
    state: tauri::State<'_, State>,
) -> Result<Page<PullCommit>, String> {
    number_valid(number)?;
    let _guard = oauth::ACCOUNT_LOCK.lock().await;
    let api = api(id, account_id, state).await?;
    let (items, has_more) = api
        .request(
            reqwest::Method::GET,
            &format!("/pulls/{number}/commits?per_page=100"),
            None,
        )
        .await?;
    Ok(Page { items, has_more })
}

#[derive(Deserialize)]
pub struct CreatePull {
    title: String,
    body: String,
    base: String,
    head: String,
    expected_head: String,
    draft: bool,
}
#[tauri::command]
pub async fn create_pull_request(
    id: i64,
    account_id: i64,
    input: CreatePull,
    state: tauri::State<'_, State>,
) -> Result<Pull, String> {
    if input.title.trim().is_empty() || input.title.len() > 256 || input.body.len() > 20000 {
        return Err(
            "Enter a title under 256 characters and a description under 20,000 characters.".into(),
        );
    }
    branch_name(&input.base)?;
    branch_name(&input.head)?;
    hash_valid(&input.expected_head)?;
    if input.base == input.head {
        return Err("Choose different base and head branches.".into());
    }
    let _guard = oauth::ACCOUNT_LOCK.lock().await;
    let api = api(id, account_id, state.clone()).await?;
    let expected_branch = input.head.clone();
    let expected_head = input.expected_head.clone();
    background(state, move |s| {
        let _guard = gitflow::MUTATIONS.lock().map_err(err)?;
        let path = repo_path(&*s.database.lock().map_err(err)?, id)?;
        let current = gitflow::state(&path)?;
        gitflow::require_clean(&current)?;
        if current.branch != expected_branch
            || current.head.as_deref() != Some(expected_head.as_str())
            || current.upstream.is_none()
            || current.ahead > 0
        {
            return Err(
                "Publish the latest commit on this branch before creating a pull request.".into(),
            );
        }
        Ok(())
    })
    .await?;
    #[derive(Deserialize)]
    struct PublishedBranch {
        commit: PublishedCommit,
    }
    #[derive(Deserialize)]
    struct PublishedCommit {
        sha: String,
    }
    let encoded = url::form_urlencoded::byte_serialize(input.head.as_bytes()).collect::<String>();
    let branch: PublishedBranch = api.get(&format!("/branches/{encoded}")).await?;
    if branch.commit.sha != input.expected_head {
        return Err(
            "The published branch changed. Fetch and review it before creating the pull request."
                .into(),
        );
    }
    // GitHub also checks that the selected branch is published; no local changes are uploaded by this API.
    api.request(reqwest::Method::POST,"/pulls",Some(json!({"title":input.title.trim(),"body":input.body,"base":input.base,"head":input.head,"draft":input.draft}))).await.map(|r|r.0)
}
fn review_body(event: &str, body: &str) -> Result<(), String> {
    if !matches!(event, "APPROVE" | "REQUEST_CHANGES" | "COMMENT")
        || body.len() > 20000
        || (event != "APPROVE" && body.trim().is_empty())
    {
        Err(
            "Choose a review action and include a comment when requesting changes or commenting."
                .into(),
        )
    } else {
        Ok(())
    }
}
fn current_pull(pull: &Pull, head: &str) -> Result<(), String> {
    hash_valid(head)?;
    if pull.state != "open" || pull.merged == Some(true) {
        return Err("This pull request is no longer open. Refresh it.".into());
    }
    if pull.head.sha != head {
        return Err("New commits were added. Refresh and review the updated pull request before continuing.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn review_pull_request(
    id: i64,
    account_id: i64,
    number: u64,
    head: String,
    event: String,
    body: String,
    state: tauri::State<'_, State>,
) -> Result<Review, String> {
    review_body(&event, &body)?;
    let _guard = oauth::ACCOUNT_LOCK.lock().await;
    let api = api(id, account_id, state).await?;
    current_pull(&api.pull(number).await?, &head)?;
    api.request(
        reqwest::Method::POST,
        &format!("/pulls/{number}/reviews"),
        Some(json!({"commit_id":head,"event":event,"body":body})),
    )
    .await
    .map(|r| r.0)
}
fn merge_allowed(details: &PullDetails, head: &str, method: &str) -> Result<(), String> {
    current_pull(&details.pull, head)?;
    if details.pull.draft
        || details.pull.mergeable != Some(true)
        || !matches!(
            details.pull.mergeable_state.as_deref(),
            Some("clean" | "has_hooks")
        )
    {
        return Err("This pull request is draft, conflicted, still being checked, or blocked by repository policy. Refresh its status.".into());
    }
    if !details.checks.complete || !details.checks.ready || !details.reviews_complete {
        return Err(
            "Checks or reviews are failing, pending, or incomplete. Resolve them before merging."
                .into(),
        );
    }
    let mut latest = std::collections::HashMap::new();
    for review in &details.reviews {
        if matches!(
            review.state.as_str(),
            "APPROVED" | "CHANGES_REQUESTED" | "DISMISSED"
        ) {
            latest.insert(&review.user.login, &review.state);
        }
    }
    if latest.values().any(|s| s.as_str() == "CHANGES_REQUESTED") {
        return Err("A reviewer requested changes. Resolve the review before merging.".into());
    }
    if !details.merge_methods.iter().any(|m| m == method) {
        return Err("This merge method is not enabled for the repository.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn merge_pull_request(
    id: i64,
    account_id: i64,
    number: u64,
    head: String,
    method: String,
    state: tauri::State<'_, State>,
) -> Result<String, String> {
    let _guard = oauth::ACCOUNT_LOCK.lock().await;
    let api = api(id, account_id, state).await?;
    api.merge(number, &head, &method).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        thread,
        time::Duration,
    };
    // Windows host network inspection can abort concurrent loopback fixtures.
    static TRANSPORT_FIXTURE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    struct Server {
        base: String,
        stop: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
        }
    }
    fn server(handler: impl Fn(&str, &[u8]) -> (u16, Vec<u8>, String) + Send + 'static) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let thread = thread::spawn(move || {
            while !signal.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut bytes = Vec::new();
                        let mut buffer = [0u8; 2048];
                        let header_end = loop {
                            match stream.read(&mut buffer) {
                                Ok(0) | Err(_) => break None,
                                Ok(n) => {
                                    bytes.extend_from_slice(&buffer[..n]);
                                    if bytes.len() > 65536 {
                                        break None;
                                    }
                                    if let Some(index) =
                                        bytes.windows(4).position(|p| p == b"\r\n\r\n")
                                    {
                                        break Some(index + 4);
                                    }
                                }
                            }
                        };
                        if let Some(end) = header_end {
                            let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
                            let length = headers
                                .lines()
                                .find_map(|line| {
                                    line.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .and_then(|s| s.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if length > 65536 {
                                continue;
                            }
                            while bytes.len() - end < length {
                                match stream.read(&mut buffer) {
                                    Ok(0) | Err(_) => break,
                                    Ok(n) => bytes.extend_from_slice(&buffer[..n]),
                                }
                            }
                            let (code, body, extra) = handler(&headers, &bytes[end..]);
                            let header=format!("HTTP/1.1 {code} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n",body.len());
                            let _ = stream.write_all(header.as_bytes());
                            let _ = stream.write_all(&body);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("fixture accept failed: {e}"),
                }
            }
        });
        Server {
            base,
            stop,
            thread: Some(thread),
        }
    }
    fn client(server: &Server) -> Api {
        Api {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap(),
            token: "fixture-token".into(),
            full: "example/repo".into(),
            test_base: Some(server.base.clone()),
        }
    }
    fn reply(headers: &str, body: &[u8], puts: &AtomicUsize) -> (u16, Vec<u8>, String) {
        assert!(headers
            .to_ascii_lowercase()
            .contains("authorization: bearer fixture-token"));
        let route = headers.lines().next().unwrap();
        let value = if route.starts_with("PUT ") {
            puts.fetch_add(1, Ordering::AcqRel);
            let body: serde_json::Value = serde_json::from_slice(body).unwrap();
            assert_eq!(body["sha"], "a".repeat(40));
            assert_eq!(body["merge_method"], "merge");
            json!({"merged":true,"sha":"c".repeat(40)})
        } else if route.contains("/pulls/1/reviews?") {
            json!([])
        } else if route.contains("/check-runs?") {
            json!({"total_count":0,"check_runs":[]})
        } else if route.contains("/status?") {
            json!({"state":"pending","total_count":0})
        } else if route.contains("/pulls/1 ") {
            serde_json::to_value(details().pull).unwrap()
        } else {
            json!({"full_name":"example/repo","default_branch":"main","allow_merge_commit":true,"allow_squash_merge":false,"allow_rebase_merge":false})
        };
        (200, serde_json::to_vec(&value).unwrap(), String::new())
    }
    #[tokio::test]
    async fn stale_reviewed_head_never_reaches_the_merge_endpoint() {
        let _fixture = TRANSPORT_FIXTURE.lock().await;
        let puts = Arc::new(AtomicUsize::new(0));
        let counter = puts.clone();
        let server = server(move |h, b| reply(h, b, &counter));
        let error = client(&server)
            .merge(1, &"b".repeat(40), "merge")
            .await
            .unwrap_err();
        assert!(error.contains("New commits"), "{error}");
        assert_eq!(puts.load(Ordering::Acquire), 0);
    }
    #[tokio::test]
    async fn merge_rechecks_provider_state_and_sends_the_exact_reviewed_sha() {
        let _fixture = TRANSPORT_FIXTURE.lock().await;
        let puts = Arc::new(AtomicUsize::new(0));
        let counter = puts.clone();
        let server = server(move |h, b| reply(h, b, &counter));
        assert!(client(&server)
            .merge(1, &"a".repeat(40), "merge")
            .await
            .unwrap()
            .contains("Merged pull request #1"));
        assert_eq!(puts.load(Ordering::Acquire), 1);
    }
    #[tokio::test]
    async fn redirects_are_not_followed_and_error_bodies_are_not_exposed() {
        let _fixture = TRANSPORT_FIXTURE.lock().await;
        let server = server(|_, _| {
            (
                302,
                b"private-body-fixture".to_vec(),
                "Location: https://example.invalid/never-follow\r\n".into(),
            )
        });
        let error = client(&server)
            .get::<serde_json::Value>("")
            .await
            .unwrap_err();
        assert!(!error.contains("private-body"));
        assert!(error.contains("could not complete"), "{error}");
    }
    #[tokio::test]
    async fn provider_response_size_is_bounded_before_json_parsing() {
        let _fixture = TRANSPORT_FIXTURE.lock().await;
        let server = server(|_, _| (200, vec![b'a'; 2 * 1024 * 1024 + 1], String::new()));
        let error = client(&server)
            .get::<serde_json::Value>("")
            .await
            .unwrap_err();
        assert!(error.contains("too large"), "{error}");
    }
    fn details() -> PullDetails {
        let sha = "a".repeat(40);
        PullDetails {
            pull: Pull {
                number: 1,
                title: "Fixture".into(),
                body: None,
                state: "open".into(),
                draft: false,
                merged: Some(false),
                mergeable: Some(true),
                mergeable_state: Some("clean".into()),
                user: User {
                    login: "author".into(),
                },
                head: PullRef {
                    label: "head".into(),
                    sha: sha.clone(),
                    name: "feature".into(),
                },
                base: PullRef {
                    label: "base".into(),
                    sha,
                    name: "main".into(),
                },
            },
            reviews: vec![],
            reviews_complete: true,
            checks: Checks {
                runs: vec![],
                legacy_state: "none".into(),
                complete: true,
                ready: true,
            },
            merge_methods: vec!["merge".into()],
        }
    }
    #[test]
    fn merge_requires_the_reviewed_head_known_checks_and_current_policy() {
        let mut d = details();
        let head = "a".repeat(40);
        assert!(merge_allowed(&d, &head, "merge").is_ok());
        assert!(merge_allowed(&d, &"b".repeat(40), "merge").is_err());
        d.checks.ready = false;
        assert!(merge_allowed(&d, &head, "merge").is_err());
        d.checks.ready = true;
        d.pull.mergeable = None;
        assert!(merge_allowed(&d, &head, "merge").is_err());
        d.pull.mergeable = Some(true);
        d.reviews.push(Review {
            id: 1,
            user: User {
                login: "reviewer".into(),
            },
            state: "CHANGES_REQUESTED".into(),
            body: None,
            commit_id: Some(head.clone()),
        });
        assert!(merge_allowed(&d, &head, "merge").is_err());
        d.reviews.push(Review {
            id: 2,
            user: User {
                login: "reviewer".into(),
            },
            state: "APPROVED".into(),
            body: None,
            commit_id: Some(head.clone()),
        });
        assert!(merge_allowed(&d, &head, "merge").is_ok());
        assert!(merge_allowed(&d, &head, "unknown").is_err());
        d.reviews_complete = false;
        assert!(merge_allowed(&d, &head, "merge").is_err());
    }
    #[test]
    fn reviews_reject_invalid_events_empty_change_requests_and_oversize_comments() {
        assert!(review_body("APPROVE", "").is_ok());
        assert!(review_body("REQUEST_CHANGES", "").is_err());
        assert!(review_body("COMMENT", &"a".repeat(20001)).is_err());
        assert!(review_body("DELETE", "").is_err());
    }
}
