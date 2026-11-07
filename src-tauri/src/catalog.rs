use super::{background, err, gh_client, oauth, pulls::json_response, Repo, State};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize, Clone)]
pub struct OnlineRepo {
    id: u64,
    name: String,
    full_name: String,
    private: bool,
}
#[derive(Deserialize, Serialize, Clone)]
pub struct Owner {
    login: String,
    #[serde(rename = "type", default)]
    kind: String,
}
#[derive(Serialize)]
pub struct Page<T> {
    items: Vec<T>,
    has_more: bool,
    warning: Option<String>,
}
#[derive(Serialize)]
pub struct Created {
    repo: Repo,
    warning: Option<String>,
}
struct Api {
    client: reqwest::Client,
    token: String,
    login: String,
}
impl Api {
    async fn request<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        route: &str,
        body: Option<serde_json::Value>,
    ) -> Result<(T, bool), String> {
        let mut request = self
            .client
            .request(method, format!("https://api.github.com{route}"))
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
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
    async fn user(&self) -> Result<Owner, String> {
        let user: Owner = self.get("/user").await?;
        if !user.login.eq_ignore_ascii_case(&self.login) {
            return Err(
                "This credential belongs to another account. Reconnect the selected account."
                    .into(),
            );
        }
        Ok(user)
    }
}
async fn api(id: i64, state: tauri::State<'_, State>) -> Result<Api, String> {
    let login = background(state.clone(), move |s| {
        s.database
            .lock()
            .map_err(err)?
            .query_row(
                "SELECT login FROM accounts WHERE id=?1 AND api_host='https://api.github.com'",
                [id],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| "Select a connected GitHub account.".into())
    })
    .await?;
    Ok(Api {
        client: gh_client()?,
        token: oauth::account_token(id, state).await?,
        login,
    })
}
fn page_valid(page: u32) -> Result<(), String> {
    if (1..=400).contains(&page) {
        Ok(())
    } else {
        Err("Page limit reached. Narrow your selection on GitHub.".into())
    }
}
fn name_valid(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 100
        || matches!(name, "." | "..")
        || name.ends_with('.')
        || name.ends_with(' ')
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    {
        return Err(
            "Use 1–100 letters, numbers, dots, dashes or underscores for the repository name."
                .into(),
        );
    }
    // Windows reserved basenames are rejected on every platform for portable projects.
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err("Choose a repository name that is valid on Windows and Linux.".into());
    }
    Ok(())
}
fn full_valid(full: &str) -> Result<(), String> {
    let (owner, name) = full
        .split_once('/')
        .ok_or("Invalid GitHub repository identity.")?;
    if owner.is_empty()
        || owner.len() > 39
        || !owner
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return Err("Invalid GitHub owner.".into());
    }
    name_valid(name)
}
fn destination(parent: &str, name: &str) -> Result<PathBuf, String> {
    name_valid(name)?;
    let parent = Path::new(parent);
    if !parent.is_absolute() {
        return Err("Choose an absolute parent folder with Browse.".into());
    }
    let parent = std::fs::canonicalize(parent)
        .map_err(|_| "The parent folder is unavailable.".to_string())?;
    if !parent.is_dir() {
        return Err("Choose a parent folder.".into());
    }
    let target = parent.join(name);
    if target.try_exists().map_err(err)? {
        return Err(
            "That folder already exists. Choose a new name; existing files will not be replaced."
                .into(),
        );
    }
    Ok(target)
}
/// Retain every mutable path component on Windows; Linux child cwd is fd-bound.
struct NewDirectory {
    path: PathBuf,
    guards: Vec<cap_std::fs::Dir>,
}
impl NewDirectory {
    fn create(parent: &str, name: &str) -> Result<Self, String> {
        use cap_fs_ext::DirExt;
        let target = destination(parent, name)?;
        let parent = target.parent().ok_or("Missing parent folder.")?;
        let mut anchor = PathBuf::new();
        let mut parts = vec![];
        for part in parent.components() {
            match part {
                std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                    anchor.push(part.as_os_str())
                }
                std::path::Component::Normal(name) => parts.push(name.to_owned()),
                _ => return Err("Choose a canonical parent folder.".into()),
            }
        }
        let mut guards =
            vec![
                cap_std::fs::Dir::open_ambient_dir(&anchor, cap_std::ambient_authority())
                    .map_err(|_| "Could not open the parent folder safely.")?,
            ];
        for part in parts {
            let next = guards
                .last()
                .unwrap()
                .open_dir_nofollow(part)
                .map_err(|_| {
                    "A folder changed or contains an unsafe alias. Choose another parent folder."
                })?;
            guards.push(next);
        }
        let parent = guards.last().unwrap();
        let child = super::create_directory::create(parent, name)?;
        if child.entries().map_err(err)?.next().is_some() {
            return Err("The new folder is no longer empty. Existing files were preserved.".into());
        }
        guards.push(child);
        Ok(Self {
            path: target,
            guards,
        })
    }
    fn dir(&self) -> &cap_std::fs::Dir {
        self.guards.last().unwrap()
    }
    fn git(&self, args: &[&str]) -> Result<String, String> {
        super::git_bound(&self.path, self.dir(), args)
    }
    fn ensure_current(&self) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            use cap_std::fs::MetadataExt as CapMetadataExt;
            use std::os::unix::fs::MetadataExt;
            let current = std::fs::metadata(&self.path).map_err(|_| {
                "The new repository moved. Add it from its current folder.".to_string()
            })?;
            let held = self.dir().dir_metadata().map_err(err)?;
            if current.dev() != held.dev() || current.ino() != held.ino() {
                return Err("The destination changed during creation. Your repository was preserved; add its current folder manually.".into());
            }
        }
        Ok(())
    }
    fn register(&self, state: &State) -> Result<Repo, String> {
        self.ensure_current()?;
        let raw = self.git(&[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=normal",
        ])?;
        let (branch, dirty, ahead, behind, _) = super::parse_status_with_head(&raw)?;
        let remote_url = self
            .git(&["remote", "get-url", "origin"])
            .ok()
            .map(|url| super::safe_remote(&url));
        self.ensure_current()?;
        let path = self.path.to_string_lossy().into_owned();
        let name = self
            .path
            .file_name()
            .ok_or("Missing repository name.")?
            .to_string_lossy()
            .into_owned();
        let db = state.database.lock().map_err(err)?;
        db.execute(
            "INSERT INTO repos(path,name,remote_url) VALUES(?1,?2,?3)",
            rusqlite::params![path, name, remote_url],
        )
        .map_err(|_| {
            "The repository is ready but could not be registered. Add its folder manually."
                .to_string()
        })?;
        let id = db.last_insert_rowid();
        super::record(
            &db,
            Some(id),
            &format!("repo-added:{id}"),
            "repo",
            "Repository added",
            Some(&name),
            super::now(),
        )?;
        Ok(Repo {
            id,
            path,
            name,
            github_full_name: remote_url.as_deref().and_then(super::full_name),
            remote_url,
            pinned: false,
            collection: None,
            tags: vec![],
            branch,
            dirty,
            ahead,
            behind,
            ci_conclusion: None,
            ci_status: None,
            status_error: None,
            status_pending: false,
        })
    }
}
fn initialize_local(parent: &str, name: &str) -> Result<NewDirectory, String> {
    let target = NewDirectory::create(parent, name)?;
    target.git(&["init", "--initial-branch=main", "--template="])?;
    Ok(target)
}
fn clone_local(
    parent: &str,
    name: &str,
    url: &str,
    credentials: Option<&super::git_auth::CloneAuthentication>,
) -> Result<NewDirectory, String> {
    let target = NewDirectory::create(parent, name)?;
    super::git_capture_impl(
        &target.path,
        &["clone", "--quiet", "--", url, "."],
        true,
        Some(target.dir()),
        credentials,
    )
    .map_err(|error| format!("{error} Partial files are preserved; inspect the destination before retrying with a new folder name."))?;
    Ok(target)
}

