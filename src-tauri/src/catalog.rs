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
fn clone_local(parent: &str, name: &str, url: &str) -> Result<NewDirectory, String> {
    let target = NewDirectory::create(parent, name)?;
    target.git(&["clone","--",url,"."]).map_err(|_|"Clone did not finish. Check system Git authentication and the destination folder before retrying; partial files are preserved.".to_string())?;
    Ok(target)
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
    let remote = {
        let _guard = oauth::ACCOUNT_LOCK.lock().await;
        let api = api(account_id, state.clone()).await?;
        api.user().await?;
        let repo: OnlineRepo = api.get(&format!("/repositories/{repository_id}")).await?;
        if repo.id != repository_id {
            return Err("The selected repository changed. Refresh its account list.".into());
        }
        full_valid(&repo.full_name)?;
        format!("https://github.com/{}.git", repo.full_name)
    };
    background(state, move |s| {
        let _guard = super::gitflow::MUTATIONS.lock().map_err(err)?;
        let target = clone_local(&parent, &name, &remote)?;
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
        )
        .unwrap();
        assert_eq!(
            git(&cloned.path, &["rev-parse", "HEAD"]).unwrap(),
            git(&source, &["rev-parse", "HEAD"]).unwrap()
        );
        assert!(clone_local(
            p.path().to_str().unwrap(),
            "copy",
            p.path().join("source").to_str().unwrap()
        )
        .is_err());
    }
}
