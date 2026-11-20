use super::{
    background, branch_name, checked_repo_path, err, full_name, gh_client, git, gitflow, oauth,
    State,
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
    #[serde(default, rename(deserialize = "html_url", serialize = "web_url"))]
    web_url: Option<String>,
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
    id: u64,
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
    repository: String,
    repository_id: String,
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
        #[cfg(test)]
        if error
            .url()
            .is_some_and(|url| url.host_str() == Some("127.0.0.1"))
        {
            return format!("GitHub fixture transport failed: {error:?}");
        }
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

#[derive(Deserialize)]
struct PublishedBranch {
    commit: PublishedCommit,
}

#[derive(Deserialize, Serialize)]
struct PublishedCommit {
    sha: String,
}

#[derive(Deserialize, Serialize)]
pub struct PullComparison {
    base_commit: PublishedCommit,
    ahead_by: u64,
    behind_by: u64,
    files: Vec<ComparisonFile>,
}

#[derive(Deserialize, Serialize)]
struct ComparisonFile {
    filename: String,
    status: String,
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
    async fn published_tip(&self, branch: &str) -> Result<String, String> {
        let encoded = url::form_urlencoded::byte_serialize(branch.as_bytes()).collect::<String>();
        let branch: PublishedBranch = self.get(&format!("/branches/{encoded}")).await?;
        hash_valid(&branch.commit.sha)?;
        Ok(branch.commit.sha)
    }
    async fn published_branch(&self, head: &str, expected: &str) -> Result<(), String> {
        if self.published_tip(head).await? != expected {
            return Err("The published branch changed. Fetch and review it before creating the pull request.".into());
        }
        Ok(())
    }
    async fn create_from_comparison(&self, input: &CreatePull) -> Result<CreatedPull, String> {
        self.published_branch(&input.head, &input.expected_head)
            .await?;
        if self.published_tip(&input.base).await? != input.expected_base {
            return Err("The base branch changed. Compare the branches again before creating the pull request.".into());
        }
        let (pull, _): (Pull, _) = self.request(
            reqwest::Method::POST, "/pulls",
            Some(json!({"title":input.title.trim(),"body":input.body,"base":input.base,"head":input.head,"draft":input.draft})),
        ).await?;
        let matches_comparison =
            pull.head.sha == input.expected_head && pull.base.sha == input.expected_base;
        Ok(CreatedPull {
            pull,
            matches_comparison,
        })
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
            repository: self.full.clone(),
            repository_id: repo.id.to_string(),
            pull,
            reviews: reviews.0,
            reviews_complete: !reviews.1,
            checks,
            merge_methods: methods(&repo),
        })
    }
    async fn merge(
        &self,
        number: u64,
        head: &str,
        base: &str,
        method: &str,
        repository_id: &str,
    ) -> Result<String, String> {
        let details = self.details(number).await?;
        verify_reviewed_repository(&details.repository_id, repository_id)?;
        verify_reviewed_base(&details.pull, base)?;
        merge_allowed(&details, head, method)?;
        let current = self.pull(number).await?;
        verify_reviewed_base(&current, base)?;
        current_pull(&current, head)?;
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
    scoped_api(id, account_id, state, None).await
}

struct PullScope {
    target: super::workflow::Target,
    account_login: String,
    repository: Option<String>,
}