fn clone_url(url: &str) -> Result<String, String> {
    let url = url.trim();
    let invalid = "Enter a repository URL such as https://github.com/owner/repo.git or git@host:owner/repo.git.";
    if url.is_empty()
        || url.len() > 32768
        || url.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(invalid.into());
    }
    // Git also accepts executable remote helpers and local paths. The URL field
    // deliberately accepts network URLs and the conventional SSH shorthand only.
    let parsed = if url.contains("://") {
        url::Url::parse(url).map_err(|_| invalid)?
    } else {
        let (authority, path) = url.split_once(':').ok_or(invalid)?;
        if !authority.contains('@') || authority.contains(['/', '\\', '?', '#']) || path.is_empty()
        {
            return Err(invalid.into());
        }
        url::Url::parse(&format!("ssh://{authority}/{path}")).map_err(|_| invalid)?
    };
    if !matches!(parsed.scheme(), "https" | "http" | "ssh" | "git")
        || parsed.host_str().is_none()
        || parsed.host_str().is_some_and(|host| host.starts_with('-'))
        || parsed.username().starts_with('-')
        || parsed.path().trim_matches('/').is_empty()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(invalid.into());
    }
    if parsed.password().is_some() || (parsed.scheme() != "ssh" && !parsed.username().is_empty()) {
        return Err("Use a URL without embedded credentials. Configure your system Git credentials or SSH key instead.".into());
    }
    Ok(url.into())
}

#[tauri::command]
pub async fn clone_repository_url(
    url: String,
    parent: String,
    name: String,
    state: tauri::State<'_, State>,
) -> Result<Repo, String> {
    let remote = clone_url(&url)?;
    name_valid(&name)?;
    background(state, move |s| {
        let _guard = super::gitflow::MUTATIONS.lock().map_err(err)?;
        let target = clone_local(&parent, &name, &remote, None)?;
        target.register(s)
    })
    .await
}
#[tauri::command]
pub async fn list_online_repos(
    account_id: i64,
    page: u32,
    state: tauri::State<'_, State>,
) -> Result<Page<OnlineRepo>, String> {
    page_valid(page)?;
    let _guard = oauth::ACCOUNT_LOCK.lock().await;
    let api = api(account_id, state).await?;
    api.user().await?;
    let (items, has_more) = api
        .request(
            reqwest::Method::GET,
            &format!("/user/repos?per_page=25&page={page}&sort=updated&direction=desc"),
            None,
        )
        .await?;
    Ok(Page {
        items,
        has_more,
        warning: None,
    })
}
#[tauri::command]
pub async fn repository_owners(
    account_id: i64,
    page: u32,
    state: tauri::State<'_, State>,
) -> Result<Page<Owner>, String> {
    page_valid(page)?;
    let _guard = oauth::ACCOUNT_LOCK.lock().await;
    let api = api(account_id, state).await?;
    let user = api.user().await?;
    let (mut orgs, has_more, warning): (Vec<Owner>, bool,Option<String>) = match api.request(reqwest::Method::GET,&format!("/user/orgs?per_page=100&page={page}"),None).await {
        Ok((items,more))=>(items,more,None),
        Err(_)=>(vec![],false,Some("Organizations are unavailable. Personal ownership is still available; reconnect with organization access if needed.".into()))
    };
    for org in &mut orgs {
        org.kind = "Organization".into();
    }
    if page == 1 {
        orgs.insert(0, user);
    }
    Ok(Page {
        items: orgs,
        has_more,
        warning,
    })
}
#[tauri::command]
pub async fn clone_repository(
    account_id: i64,
    repository_id: u64,
    parent: String,
    name: String,
    state: tauri::State<'_, State>,
) -> Result<Repo, String> {
    if repository_id == 0 {
        return Err("Choose a repository.".into());
    }
    name_valid(&name)?;
    let (remote, credentials) = {
        let _guard = oauth::ACCOUNT_LOCK.lock().await;
        let api = api(account_id, state.clone()).await?;
        api.user().await?;
        let repo: OnlineRepo = api.get(&format!("/repositories/{repository_id}")).await?;
        if repo.id != repository_id {
            return Err("The selected repository changed. Refresh its account list.".into());
        }
        full_valid(&repo.full_name)?;
        (
            format!("https://github.com/{}.git", repo.full_name),
            if repo.private {
                super::git_auth::CloneAuthentication::Github(super::git_auth::GithubCredentials {
                    token: api.token,
                    full_name: repo.full_name,
                })
            } else {
                super::git_auth::CloneAuthentication::PublicGithub
            },
        )
    };
    background(state, move |s| {
        let _guard = super::gitflow::MUTATIONS.lock().map_err(err)?;
        let target = clone_local(&parent, &name, &remote, Some(&credentials))?;
        target.register(s)
    })
    .await
}
#[derive(Deserialize)]
pub struct Create {
    parent: String,
    name: String,
    account_id: Option<i64>,
    owner: Option<String>,
    private: bool,
}
#[tauri::command]
pub async fn create_repository(
    input: Create,
    state: tauri::State<'_, State>,
) -> Result<Created, String> {
    name_valid(&input.name)?;
    let _account_guard = oauth::ACCOUNT_LOCK.lock().await;
    let remote = if let Some(id) = input.account_id {
        let api = api(id, state.clone()).await?;
        let user = api.user().await?;
        let owner = input.owner.as_deref().ok_or("Choose an owner.")?;
        full_valid(&format!("{owner}/{}", input.name))?;
        let route = if owner.eq_ignore_ascii_case(&user.login) {
            "/user/repos".to_string()
        } else {
            #[derive(Deserialize)]
            struct Membership {
                state: String,
            }
            let membership: Membership = api
                .get(&format!("/orgs/{owner}/memberships/{}", user.login))
                .await?;
            if membership.state != "active" {
                return Err("This account is not an active member of that organization.".into());
            }
            format!("/orgs/{owner}/repos")
        };
        Some((api, owner.to_owned(), route))
    } else {
        None
    };
    let parent = input.parent.clone();
    let name = input.name.clone();
    let (repo, target) = background(state.clone(), move |s| {
        let _guard = super::gitflow::MUTATIONS.lock().map_err(err)?;
        let target = initialize_local(&parent, &name)?;
        let repo = target.register(s)?;
        Ok((repo, target))
    })
    .await?;
    let Some((api, owner, route)) = remote else {
        return Ok(Created {
            repo,
            warning: None,
        });
    };
    let created = api
        .request::<OnlineRepo>(
            reqwest::Method::POST,
            &route,
            Some(json!({"name":input.name,"private":input.private,"auto_init":false})),
        )
        .await;
    match created {
        Ok((remote,_)) if remote.full_name.eq_ignore_ascii_case(&format!("{owner}/{}",input.name)) && remote.private==input.private => {
            let url=format!("https://github.com/{}.git",remote.full_name);let id=repo.id;
            let mut attached=repo.clone();
            match background(state,move|s|{
                let _guard=super::gitflow::MUTATIONS.lock().map_err(err)?;
                target.ensure_current()?;
                target.git(&["remote","add","origin",&url])?;
                s.database.lock().map_err(err)?.execute("UPDATE repos SET remote_url=?1 WHERE id=?2",rusqlite::params![url,id]).map_err(err)?;
                attached.github_full_name=super::full_name(&url);attached.remote_url=Some(url);Ok(attached)
            }).await {
                Ok(repo)=>Ok(Created{repo,warning:None}),
                Err(_)=>Ok(Created{repo,warning:Some("GitHub created the repository, but the local remote could not be attached. Your local repository is ready; configure its origin with Git.".into())})
            }
        }
        _=>Ok(Created{repo,warning:Some("Local repository created. GitHub setup did not finish. Check account permissions and GitHub before retrying creation; your local files are safe.".into())})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git;
    #[test]
    fn clone_urls_accept_network_remotes_without_credentials() {
        for url in [
            "https://github.com/owner/project.git",
            "https://gitlab.com/team/subgroup/project/",
            "git@github.com:owner/project.git",
            "ssh://git@example.com:2222/team/project.git",
            "git://example.com/team/project.git",
            "http://localhost:8000/project.git",
        ] {
            assert_eq!(clone_url(&format!("  {url}  ")).unwrap(), url);
        }
        for url in [
            "",
            "--upload-pack=command",
            "ext::command",
            "/tmp/repo",
            "file:///tmp/repo",
            "https://github.com/",
            "https://github.com/owner/repo#readme",
            "https://user:secret@github.com/owner/repo.git",
            "https://secret@github.com/owner/repo.git",
            "git@host:",
            "ssh://git:secret@host/repo.git",
            "https://host/repo\ncommand",
        ] {
            assert!(clone_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn clone_errors_distinguish_access_from_local_permissions_and_hide_stderr() {
        assert!(crate::clone_failure("Permission denied (publickey).", false).contains("SSH key"));
        assert!(crate::clone_failure(
            "fatal: Authentication failed for https://secret@github.com/owner/project",
            true
        )
        .contains("Reconnect"));
        assert!(crate::clone_failure(
            "fatal: could not create work tree dir: Permission denied",
            false
        )
        .contains("writable"));
        assert!(crate::clone_failure(
            "fatal: unable to access https://secret@host/repo: Could not resolve host",
            false
        )
        .contains("connection"));
        assert!(!crate::clone_failure("fatal: https://secret@host/", false).contains("secret"));
    }
    #[test]
    fn git_execution_does_not_select_an_executable_from_the_new_repository() {
        let parent = tempfile::tempdir().unwrap();
        let target = initialize_local(parent.path().to_str().unwrap(), "project").unwrap();
        let fake = target
            .path
            .join(if cfg!(windows) { "git.exe" } else { "git" });
        std::fs::write(&fake, "#!/bin/sh\nexit 97\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert!(target
            .git(&["rev-parse", "--is-inside-work-tree"])
            .unwrap()
            .contains("true"));
    }
    #[test]
    fn provider_organization_shape_does_not_require_a_type_field() {
        let owner: Owner = serde_json::from_str(r#"{"login":"example-org","id":42}"#).unwrap();
        assert_eq!(owner.login, "example-org");
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn group_writable_workspaces_can_clone_into_a_private_new_directory() {
        use std::os::unix::fs::PermissionsExt;
        let parent = tempfile::tempdir().unwrap();
        std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o775)).unwrap();
        let source = initialize_local(parent.path().to_str().unwrap(), "source").unwrap();
        let cloned = clone_local(
            parent.path().to_str().unwrap(),
            "copy",
            source.path.to_str().unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(
            std::fs::metadata(&cloned.path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert!(cloned
            .git(&["rev-parse", "--is-inside-work-tree"])
            .unwrap()
            .contains("true"));
        std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(NewDirectory::create(parent.path().to_str().unwrap(), "unsafe").is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn init_uses_retained_directory_after_the_destination_name_is_replaced() {
        let parent = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = NewDirectory::create(parent.path().to_str().unwrap(), "new").unwrap();
        std::fs::rename(&target.path, parent.path().join("moved")).unwrap();
        std::os::unix::fs::symlink(outside.path(), &target.path).unwrap();
        target
            .git(&["init", "--initial-branch=main", "--template="])
            .unwrap();
        assert!(parent.path().join("moved/.git").is_dir());
        assert!(!outside.path().join(".git").exists());
        assert!(target.ensure_current().is_err());
    }
    #[cfg(windows)]
    #[test]
    fn creation_retains_directory_components_against_rename() {
        let parent = tempfile::tempdir().unwrap();
        let target = NewDirectory::create(parent.path().to_str().unwrap(), "new").unwrap();
        assert!(std::fs::rename(&target.path, parent.path().join("moved")).is_err());
        assert!(std::fs::rename(parent.path(), parent.path().with_extension("moved")).is_err());
    }
    #[test]
    fn names_cannot_escape_or_replace_existing_folders() {
        let p = tempfile::tempdir().unwrap();
        for name in [
            "../outside",
            "a/b",
            "-flag/other",
            ".",
            "..",
            "NUL",
            "COM1.txt",
            "trailing.",
        ] {
            assert!(
                destination(p.path().to_str().unwrap(), name).is_err(),
                "{name}"
            );
        }
        let holder = initialize_local(p.path().to_str().unwrap(), "project").unwrap();
        let path = holder.path.clone();
        std::fs::write(path.join("keep.txt"), "keep").unwrap();
        assert!(initialize_local(p.path().to_str().unwrap(), "project").is_err());
        assert_eq!(
            std::fs::read_to_string(path.join("keep.txt")).unwrap(),
            "keep"
        );
        assert!(!super::super::gitflow::state(&path).unwrap().has_head);
    }
    #[test]
    fn local_clone_registers_real_git_history_without_overwriting() {
        let p = tempfile::tempdir().unwrap();
        let source_holder = initialize_local(p.path().to_str().unwrap(), "source").unwrap();
        let source = source_holder.path.clone();
        git(&source, &["config", "user.name", "Fixture"]).unwrap();
        git(
            &source,
            &["config", "user.email", "fixture@example.invalid"],
        )
        .unwrap();
        std::fs::write(source.join("readme.txt"), "example").unwrap();
        git(&source, &["add", "."]).unwrap();
        git(
            &source,
            &["-c", "commit.gpgsign=false", "commit", "-m", "initial"],
        )
        .unwrap();
        let cloned = clone_local(
            p.path().to_str().unwrap(),
            "copy",
            p.path().join("source").to_str().unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(
            git(&cloned.path, &["rev-parse", "HEAD"]).unwrap(),
            git(&source, &["rev-parse", "HEAD"]).unwrap()
        );
        assert_eq!(
            std::fs::read_to_string(cloned.path.join("readme.txt")).unwrap(),
            "example"
        );
        assert_eq!(
            git(&cloned.path, &["remote", "get-url", "origin"]).unwrap(),
            source.to_str().unwrap()
        );
        let state = State {
            database: std::sync::Arc::new(std::sync::Mutex::new(
                crate::initialize(&p.path().join("workspace.sqlite")).unwrap(),
            )),
            credential_service: "reponiq-test",
        };
        let registered = cloned.register(&state).unwrap();
        assert_eq!(registered.name, "copy");
        assert_eq!(registered.branch, "main");
        assert_eq!(registered.dirty, 0);
        let credentials = super::super::git_auth::CloneAuthentication::Github(
            super::super::git_auth::GithubCredentials {
                token: "fixture-secret".into(),
                full_name: "owner/project".into(),
            },
        );
        let authenticated = clone_local(
            p.path().to_str().unwrap(),
            "account-copy",
            source.to_str().unwrap(),
            Some(&credentials),
        )
        .unwrap();
        let config = std::fs::read_to_string(authenticated.path.join(".git/config")).unwrap();
        assert!(!config.contains("fixture-secret"));
        assert!(!config.contains("credential"));
        assert!(!config.contains("extraHeader"));
        assert!(clone_local(
            p.path().to_str().unwrap(),
            "copy",
            p.path().join("source").to_str().unwrap(),
            None,
        )
        .is_err());
    }

    #[test]
    fn failed_clones_preserve_the_reason_and_do_not_register_a_repository() {
        let p = tempfile::tempdir().unwrap();
        let result = clone_local(
            p.path().to_str().unwrap(),
            "copy",
            p.path().join("missing").to_str().unwrap(),
            None,
        );
        let error = result.err().unwrap();
        assert!(error.contains("Partial files are preserved"));
        assert!(p.path().join("copy").is_dir());
        assert!(!p.path().join("copy/.git").exists());
    }
}