async fn scoped_api(
    id: i64,
    account_id: i64,
    state: tauri::State<'_, State>,
    expected: Option<PullScope>,
) -> Result<Api, String> {
    let full = background(state.clone(), move |s| {
        let (path, host) = {
            let db = s.database.lock().map_err(err)?;
            let path = if let Some(scope) = &expected {
                scope.target.verified_path(&db)?
            } else {
                checked_repo_path(&db, id)?
            };
            let (host, login): (String, String) = db
                .query_row(
                    "SELECT api_host,login FROM accounts WHERE id=?1",
                    [account_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(|_| "Select a connected GitHub account.".to_string())?;
            if let Some(scope) = &expected {
                if !login.eq_ignore_ascii_case(&scope.account_login) {
                    return Err("The GitHub account changed. Reopen the pull request.".into());
                }
            }
            (path, host)
        };
        super::validate_host(&host)?;
        let remote = git(&path, &["remote", "get-url", "origin"])?;
        let full = full_name(&remote)
            .ok_or("In-app pull requests currently support GitHub.com origin remotes.")?;
        if let Some(repository) = expected
            .as_ref()
            .and_then(|scope| scope.repository.as_deref())
        {
            if !full.eq_ignore_ascii_case(repository) {
                return Err("The GitHub origin changed. Reopen the pull request.".into());
            }
        }
        Ok(full)
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
    let _guard = oauth::lock_account().await?;
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
    let _guard = oauth::lock_account().await?;
    let api = api(id, account_id, state.clone()).await?;
    let (items, has_more) = api
        .request(
            reqwest::Method::GET,
            &format!("/pulls?state=open&sort=updated&per_page=25&page={page}"),
            None,
        )
        .await?;
    let _ = background(state, move |s| {
        let db = s.database.lock().map_err(err)?;
        let login: String = db
            .query_row(
                "SELECT login FROM accounts WHERE id=?1",
                [account_id],
                |row| row.get(0),
            )
            .map_err(err)?;
        super::workflow::used_pr_account(&db, id, &login)
    })
    .await;
    Ok(Page { items, has_more })
}

fn check_link(full: &str, number: u64, candidate: Option<&str>) -> String {
    let root = format!("/{full}/");
    let safe = candidate
        .and_then(|raw| url::Url::parse(raw).ok())
        .filter(|url| {
            url.scheme() == "https"
                && url.host_str() == Some("github.com")
                && url.port().is_none()
                && url.username().is_empty()
                && url.password().is_none()
                && url
                    .path()
                    .get(..root.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&root))
                && url.path().get(root.len()..).is_some_and(|suffix| {
                    ["actions/runs/", "runs/"].iter().any(|prefix| {
                        suffix.strip_prefix(prefix).is_some_and(|tail| {
                            tail.split('/').next().is_some_and(|id| {
                                !id.is_empty() && id.bytes().all(|ch| ch.is_ascii_digit())
                            }) && tail.split('/').all(|part| {
                                !part.is_empty()
                                    && part
                                        .bytes()
                                        .all(|ch| ch.is_ascii_alphanumeric() || ch == b'-')
                            })
                        })
                    })
                })
        });
    safe.map(String::from)
        .unwrap_or_else(|| format!("https://github.com/{full}/pull/{number}/checks"))
}

#[tauri::command]
pub async fn open_pull_check(
    id: i64,
    number: u64,
    web_url: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
) -> Result<(), String> {
    number_valid(number)?;
    background(state, move |s| {
        use tauri_plugin_opener::OpenerExt;
        let path = checked_repo_path(&*s.database.lock().map_err(err)?, id)?;
        let remote = git(&path, &["remote", "get-url", "origin"])?;
        let full = full_name(&remote).ok_or("This repository has no GitHub checks page.")?;
        app.opener()
            .open_url(
                check_link(&full, number, web_url.as_deref()),
                None::<String>,
            )
            .map_err(|_| "Could not open the GitHub checks page.".into())
    })
    .await
}
#[tauri::command]
pub async fn pull_request_details(
    target: super::workflow::Target,
    account_id: i64,
    account_login: String,
    number: u64,
    state: tauri::State<'_, State>,
) -> Result<PullDetails, String> {
    let _guard = oauth::lock_account().await?;
    let scope = PullScope {
        target,
        account_login,
        repository: None,
    };
    scoped_api(scope.target.id, account_id, state, Some(scope))
        .await?
        .details(number)
        .await
}
#[tauri::command]
pub async fn pull_request_files(
    review: ReviewedPull,
    page: u32,
    commit: Option<String>,
    state: tauri::State<'_, State>,
) -> Result<Page<PullFile>, String> {
    let number = review.number;
    if !(1..=30).contains(&page) {
        return Err("File paging limit reached. Use GitHub for the remaining files.".into());
    }
    let _guard = oauth::lock_account().await?;
    let api = review.api(state).await?;
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
    review: ReviewedPull,
    state: tauri::State<'_, State>,
) -> Result<Page<PullCommit>, String> {
    let number = review.number;
    let _guard = oauth::lock_account().await?;
    let api = review.api(state).await?;
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
    expected_base: String,
    draft: bool,
}
#[tauri::command]
pub async fn compare_pull_branches(
    id: i64,
    account_id: i64,
    base: String,
    head: String,
    expected_head: String,
    state: tauri::State<'_, State>,
) -> Result<PullComparison, String> {
    branch_name(&base)?;
    branch_name(&head)?;
    hash_valid(&expected_head)?;
    if base == head {
        return Err("Choose different base and head branches.".into());
    }
    let _guard = oauth::lock_account().await?;
    let api = api(id, account_id, state).await?;
    api.published_branch(&head, &expected_head).await?;
    let encoded_base = url::form_urlencoded::byte_serialize(base.as_bytes()).collect::<String>();
    let comparison: PullComparison = api
        .get(&format!(
            "/compare/{encoded_base}...{expected_head}?page=1&per_page=1"
        ))
        .await?;
    hash_valid(&comparison.base_commit.sha)?;
    Ok(comparison)
}

#[tauri::command]
pub async fn create_pull_request(
    id: i64,
    account_id: i64,
    input: CreatePull,
    state: tauri::State<'_, State>,
) -> Result<CreatedPull, String> {
    let _update = super::updater::workspace_job()?;
    if input.title.trim().is_empty()
        || input.title.encode_utf16().count() > 256
        || input.body.encode_utf16().count() > 20000
    {
        return Err(
            "Enter a title under 256 characters and a description under 20,000 characters.".into(),
        );
    }
    branch_name(&input.base)?;
    branch_name(&input.head)?;
    hash_valid(&input.expected_head)?;
    hash_valid(&input.expected_base)?;
    if input.base == input.head {
        return Err("Choose different base and head branches.".into());
    }
    let _guard = oauth::lock_account().await?;
    let api = api(id, account_id, state.clone()).await?;
    let expected_branch = input.head.clone();
    let expected_head = input.expected_head.clone();
    background(state, move |s| {
        let _guard = gitflow::MUTATIONS.lock().map_err(err)?;
        let path = checked_repo_path(&*s.database.lock().map_err(err)?, id)?;
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
    api.create_from_comparison(&input).await
}

#[derive(Serialize)]
pub struct CreatedPull {
    pull: Pull,
    matches_comparison: bool,
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
    let _update = super::updater::workspace_job()?;
    review_body(&event, &body)?;
    let _guard = oauth::lock_account().await?;
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
fn verify_reviewed_repository(actual: &str, expected: &str) -> Result<(), String> {
    if actual == "0" || actual != expected {
        return Err("The GitHub repository changed. Reopen the pull request.".into());
    }
    Ok(())
}

fn verify_reviewed_base(pull: &Pull, base: &str) -> Result<(), String> {
    if pull.base.name != base {
        return Err("The destination branch changed. Reopen the pull request.".into());
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedPull {
    target: super::workflow::Target,
    account_id: i64,
    account_login: String,
    repository: String,
    repository_id: String,
    number: u64,
    head: String,
    base: String,
}

impl ReviewedPull {
    async fn scoped_api(&self, state: tauri::State<'_, State>) -> Result<Api, String> {
        number_valid(self.number)?;
        let scope = PullScope {
            target: self.target.clone(),
            account_login: self.account_login.clone(),
            repository: Some(self.repository.clone()),
        };
        scoped_api(self.target.id, self.account_id, state, Some(scope)).await
    }

    async fn api(&self, state: tauri::State<'_, State>) -> Result<Api, String> {
        let api = self.scoped_api(state).await?;
        let (repo, pull) = tokio::try_join!(api.get::<Repository>(""), api.pull(self.number))?;
        verify_reviewed_repository(&repo.id.to_string(), &self.repository_id)?;
        verify_reviewed_base(&pull, &self.base)?;
        if pull.head.sha != self.head {
            return Err("New commits were added. Reopen the pull request.".into());
        }
        Ok(api)
    }
}

#[tauri::command]
pub async fn merge_pull_request(
    review: ReviewedPull,
    method: String,
    state: tauri::State<'_, State>,
) -> Result<String, String> {
    let _update = super::updater::workspace_job()?;
    let _guard = oauth::lock_account().await?;
    if review.target.registration.is_none() {
        return Err("Refresh the repository before merging.".into());
    }
    let api = review.scoped_api(state).await?;
    api.merge(
        review.number,
        &review.head,
        &review.base,
        &method,
        &review.repository_id,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_check_links_stay_on_the_repository_and_github() {
        let fallback = "https://github.com/example/app/pull/7/checks";
        let run = "https://github.com/example/app/actions/runs/123/job/456";
        assert_eq!(check_link("example/app", 7, Some(run)), run);
        for url in [
            "https://github.com/lookalike/app/actions/runs/123",
            "https://example.test/example/app/actions/runs/123",
            "https://github.com/example/app/issues/123",
            "http://github.com/example/app/actions/runs/123",
        ] {
            assert_eq!(check_link("example/app", 7, Some(url)), fallback);
        }
    }
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
                        // Windows accept can inherit the listener's nonblocking
                        // mode. Wait for the request instead of closing on WouldBlock.
                        stream.set_nonblocking(false).unwrap();
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
            json!({"id":42,"full_name":"example/repo","default_branch":"main","allow_merge_commit":true,"allow_squash_merge":false,"allow_rebase_merge":false})
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
            .merge(1, &"b".repeat(40), "main", "merge", "42")
            .await
            .unwrap_err();
        assert!(error.contains("New commits"), "{error}");
        assert_eq!(puts.load(Ordering::Acquire), 0);
    }
    #[tokio::test]
    async fn merge_requires_the_reviewed_provider_repository() {
        let _fixture = TRANSPORT_FIXTURE.lock().await;
        let puts = Arc::new(AtomicUsize::new(0));
        let counter = puts.clone();
        let server = server(move |h, b| reply(h, b, &counter));
        let api = client(&server);
        let reviewed = api.details(1).await.unwrap();
        assert_eq!(reviewed.repository, "example/repo");
        assert_eq!(reviewed.repository_id, "42");
        let error = api
            .merge(1, &reviewed.pull.head.sha, "main", "merge", "43")
            .await
            .unwrap_err();
        assert!(error.contains("repository changed"), "{error}");
        assert_eq!(puts.load(Ordering::Acquire), 0);
    }
    #[tokio::test]
    async fn merge_rejects_a_retargeted_pull_with_the_same_head() {
        let _fixture = TRANSPORT_FIXTURE.lock().await;
        let puts = Arc::new(AtomicUsize::new(0));
        let pulls = AtomicUsize::new(0);
        let counter = puts.clone();
        let server = server(move |headers, body| {
            if headers
                .lines()
                .next()
                .unwrap_or_default()
                .contains("/pulls/1 ")
                && pulls.fetch_add(1, Ordering::AcqRel) > 0
            {
                let mut retargeted = details().pull;
                retargeted.base.name = "release".into();
                return (200, serde_json::to_vec(&retargeted).unwrap(), String::new());
            }
            reply(headers, body, &counter)
        });
        let error = client(&server)
            .merge(1, &"a".repeat(40), "main", "merge", "42")
            .await
            .unwrap_err();
        assert!(error.contains("destination branch changed"), "{error}");
        assert_eq!(puts.load(Ordering::Acquire), 0);
    }
    #[tokio::test]
    async fn merge_rechecks_provider_state_and_sends_the_exact_reviewed_sha() {
        let _fixture = TRANSPORT_FIXTURE.lock().await;
        let puts = Arc::new(AtomicUsize::new(0));
        let counter = puts.clone();
        let server = server(move |h, b| reply(h, b, &counter));
        assert!(client(&server)
            .merge(1, &"a".repeat(40), "main", "merge", "42")
            .await
            .unwrap()
            .contains("Merged pull request #1"));
        assert_eq!(puts.load(Ordering::Acquire), 1);
    }
    #[tokio::test]
    async fn moved_comparison_base_never_reaches_the_create_endpoint() {
        let _fixture = TRANSPORT_FIXTURE.lock().await;
        let requests = Arc::new(AtomicUsize::new(0));
        let count = requests.clone();
        let server = server(move |headers, _| {
            let route = headers.lines().next().unwrap();
            if route.starts_with("POST ") {
                count.fetch_add(1, Ordering::AcqRel);
            }
            let head = if route.contains("/branches/topic ") {
                "a"
            } else {
                "b"
            };
            (
                200,
                serde_json::to_vec(&json!({"commit":{"sha":head.repeat(40)}})).unwrap(),
                String::new(),
            )
        });
        let input = CreatePull {
            title: "Update".into(),
            body: String::new(),
            base: "main".into(),
            head: "topic".into(),
            expected_head: "a".repeat(40),
            expected_base: "c".repeat(40),
            draft: false,
        };
        match client(&server).create_from_comparison(&input).await {
            Ok(_) => panic!("a moved base must block PR creation"),
            Err(error) => assert!(error.contains("base branch changed"), "{error}"),
        }
        assert_eq!(requests.load(Ordering::Acquire), 0);
    }
    #[tokio::test]
    async fn created_pull_reports_if_github_received_a_different_commit() {
        let _fixture = TRANSPORT_FIXTURE.lock().await;
        let server = server(|headers, _| {
            let value = if headers.starts_with("POST ") {
                let mut pull = details().pull;
                pull.head.sha = "b".repeat(40);
                serde_json::to_value(pull).unwrap()
            } else {
                json!({"commit":{"sha":"a".repeat(40)}})
            };
            (200, serde_json::to_vec(&value).unwrap(), String::new())
        });
        let input = CreatePull {
            title: "Update".into(),
            body: String::new(),
            base: "main".into(),
            head: "topic".into(),
            expected_head: "a".repeat(40),
            expected_base: "a".repeat(40),
            draft: false,
        };
        let created = client(&server)
            .create_from_comparison(&input)
            .await
            .unwrap();
        assert_eq!(created.pull.number, 1);
        assert!(!created.matches_comparison);
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
            repository: "example/repo".into(),
            repository_id: "42".into(),
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
