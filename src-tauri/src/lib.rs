use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager,
};
mod branches;
mod catalog;
mod conflict_file;
mod create_directory;
mod diagnostics;
mod git_auth;
mod git_process;
mod git_scratch;
#[cfg(target_os = "linux")]
#[doc(hidden)]
pub use git_process::run_git_supervisor;
mod appearance;
mod gitflow;
mod launch_settings;
mod oauth;
mod pulls;
mod registry_theme;
mod repo_actions;
#[cfg(target_os = "linux")]
mod repository_identity;
mod startup;
mod storage;
mod updater;
mod workflow;
mod workspace;
#[doc(hidden)]
pub use catalog::run_private_clone_worker;
#[doc(hidden)]
pub use workspace::run_workspace_worker;

#[derive(Clone)]
struct State {
    database: Arc<Mutex<Connection>>,
    credential_service: &'static str,
}
#[derive(Deserialize)]
struct GitExpectation {
    branch: String,
    head: Option<String>,
    path: Option<String>,
    author: Option<String>,
    registration: Option<String>,
}
#[derive(Serialize, Clone)]
struct Repo {
    id: i64,
    registration: Option<String>,
    path: String,
    name: String,
    remote_url: Option<String>,
    pinned: bool,
    collection: Option<String>,
    tags: Vec<String>,
    branch: String,
    dirty: usize,
    ahead: i64,
    behind: i64,
    github_full_name: Option<String>,
    ci_conclusion: Option<String>,
    ci_status: Option<String>,
    status_error: Option<String>,
    status_pending: bool,
}
#[derive(Serialize)]
struct Commit {
    hash: String,
    subject: String,
    author: String,
    timestamp: i64,
}
#[derive(Serialize)]
struct Change {
    x: String,
    y: String,
    path: String,
    original_path: Option<String>,
}
#[derive(Serialize)]
struct Details {
    commits: Vec<Commit>,
    changes: Vec<Change>,
    branches: Vec<branches::Branch>,
    saved_changes: Vec<branches::SavedChanges>,
    git: gitflow::GitState,
}
#[derive(Serialize)]
struct Activity {
    id: i64,
    repo_id: Option<i64>,
    repo_name: Option<String>,
    kind: String,
    title: String,
    detail: Option<String>,
    can_open_in_browser: bool,
    occurred_at: i64,
}
#[derive(Serialize)]
struct Account {
    id: i64,
    login: String,
    api_host: String,
    created_at: i64,
}
#[derive(Deserialize)]
struct GithubUser {
    login: String,
}
#[derive(Deserialize)]
struct WorkflowRuns {
    workflow_runs: Vec<WorkflowRun>,
}
#[derive(Deserialize)]
struct WorkflowRun {
    id: i64,
    name: Option<String>,
    status: String,
    conclusion: Option<String>,
    html_url: String,
    created_at: String,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
fn initialize(database: &Path) -> Result<Connection, String> {
    let base = database.parent().ok_or("Missing database directory")?;
    std::fs::create_dir_all(&base).map_err(err)?;
    let db = Connection::open(database).map_err(err)?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
       CREATE TABLE IF NOT EXISTS repos(id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, name TEXT NOT NULL, remote_url TEXT, pinned INTEGER NOT NULL DEFAULT 0, collection TEXT, tags TEXT NOT NULL DEFAULT '[]', ci_status TEXT, ci_conclusion TEXT);
       CREATE TABLE IF NOT EXISTS activity(id INTEGER PRIMARY KEY, repo_id INTEGER REFERENCES repos(id) ON DELETE SET NULL, source_key TEXT UNIQUE, kind TEXT NOT NULL, title TEXT NOT NULL, detail TEXT, occurred_at INTEGER NOT NULL);
       CREATE TABLE IF NOT EXISTS preferences(key TEXT PRIMARY KEY, value TEXT NOT NULL);
       CREATE TABLE IF NOT EXISTS accounts(id INTEGER PRIMARY KEY, login TEXT NOT NULL, api_host TEXT NOT NULL, created_at INTEGER NOT NULL, UNIQUE(login,api_host));
       CREATE INDEX IF NOT EXISTS idx_activity_date ON activity(occurred_at DESC);").map_err(err)?;
    workflow::initialize(&db)?;
    #[cfg(target_os = "linux")]
    repository_identity::initialize(&db)?;
    {
        let mut stmt = db
            .prepare("SELECT id,remote_url FROM repos WHERE remote_url IS NOT NULL")
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .map_err(err)?;
        for row in rows {
            let (id, remote) = row.map_err(err)?;
            let safe = safe_remote(&remote);
            if safe != remote {
                db.execute(
                    "UPDATE repos SET remote_url=?1 WHERE id=?2",
                    params![safe, id],
                )
                .map_err(err)?;
            }
        }
    }
    Ok(db)
}
fn dirs_path() -> Result<PathBuf, String> {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or("APPDATA not found".into())
    } else {
        Ok(std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local/share")
            }))
    }
}
fn git_binary() -> Result<PathBuf, String> {
    static BINARY: std::sync::OnceLock<Result<PathBuf, String>> = std::sync::OnceLock::new();
    BINARY
        .get_or_init(|| {
            let paths = std::env::var_os("PATH").ok_or("Install Git and restart Reponiq.")?;
            for directory in std::env::split_paths(&paths).filter(|p| p.is_absolute()) {
                let binary = directory.join(if cfg!(windows) { "git.exe" } else { "git" });
                if !binary.is_file() {
                    continue;
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if std::fs::metadata(&binary)
                        .map_err(err)?
                        .permissions()
                        .mode()
                        & 0o111
                        == 0
                    {
                        continue;
                    }
                }
                return std::fs::canonicalize(binary).map_err(err);
            }
            Err("Could not find installed Git on PATH. Install Git and restart Reponiq.".into())
        })
        .clone()
}
fn git(path: &Path, args: &[&str]) -> Result<String, String> {
    git_capture(path, args, true)
}
fn git_with_input(path: &Path, args: &[&str], input: &[u8]) -> Result<String, String> {
    git_capture_impl(path, args, true, None, None, false, Some(input))
}
fn git_exact(path: &Path, args: &[&str]) -> Result<String, String> {
    git_capture(path, args, false)
}
fn git_capture(path: &Path, args: &[&str], trim: bool) -> Result<String, String> {
    git_capture_impl(path, args, trim, None, None, false, None)
}
fn git_bound(path: &Path, directory: &cap_std::fs::Dir, args: &[&str]) -> Result<String, String> {
    git_capture_impl(path, args, true, Some(directory), None, false, None)
}
fn git_startup_fetch(path: &Path) -> Result<String, String> {
    git_capture_impl(
        path,
        &[
            "fetch",
            "--all",
            "--prune",
            "--no-prune-tags",
            "--no-tags",
            "--no-recurse-submodules",
        ],
        true,
        None,
        None,
        true,
        None,
    )
}
fn git_capture_impl(
    path: &Path,
    args: &[&str],
    trim: bool,
    directory: Option<&cap_std::fs::Dir>,
    credentials: Option<&git_auth::CloneAuthentication>,
    quiet_startup: bool,
    input: Option<&[u8]>,
) -> Result<String, String> {
    use std::io::{Seek, Write};
    use std::process::Stdio;
    let mut arguments = args.iter().copied();
    let mut operation = "command";
    while let Some(argument) = arguments.next() {
        if argument == "-c" {
            arguments.next();
        } else if !argument.starts_with('-') {
            operation = argument;
            break;
        }
    }
    log::debug!(target: "reponiq::git", "{operation} started in {}", path.display());
    let mut command = Command::new(git_binary()?);
    command
        .env_remove("REPONIQ_GIT_TOKEN")
        .env_remove("REPONIQ_GIT_PATH");
    #[cfg(target_os = "linux")]
    for selector in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(selector);
    }
    // The supervisor owns short-lived commands. A new FSMonitor daemon would
    // be stopped on exit, so these commands use Git's regular status scan.
    command.args(["-c", "core.fsmonitor=false"]);
    if let Some(credentials) = credentials {
        credentials.configure(&mut command);
    }
    if quiet_startup {
        command
            .args(["-c", "core.askPass="])
            .env("GIT_ASKPASS", "")
            .env("SSH_ASKPASS", "")
            .env("SSH_ASKPASS_REQUIRE", "never");
    }
    #[cfg(target_os = "linux")]
    let checked_directory = if directory.is_none() {
        let can_discover =
            matches!(operation, "init" | "clone") || args == ["rev-parse", "--show-toplevel"];
        let (directory, git_dir) = repository_identity::open_for_git(path, can_discover)?;
        if let Some(git_dir) = git_dir {
            // Bind commands to this checkout even if its own metadata becomes
            // invalid. Ordinary Git discovery could select an enclosing repo.
            command.env("GIT_DIR", git_dir);
            if git_dir == ".git" {
                command.env("GIT_WORK_TREE", ".");
            }
        }
        Some(directory)
    } else {
        // Create/Clone already own a private retained capability. Continue on
        // that object if the pathname moves, and verify it before registration.
        None
    };
    #[cfg(target_os = "linux")]
    let directory = directory.or(checked_directory.as_ref());
    if let Some(directory) = directory {
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;
            command.current_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()));
        }
        #[cfg(windows)]
        {
            let _ = directory;
            command.current_dir(path);
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            let _ = directory;
            return Err("Safe repository creation is unavailable on this platform.".into());
        }
    }
    command
        .arg("-C")
        .arg(if directory.is_some() {
            Path::new(".")
        } else {
            path
        })
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GCM_INTERACTIVE", "never");
    let scratch = git_scratch::prepare(&mut command)?;
    let stdin = if let Some(bytes) = input {
        if bytes.len() > git_process::INPUT_LIMIT {
            return Err("The selected paths are too large for one Git operation. Select fewer files and try again.".into());
        }
        // Prepare the entire path list before launching a mutating Git command.
        let mut file = match &scratch {
            Some(scratch) => tempfile::tempfile_in(scratch.path()),
            None => tempfile::tempfile(),
        }
        .map_err(err)?;
        file.write_all(bytes).map_err(err)?;
        file.rewind().map_err(err)?;
        Stdio::from(file)
    } else {
        Stdio::null()
    };
    let start = std::time::Instant::now();
    let cloning = args.first() == Some(&"clone");
    let deadline = if cloning {
        15 * 60
    } else if args
        .first()
        .is_some_and(|verb| matches!(*verb, "fetch" | "pull" | "push"))
    {
        60
    } else {
        10
    };
    let output = match git_process::capture(
        command,
        stdin,
        std::time::Duration::from_secs(deadline),
    ) {
        Ok(output) => output,
        Err(git_process::Error::Start(error)) => {
            log::error!(target: "reponiq::git", "{operation} could not start: {error}");
            return Err(
                "Could not start Git. Check your Git installation and restart Reponiq.".into(),
            );
        }
        Err(git_process::Error::Io(error)) => {
            log::error!(target: "reponiq::git", "{operation} process supervision failed: {error}");
            return Err(
                "Could not monitor Git. Check the repository before retrying an action.".into(),
            );
        }
        Err(git_process::Error::Timeout) => {
            log::error!(target: "reponiq::git", "{operation} stopped after {} ms: time limit reached", start.elapsed().as_millis());
            if cloning {
                return Err("Clone exceeded its 15-minute time limit. Check the connection and destination folder before retrying.".into());
            }
            return Err(
                "Git exceeded its time limit. Check the repository before retrying an action."
                    .into(),
            );
        }
        Err(git_process::Error::OutputLimit) => {
            log::error!(target: "reponiq::git", "{operation} exceeded its output limit");
            return Err("Git output is too large. Check the repository before retrying, or use Git in a terminal for the full output.".into());
        }
    };
    let status = output.status;
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    if !status.success() {
        log::error!(target: "reponiq::git", "{operation} failed ({status}) after {} ms\n{diagnostic}", start.elapsed().as_millis());
        if diagnostic.contains("detected dubious ownership") {
            return Err("Git does not trust this folder's ownership. Check its owner and approve only this trusted repository in Git before importing.".into());
        }
        let lower = diagnostic.to_ascii_lowercase();
        if matches!(operation, "fetch" | "push" | "ls-remote") {
            if lower.contains("authentication failed")
                || lower.contains("could not read username")
                || lower.contains("permission denied (publickey")
            {
                return Err(
                    "Git could not authenticate. Check your system Git credentials or SSH key."
                        .into(),
                );
            }
            if lower.contains("could not resolve host")
                || lower.contains("failed to connect")
                || lower.contains("connection timed out")
            {
                return Err(
                    "Could not connect to the Git remote. Check your connection or VPN and retry."
                        .into(),
                );
            }
        }
        if cloning {
            return Err(clone_failure(
                &diagnostic,
                credentials.is_some_and(git_auth::CloneAuthentication::uses_account),
            )
            .into());
        }
        if args.first() == Some(&"switch") {
            if diagnostic.contains("already checked out at")
                || diagnostic.contains("already used by worktree")
            {
                return Err("This branch is open in another worktree. Open that folder or switch its branch first.".into());
            }
            if diagnostic.contains("would be overwritten by checkout")
                || diagnostic.contains("would be overwritten by switch")
            {
                return Err("Switching would overwrite working files. Review your changes and move any obstructing ignored files before trying again.".into());
            }
        }
        return Err(format!("Git exited with status {}. Check repository access, branch state, and system Git credentials.",status.code().unwrap_or(-1)));
    }
    log::debug!(target: "reponiq::git", "{operation} completed in {} ms", start.elapsed().as_millis());
    if !diagnostic.trim().is_empty() {
        log::info!(target: "reponiq::git", "{operation} output:\n{diagnostic}");
    }
    if trim {
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end_matches(['\r', '\n'])
            .to_string())
    } else {
        String::from_utf8(output.stdout)
            .map_err(|_| "This Git output is not UTF-8 text. Use your external Git tools.".into())
    }
}

fn clone_failure(diagnostic: &str, account: bool) -> &'static str {
    // Classify stderr without returning it: Git may print credentials or URLs.
    let diagnostic = diagnostic.to_ascii_lowercase();
    if diagnostic.contains("authentication failed")
        || diagnostic.contains("could not read username")
        || diagnostic.contains("could not read password")
        || diagnostic.contains("repository not found")
        || diagnostic.contains("permission denied (publickey")
        || diagnostic.contains("access denied")
        || diagnostic.contains("error: 403")
        || diagnostic.contains("error: 401")
    {
        if account {
            "GitHub refused access. Reconnect the selected account with repository access and check any organization SSO requirements."
        } else {
            "Git could not authenticate or find the repository. Check the URL and your system Git credentials or SSH key."
        }
    } else if diagnostic.contains("permission denied") || diagnostic.contains("access is denied") {
        "Git could not write to the destination. Choose a writable parent folder and check its permissions."
    } else if diagnostic.contains("could not resolve host")
        || diagnostic.contains("failed to connect")
        || diagnostic.contains("unable to access")
        || diagnostic.contains("connection timed out")
    {
        "Git could not reach the repository. Check your connection, proxy settings, and repository URL."
    } else if diagnostic.contains("no space left") || diagnostic.contains("disk full") {
        "There is not enough disk space to clone this repository."
    } else if diagnostic.contains("checkout failed") {
        "Git downloaded the repository but could not check out its files. Inspect the destination for file-name or permission problems."
    } else {
        "Git could not finish cloning. Check repository access, system Git settings, and the destination folder."
    }
}

fn root(path: &str) -> Result<PathBuf, String> {
    let dir = std::fs::canonicalize(path).map_err(err)?;
    if !dir.is_dir() {
        return Err("Path is not a directory".into());
    }
    let output = git(&dir, &["rev-parse", "--show-toplevel"])?;
    std::fs::canonicalize(output).map_err(err)
}
fn full_name(remote: &str) -> Option<String> {
    let raw = remote.trim_end_matches('/').trim_end_matches(".git");
    let path = if let Some(v) = raw.strip_prefix("git@github.com:") {
        v.to_string()
    } else if let Ok(u) = url::Url::parse(raw) {
        if u.host_str() != Some("github.com") {
            return None;
        }
        u.path().trim_start_matches('/').to_string()
    } else {
        return None;
    };
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|v| {
            matches!(*v, "" | "." | "..")
                || !v
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        })
    {
        return None;
    }
    Some(format!("{}/{}", parts[0], parts[1]))
}

fn record(
    db: &Connection,
    id: Option<i64>,
    key: &str,
    kind: &str,
    title: &str,
    detail: Option<&str>,
    time: i64,
) -> Result<(), String> {
    db.execute("INSERT OR IGNORE INTO activity(repo_id,source_key,kind,title,detail,occurred_at) VALUES (?1,?2,?3,?4,?5,?6)",params![id,key,kind,title,detail,time]).map_err(err)?;
    Ok(())
}
fn repo_path(db: &Connection, id: i64) -> Result<PathBuf, String> {
    let path: String = db
        .query_row("SELECT path FROM repos WHERE id=?1", [id], |r| r.get(0))
        .map_err(err)?;
    Ok(PathBuf::from(path))
}
fn checked_repo_path(db: &Connection, id: i64) -> Result<PathBuf, String> {
    let path = repo_path(db, id)?;
    #[cfg(target_os = "linux")]
    repository_identity::check(db, id, &path, false)?;
    Ok(path)
}
fn status(path: &Path) -> Result<(String, usize, i64, i64), String> {
    let (branch, dirty, ahead, behind, _) = status_with_head(path)?;
    Ok((branch, dirty, ahead, behind))
}
fn status_with_head(path: &Path) -> Result<(String, usize, i64, i64, Option<String>), String> {
    let raw = git(
        path,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=normal",
        ],
    )?;
    parse_status_with_head(&raw)
}
fn parse_status_with_head(raw: &str) -> Result<(String, usize, i64, i64, Option<String>), String> {
    let mut branch = "(detached)".to_string();
    let mut dirty = 0;
    let mut ahead = 0;
    let mut behind = 0;
    let mut head = None;
    for line in raw.lines() {
        if let Some(value) = line.strip_prefix("# branch.head ") {
            branch = if value == "(detached)" {
                "(detached)".into()
            } else {
                value.into()
            };
        } else if let Some(value) = line.strip_prefix("# branch.oid ") {
            if value != "(initial)" {
                head = Some(value.to_string());
            }
        } else if let Some(value) = line.strip_prefix("# branch.ab ") {
            let mut p = value.split_whitespace();
            ahead = p
                .next()
                .and_then(|v| v.trim_start_matches('+').parse().ok())
                .unwrap_or(0);
            behind = p
                .next()
                .and_then(|v| v.trim_start_matches('-').parse().ok())
                .unwrap_or(0);
        } else if !line.starts_with('#') && !line.is_empty() {
            dirty += 1;
        }
    }
    Ok((branch, dirty, ahead, behind, head))
}

fn build_repo(
    id: i64,
    path: String,
    name: String,
    remote_url: Option<String>,
    pinned: i64,
    collection: Option<String>,
    tags: String,
    ci_status: Option<String>,
    ci_conclusion: Option<String>,
) -> Repo {
    let mut repo = repository_metadata(
        id,
        path,
        name,
        remote_url,
        pinned,
        collection,
        tags,
        ci_status,
        ci_conclusion,
    );
    update_summary(&mut repo);
    repo
}
fn update_summary(repo: &mut Repo) {
    match status(Path::new(&repo.path)) {
        Ok((branch, dirty, ahead, behind)) => {
            repo.branch = branch;
            repo.dirty = dirty;
            repo.ahead = ahead;
            repo.behind = behind;
            repo.status_error = None;
        }
        Err(error) => {
            repo.branch = "Unavailable".into();
            repo.status_error = Some(error);
        }
    }
    repo.status_pending = false;
}
fn repository_metadata(
    id: i64,
    path: String,
    name: String,
    remote_url: Option<String>,
    pinned: i64,
    collection: Option<String>,
    tags: String,
    ci_status: Option<String>,
    ci_conclusion: Option<String>,
) -> Repo {
    let github_full_name = remote_url.as_deref().and_then(full_name);
    Repo {
        id,
        registration: None,
        path,
        name,
        remote_url,
        pinned: pinned != 0,
        collection,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        branch: String::new(),
        dirty: 0,
        ahead: 0,
        behind: 0,
        github_full_name,
        ci_conclusion,
        ci_status,
        status_error: None,
        status_pending: true,
    }
}
#[derive(Serialize)]
struct RepoSummary {
    id: i64,
    path: String,
    registration: Option<String>,
    branch: String,
    dirty: usize,
    ahead: i64,
    behind: i64,
    status_error: Option<String>,
    conflicts: usize,
    missing: bool,
}
fn repo_summary_impl(id: i64, state: &State) -> Result<RepoSummary, String> {
    let (path, registration, identity) = {
        let db = state.database.lock().map_err(err)?;
        (
            repo_path(&db, id)?,
            workflow::registration(&db, id)?,
            checked_repo_path(&db, id),
        )
    };
    let gone = matches!(path.try_exists(), Ok(false));
    #[cfg(target_os = "linux")]
    let missing = gone
        || identity
            .as_ref()
            .is_err_and(|error| repository_identity::requires_locate(error));
    #[cfg(not(target_os = "linux"))]
    let missing = gone;
    let result = if gone {
        Err("The repository folder is missing.".into())
    } else if let Err(error) = identity {
        Err(error)
    } else {
        git(
            &path,
            &[
                "status",
                "--porcelain=v2",
                "--branch",
                "--untracked-files=normal",
            ],
        )
    };
    let conflicts = result
        .as_ref()
        .map(|raw| raw.lines().filter(|line| line.starts_with("u ")).count())
        .unwrap_or(0);
    let (branch, dirty, ahead, behind, status_error) =
        match result.and_then(|raw| parse_status_with_head(&raw)) {
            Ok((b, d, a, z, _)) => (b, d, a, z, None),
            Err(e) => ("Unavailable".into(), 0, 0, 0, Some(e)),
        };
    let summary = RepoSummary {
        id,
        path: path.to_string_lossy().into_owned(),
        registration,
        branch,
        dirty,
        ahead,
        behind,
        status_error,
        conflicts,
        missing,
    };
    if summary.status_error.is_none() {
        if let Ok(db) = state.database.lock() {
            let _ = workflow::cache_status(&db, &summary);
        }
    }
    Ok(summary)
}

fn list_repos_impl(state: &State) -> Result<Vec<Repo>, String> {
    let db = state.database.lock().map_err(err)?;
    let mut stmt=db.prepare("SELECT r.id,path,name,remote_url,pinned,collection,tags,ci_status,ci_conclusion,w.token FROM repos r LEFT JOIN workflow_registrations w ON w.repo_id=r.id ORDER BY pinned DESC,name COLLATE NOCASE").map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })
        .map_err(err)?;
    let rows = rows.collect::<Result<Vec<_>, _>>().map_err(err)?;
    drop(stmt);
    drop(db);
    let mut result = Vec::new();
    for row in rows.into_iter().map(Ok::<_, rusqlite::Error>) {
        let (id, path, name, remote, pin, col, tags, cs, cc, registration) = row.map_err(err)?;
        let mut repo = repository_metadata(id, path, name, remote, pin, col, tags, cs, cc);
        repo.registration = registration;
        result.push(repo);
    }
    Ok(result)
}
#[derive(Serialize, Deserialize)]
struct PreparedRepo {
    path: String,
    name: String,
    remote: Option<String>,
    #[cfg(target_os = "linux")]
    identity: repository_identity::Identity,
}

fn prepare_repo(path: &str) -> Result<PreparedRepo, String> {
    let path = root(path)?;
    let canonical = path
        .to_str()
        .ok_or("The folder name is not UTF-8.")?
        .to_owned();
    let name = path
        .file_name()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| "repository".into());
    let remote = git(&path, &["remote", "get-url", "origin"])
        .ok()
        .map(|url| safe_remote(&url));
    Ok(PreparedRepo {
        path: canonical,
        name,
        remote,
        #[cfg(target_os = "linux")]
        identity: repository_identity::observe(&path)?,
    })
}

fn register_repo(prepared: PreparedRepo, state: &State) -> Result<Repo, String> {
    let PreparedRepo {
        path: canonical,
        name,
        remote,
        #[cfg(target_os = "linux")]
        identity,
    } = prepared;
    let mut db = state.database.lock().map_err(err)?;
    let tx = db.transaction().map_err(err)?;
    tx.execute(
        "INSERT OR IGNORE INTO repos(path,name,remote_url) VALUES(?1,?2,?3)",
        params![canonical, name, remote],
    )
    .map_err(err)?;
    let row=tx.query_row("SELECT id,name,remote_url,pinned,collection,tags,ci_status,ci_conclusion FROM repos WHERE path=?1",[&canonical],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,i64>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,String>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,Option<String>>(7)?))).map_err(err)?;
    #[cfg(target_os = "linux")]
    repository_identity::record_identity(&tx, row.0, &identity, false)?;
    record(
        &tx,
        Some(row.0),
        &format!("repo-added:{}", row.0),
        "repo",
        "Repository added",
        Some(&name),
        now(),
    )?;
    let registration = workflow::registration(&tx, row.0)?;
    tx.commit().map_err(err)?;
    drop(db);
    let mut repo = build_repo(
        row.0, canonical, row.1, row.2, row.3, row.4, row.5, row.6, row.7,
    );
    repo.registration = registration;
    Ok(repo)
}
fn rename_repo_impl(target: workflow::Target, name: String, state: &State) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
        return Err("Enter a repository name between 1 and 120 characters.".into());
    }
    let mut db = state.database.lock().map_err(err)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    if !target.is_current_registration(&tx)? {
        return Err("The repository changed. Reopen it and try again.".into());
    }
    tx.execute(
        "UPDATE repos SET name=?1 WHERE id=?2",
        params![name, target.id],
    )
    .map_err(err)?;
    tx.commit().map_err(err)
}

fn delete_repository_folder(db: &Connection, target: &workflow::Target) -> Result<(), String> {
    use cap_fs_ext::DirExt;
    use cap_std::fs::Dir;

    let path = target.verified_path(db)?;
    let parent = path
        .parent()
        .ok_or("The repository must be inside a parent folder.")?;
    let name = path
        .file_name()
        .ok_or("The repository folder has no name.")?;
    if root(&target.path)? != path {
        return Err("The repository root changed. Review its location before deleting it.".into());
    }

    let parent_dir = Dir::open_ambient_dir(parent, cap_std::ambient_authority()).map_err(err)?;
    let folder = parent_dir.open_dir_nofollow(name).map_err(err)?;
    folder
        .open_dir_nofollow(".git")
        .map_err(|_| "Only a standalone repository folder can be deleted here.")?;
    if git(&path, &["worktree", "list", "--porcelain", "-z"])?
        .split('\0')
        .filter(|field| field.starts_with("worktree "))
        .count()
        != 1
    {
        return Err("Remove linked worktrees before deleting this repository folder.".into());
    }

    let inside_folder = |other: &Path| {
        #[cfg(windows)]
        return PathBuf::from(other.to_string_lossy().to_lowercase())
            .starts_with(PathBuf::from(path.to_string_lossy().to_lowercase()));
        #[cfg(not(windows))]
        other.starts_with(&path)
    };
    if let Some(database_path) = db.path() {
        let database_path = std::fs::canonicalize(database_path).map_err(err)?;
        if inside_folder(&database_path) {
            return Err("This folder contains Reponiq's workspace data.".into());
        }
    }

    let mut paths = db
        .prepare("SELECT path FROM repos WHERE id<>?1")
        .map_err(err)?;
    let registered = paths
        .query_map([target.id], |row| row.get::<_, String>(0))
        .map_err(err)?;
    for registered_path in registered {
        if inside_folder(Path::new(&registered_path.map_err(err)?)) {
            return Err("This folder contains another registered repository.".into());
        }
    }

    if root(&target.path)? != path {
        return Err("The repository root changed. Review its location before deleting it.".into());
    }
    #[cfg(target_os = "linux")]
    {
        use cap_std::fs::MetadataExt;
        let metadata = folder.dir_metadata().map_err(err)?;
        repository_identity::record_identity(
            db,
            target.id,
            &(
                target.path.clone(),
                metadata.dev().to_string(),
                metadata.ino().to_string(),
            ),
            false,
        )?;
        target.verified_path(db)?;
    }
    folder.remove_open_dir_all().map_err(|error| {
        format!(
            "The folder could not be fully deleted. The repository is still registered. {error}"
        )
    })?;
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(
            "The folder could not be confirmed deleted. The repository is still registered.".into(),
        ),
    }
}

fn remove_repo_impl(
    target: workflow::Target,
    delete_folder: bool,
    state: &State,
) -> Result<(), String> {
    let _mutation = gitflow::MUTATIONS.lock().map_err(err)?;
    let mut db = state.database.lock().map_err(err)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    if !target.is_current_registration(&tx)? {
        return Err("The repository changed. Reopen it and try again.".into());
    }
    if delete_folder {
        delete_repository_folder(&tx, &target)?;
    }
    tx.execute("DELETE FROM repos WHERE id=?1", [target.id])
        .map_err(err)?;
    tx.commit().map_err(err)
}
fn pin_repo_impl(id: i64, pinned: bool, state: &State) -> Result<(), String> {
    workflow::pin(&mut *state.database.lock().map_err(err)?, id, pinned)
}
fn organize_repo_impl(
    id: i64,
    collection: String,
    tags: Vec<String>,
    state: &State,
) -> Result<(), String> {
    if tags.len() > 20 || tags.iter().any(|t| t.len() > 50) || collection.len() > 100 {
        return Err("Collection or tags exceed limits".into());
    }
    let tags: Vec<String> = tags
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    state
        .database
        .lock()
        .map_err(err)?
        .execute(
            "UPDATE repos SET collection=?1,tags=?2 WHERE id=?3",
            params![
                if collection.trim().is_empty() {
                    None
                } else {
                    Some(collection.trim())
                },
                serde_json::to_string(&tags).map_err(err)?,
                id
            ],
        )
        .map_err(err)?;
    Ok(())
}
fn refresh_repos_impl(state: &State) -> Result<Vec<Repo>, String> {
    list_repos_impl(state)
}
fn repo_details_impl(id: i64, state: &State) -> Result<Details, String> {
    let path = checked_repo_path(&*state.database.lock().map_err(err)?, id)?;
    let raw = git(&path, &["log", "-35", "--format=%H%x1f%s%x1f%an%x1f%ct"]).unwrap_or_default();
    let commits = raw
        .lines()
        .filter_map(|x| {
            let p: Vec<_> = x.split('\x1f').collect();
            if p.len() != 4 {
                return None;
            }
            Some(Commit {
                hash: p[0].to_string(),
                subject: p[1].into(),
                author: p[2].into(),
                timestamp: p[3].parse().ok()?,
            })
        })
        .collect();
    let changes = gitflow::commit_changes(&path)?;
    let git = gitflow::state(&path)?;
    let branches = branches::list(&path, &git.remotes)?;
    let saved_changes = branches::saved_changes(&path, &git.branch)?;
    Ok(Details {
        commits,
        changes,
        branches,
        saved_changes,
        git,
    })
}
fn parse_changes(raw: &str) -> Vec<Change> {
    let mut entries = raw.split('\0').filter(|s| !s.is_empty());
    let mut changes = Vec::new();
    while let Some(entry) = entries.next() {
        if entry.len() <= 3 || !entry.is_char_boundary(3) {
            continue;
        }
        let x = entry.get(..1).unwrap_or(" ");
        let y = entry.get(1..2).unwrap_or(" ");
        let original_path = if x == "R" || x == "C" || y == "R" || y == "C" {
            entries.next().map(String::from)
        } else {
            None
        };
        changes.push(Change {
            x: x.into(),
            y: y.into(),
            path: entry[3..].into(),
            original_path,
        });
    }
    changes
}
fn repository_diff(path: &Path, file: Option<&str>) -> Result<String, String> {
    if let Some(file) = file {
        if file.is_empty()
            || file.len() > 32768
            || file.contains('\0')
            || Path::new(file).components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
        {
            return Err("Choose a file inside the selected repository".into());
        }
    }
    let mut args = vec![
        "--literal-pathspecs",
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--cached",
        "--",
    ];
    if let Some(file) = file {
        args.push(file);
    }
    let staged = git(path, &args)?;
    // Remove the mode option only; a literal filename may also be --cached.
    args.remove(5);
    let unstaged = git(path, &args)?;
    if staged.len() + unstaged.len() > 4 * 1024 * 1024 {
        return Err("Diff exceeds the display limit. Review it with system Git.".into());
    }
    let mut parts = Vec::new();
    if !staged.is_empty() {
        parts.push(format!("Staged changes\n{staged}"));
    }
    if !unstaged.is_empty() {
        parts.push(format!("Working tree changes\n{unstaged}"));
    }
    Ok(parts.join("\n\n"))
}

fn branch_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.starts_with('-')
        || name.starts_with('/')
        || name.ends_with('/')
        || name.contains("..")
        || name.contains("@{")
        || name.ends_with(".lock")
        || name
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "~^:?*[\\".contains(c))
    {
        return Err("Invalid branch name".into());
    }
    Ok(())
}
fn git_action_impl(
    id: i64,
    action: String,
    args: Vec<String>,
    expected: Option<GitExpectation>,
    state: &State,
) -> Result<String, String> {
    let _mutation = gitflow::MUTATIONS.lock().map_err(err)?;
    let path = checked_repo_path(&*state.database.lock().map_err(err)?, id)?;
    if matches!(path.try_exists(), Ok(false)) {
        return Err("The repository folder is missing. Use Locate folder.".into());
    }
    let repository = gitflow::repository_lock(&path)?;
    let _repository_guard = repository.try_lock().map_err(|_| {
        "This repository is fetching in the background. Try again when the fetch finishes."
    })?;
    if let Some(expected) = &expected {
        if let Some(token) = &expected.registration {
            if workflow::registration(&*state.database.lock().map_err(err)?, id)?.as_ref()
                != Some(token)
            {
                return Err(
                    "The repository registration changed. Refresh the repository and try again."
                        .into(),
                );
            }
        }
        if expected
            .path
            .as_deref()
            .is_some_and(|expected| path != Path::new(expected))
        {
            return Err(
                "The repository location changed. Refresh the repository and try again.".into(),
            );
        }
        let current = gitflow::state(&path)?;
        if current.branch != expected.branch || current.head != expected.head {
            return Err(
                "The selected branch or commit changed. Refresh the repository and try again."
                    .into(),
            );
        }
        if matches!(action.as_str(), "commit" | "commit_paths")
            && expected.author.as_ref().is_some_and(|author| {
                repo_actions::effective_author(&path).as_ref() != Some(author)
            })
        {
            return Err(
                "The Git author changed. Review the current author before committing.".into(),
            );
        }
    }
    let mark_fetch = || {
        if let Ok(db) = state.database.lock() {
            let _ = workflow::fetched(&db, id, &path);
        }
    };
    let output = match action.as_str() {
        "fetch" if args.is_empty() => gitflow::fetch(&path).inspect(|_| mark_fetch()),
        "pull" if args.is_empty() => gitflow::pull_recording_fetch(&path, false, mark_fetch),
        "merge_upstream" if args.is_empty() => {
            gitflow::pull_recording_fetch(&path, true, mark_fetch)
        }
        "push" if args.is_empty() => gitflow::push_recording(&path, expected.as_ref(), |head| {
            if let Ok(db) = state.database.lock() {
                if repo_path(&db, id).is_ok_and(|current| current == path) {
                    let _ = workflow::record_publication(&db, id, head);
                }
            }
        }),
        "stage_file" if args.len() == 1 => gitflow::stage_file(&path, &args[0], true),
        "unstage_file" if args.len() == 1 => gitflow::stage_file(&path, &args[0], false),
        "stage_all" if args.is_empty() => {
            if !gitflow::state(&path)?.conflicts.is_empty() {
                return Err("Resolve conflicts before staging all files.".into());
            }
            git(&path, &["add", "-A"])
        }
        "unstage_all" if args.is_empty() => {
            let s = gitflow::state(&path)?;
            if s.operation.is_some() {
                return Err(
                    "Finish or abort the current operation before unstaging all files.".into(),
                );
            }
            if s.has_head {
                git(&path, &["reset", "--mixed", "HEAD"])
            } else {
                git(&path, &["rm", "--cached", "-r", "--", "."])
            }
        }
        "continue_operation" if args.is_empty() => gitflow::continue_operation(&path, false),
        "abort_operation" if args.is_empty() => gitflow::continue_operation(&path, true),
        "revert" if args.len() == 1 => gitflow::revert(&path, &args[0]),
        "resolve_file" if args.len() == 3 => {
            gitflow::resolve_file(&path, &args[0], &args[1], &args[2])
        }
        "resolve_delete" if args.len() == 2 => gitflow::resolve_delete(&path, &args[0], &args[1]),
        "commit"
            if args.len() == 1
                && !args[0].trim().is_empty()
                && args[0].encode_utf16().count() <= 500 =>
        {
            let s = gitflow::state(&path)?;
            if s.operation.is_some() || !s.conflicts.is_empty() {
                return Err("Use Continue to finish the current Git operation.".into());
            }
            if s.staged == 0 {
                return Err("Stage at least one file before committing.".into());
            }
            git(&path, &["-c", "core.editor=true", "commit", "-m", &args[0]])
        }
        "commit_paths"
            if args.len() > 1
                && !args[0].trim().is_empty()
                && args[0].encode_utf16().count() <= 500 =>
        {
            gitflow::commit_paths(&path, &args[0], &args[1..])
        }
        "checkout" if args.len() == 1 => {
            gitflow::require_clean(&gitflow::state(&path)?)?;
            branch_name(&args[0])?;
            branches::switch(&path, &format!("refs/heads/{}", args[0]), "clean")
        }
        "switch_branch" if args.len() == 2 => branches::switch(&path, &args[0], &args[1]),
        "stash_changes" if args.is_empty() => branches::stash(&path),
        "restore_changes" if args.len() == 1 => branches::restore(&path, &args[0]),
        "create_branch" if args.len() == 1 => {
            if gitflow::state(&path)?.operation.is_some() {
                return Err("Finish or abort the current Git operation first.".into());
            }
            branch_name(&args[0])?;
            git(
                &path,
                &[
                    "switch",
                    "--no-overwrite-ignore",
                    "--no-recurse-submodules",
                    "-c",
                    &args[0],
                ],
            )
        }
        "diff" if args.len() <= 1 => repository_diff(&path, args.first().map(String::as_str)),
        _ => Err("Unsupported Git action or invalid arguments".into()),
    }?;
    if action == "diff" {
        return Ok(output);
    }
    let db = state.database.lock().map_err(err)?;
    let description = if args.is_empty() {
        action.clone()
    } else if action == "resolve_file" || action == "resolve_delete" {
        format!("Resolve conflict: {}", args[0])
    } else {
        format!("{}: {}", action, args.join(" "))
    };
    if record(
        &db,
        Some(id),
        &format!("git:{}:{}:{}", id, now(), description),
        "git",
        &format!("Git: {action}"),
        Some(&description),
        now(),
    )
    .is_err()
    {
        return Ok(format!(
            "{output}\nGit action completed, but the local activity entry could not be saved."
        ));
    }
    Ok(output)
}
fn workflow_run_url(raw: &str) -> Option<url::Url> {
    let url = url::Url::parse(raw).ok()?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let segments: Vec<_> = url.path_segments()?.take(6).collect();
    let [owner, repo, "actions", "runs", run] = segments.as_slice() else {
        return None;
    };
    let valid_name = |part: &str| {
        !matches!(part, "" | "." | "..")
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    };
    (valid_name(owner)
        && valid_name(repo)
        && run.bytes().all(|byte| byte.is_ascii_digit())
        && run.parse::<u64>().is_ok_and(|id| id > 0))
    .then_some(url)
}

#[cfg(test)]
#[test]
fn workflow_run_url_only_accepts_github_workflow_runs() {
    let valid = "https://github.com/owner/repo/actions/runs/123";
    assert_eq!(workflow_run_url(valid).unwrap().as_str(), valid);
    for invalid in [
        "https://example.com/owner/repo/actions/runs/123",
        "http://github.com/owner/repo/actions/runs/123",
        "https://github.com/owner/repo/actions/runs/123?tab=overview",
        "https://github.com/owner/repo/actions/runs/123/extra",
        "https://github.com/owner/repo/actions/runs/not-a-number",
        "https://github.com/owner/repo/issues/123",
    ] {
        assert!(workflow_run_url(invalid).is_none(), "{invalid}");
    }
}

fn get_activity_impl(state: &State) -> Result<Vec<Activity>, String> {
    let db = state.database.lock().map_err(err)?;
    let mut stmt=db.prepare("SELECT a.id,a.repo_id,r.name,a.kind,a.title,a.detail,a.occurred_at FROM activity a LEFT JOIN repos r ON r.id=a.repo_id ORDER BY a.occurred_at DESC,a.id DESC LIMIT 250").map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            let kind: String = r.get(3)?;
            let detail: Option<String> = r.get(5)?;
            let can_open_in_browser =
                kind == "ci" && detail.as_deref().and_then(workflow_run_url).is_some();
            Ok(Activity {
                id: r.get(0)?,
                repo_id: r.get(1)?,
                repo_name: r.get(2)?,
                kind,
                title: r.get(4)?,
                detail,
                can_open_in_browser,
                occurred_at: r.get(6)?,
            })
        })
        .map_err(err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(err)
}
fn list_accounts_impl(state: &State) -> Result<Vec<Account>, String> {
    let db = state.database.lock().map_err(err)?;
    let mut stmt = db
        .prepare("SELECT id,login,api_host,created_at FROM accounts ORDER BY id")
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Account {
                id: r.get(0)?,
                login: r.get(1)?,
                api_host: r.get(2)?,
                created_at: r.get(3)?,
            })
        })
        .map_err(err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(err)
}
fn validate_host(raw: &str) -> Result<String, String> {
    let parsed = url::Url::parse(raw).map_err(err)?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("api.github.com")
        || parsed.path() != "/" && !parsed.path().is_empty()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("MVP supports only https://api.github.com".into());
    }
    Ok("https://api.github.com".into())
}
fn gh_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("Reponiq/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(err)
}
async fn gh_get(
    client: &reqwest::Client,
    url: &str,
    token: &str,
) -> Result<reqwest::Response, String> {
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .bearer_auth(token)
        .send()
        .await
        .map_err(err)?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub API returned {}. Check token access, connection, or API rate limits.",
            response.status()
        ));
    }
    Ok(response)
}
#[tauri::command]
async fn connect_account(
    token: String,
    api_host: String,
    state: tauri::State<'_, State>,
) -> Result<Account, String> {
    let _account_guard = oauth::lock_account().await?;
    let host = validate_host(&api_host)?;
    if token.trim().is_empty() || token.len() > 4096 {
        return Err("Enter a valid GitHub token.".into());
    }
    diagnostics::remember_secret(&token);
    let client = gh_client()?;
    let user: GithubUser = gh_get(&client, &format!("{host}/user"), &token)
        .await?
        .json()
        .await
        .map_err(err)?;
    background(state, move |state| {
        save_account(user.login, host, &token, None, state)
    })
    .await
}
fn save_account(
    login: String,
    host: String,
    token: &str,
    oauth_credential: Option<&oauth::Credential>,
    state: &State,
) -> Result<Account, String> {
    let mut connection = state.database.lock().map_err(err)?;
    let db = connection.transaction().map_err(err)?;
    db.execute(
        "INSERT OR IGNORE INTO accounts(login,api_host,created_at) VALUES(?1,?2,?3)",
        params![login, host, now()],
    )
    .map_err(err)?;
    let account = db
        .query_row(
            "SELECT id,login,api_host,created_at FROM accounts WHERE login=?1 AND api_host=?2",
            params![login, host],
            |r| {
                Ok(Account {
                    id: r.get(0)?,
                    login: r.get(1)?,
                    api_host: r.get(2)?,
                    created_at: r.get(3)?,
                })
            },
        )
        .map_err(err)?;
    let access_entry = oauth::entry(state.credential_service, account.id, false)?;
    let metadata_entry = oauth::entry(state.credential_service, account.id, true)?;
    let old_access = oauth::read_secret(&access_entry)?;
    let old_metadata = oauth::read_secret(&metadata_entry)?;
    let metadata = oauth_credential
        .map(|credential| {
            let mut bound = credential.clone();
            bound.bind_account(&account.login, account.created_at);
            serde_json::to_string(&bound)
        })
        .transpose()
        .map_err(err)?;
    let result = (|| {
        oauth::write_secret(&metadata_entry, metadata.as_deref())?;
        oauth::write_secret(&access_entry, Some(token))?;
        db.commit().map_err(err)
    })();
    if result.is_err() {
        let metadata_restored = oauth::write_secret(&metadata_entry, old_metadata.as_deref());
        let access_restored = oauth::write_secret(&access_entry, old_access.as_deref());
        if metadata_restored.is_err() || access_restored.is_err() {
            return Err("Could not finish saving sign-in. Unlock the OS credential store and reconnect this account.".into());
        }
    }
    result.map(|()| account)
}
fn disconnect_account_impl(id: i64, state: &State) -> Result<(), String> {
    let entry = oauth::entry(state.credential_service, id, false)?;
    let metadata = oauth::entry(state.credential_service, id, true)?;
    let previous = oauth::read_secret(&metadata)?;
    oauth::write_secret(&metadata, None)?;
    if let Err(error) = oauth::write_secret(&entry, None) {
        oauth::write_secret(&metadata, previous.as_deref())?;
        return Err(error);
    }
    state
        .database
        .lock()
        .map_err(err)?
        .execute("DELETE FROM accounts WHERE id=?1", [id])
        .map_err(err)?;
    Ok(())
}
#[tauri::command]
async fn sync_account(id: i64, state: tauri::State<'_, State>) -> Result<String, String> {
    let _account_guard = oauth::lock_account().await?;
    let (login, host) = {
        let db = state.database.lock().map_err(err)?;
        db.query_row(
            "SELECT login,api_host FROM accounts WHERE id=?1",
            [id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .map_err(err)?
    };
    validate_host(&host)?;
    let token = oauth::account_token(id, state.clone()).await?;
    let client = gh_client()?;
    let repos = {
        let db = state.database.lock().map_err(err)?;
        let mut stmt = db
            .prepare(
                "SELECT r.id,r.path,w.token,r.remote_url FROM repos r
                      LEFT JOIN workflow_registrations w ON w.repo_id=r.id
                      WHERE r.remote_url IS NOT NULL",
            )
            .map_err(err)?;
        let result = stmt
            .query_map([], |r| {
                Ok((
                    workflow::Target {
                        id: r.get(0)?,
                        path: r.get(1)?,
                        registration: r.get(2)?,
                    },
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        result
    };
    let mut updated = 0;
    for (target, remote) in repos {
        let Some(full) = full_name(&remote) else {
            continue;
        };
        let url = format!("{host}/repos/{full}/actions/runs?per_page=5");
        let response = gh_get(&client, &url, &token).await;
        let response =
            response.map_err(|e| format!("Sync stopped after {updated} repositories: {e}"))?;
        let data: WorkflowRuns = response.json().await.map_err(err)?;
        let latest = data.workflow_runs.first();
        if let Some(run) = latest {
            let mut db = state.database.lock().map_err(err)?;
            let tx = db
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            if !target.is_current_registration(&tx)? {
                continue;
            }
            let changed = tx
                .execute(
                    "UPDATE repos SET ci_status=?1,ci_conclusion=?2 WHERE id=?3 AND remote_url=?4",
                    params![run.status, run.conclusion, target.id, remote],
                )
                .map_err(err)?;
            if changed != 1 {
                continue;
            }
            for item in &data.workflow_runs {
                let timestamp = chrono::DateTime::parse_from_rfc3339(&item.created_at)
                    .map(|x| x.timestamp())
                    .unwrap_or_else(|_| now());
                record(
                    &tx,
                    Some(target.id),
                    &format!("github-run:{}", item.id),
                    "ci",
                    &format!(
                        "{}: {}",
                        item.name.as_deref().unwrap_or("Workflow"),
                        item.conclusion.as_deref().unwrap_or(&item.status)
                    ),
                    Some(&item.html_url),
                    timestamp,
                )?;
            }
            tx.commit().map_err(err)?;
            updated += 1;
        }
    }
    Ok(format!(
        "Checked {updated} repository workflows for @{login}"
    ))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let fontconfig = startup::configure_font_cache();
    diagnostics::init();
    if let Some(path) = fontconfig {
        log::info!(
            "Using AppImage fonts with an isolated cache: {}",
            path.display()
        );
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::Updates::default())
        .manage(updater::QuitHandler::default())
        .manage(oauth::LoginState::default())
        .setup(|app| {
            let storage = storage::resolve(
                &dirs_path().map_err(std::io::Error::other)?,
                cfg!(target_os = "windows"),
            )
            .map_err(std::io::Error::other)?;
            if let Some(parent) = storage.database.parent() {
                diagnostics::attach(&parent.join("logs"));
            }
            log::info!(
                "Opening workspace database at {}",
                storage.database.display()
            );
            let db = initialize(&storage.database).map_err(std::io::Error::other)?;
            app.manage(State {
                database: Arc::new(Mutex::new(db)),
                credential_service: storage.credential_service,
            });
            let show = MenuItem::with_id(app, "show", "Show Reponiq", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Reponiq", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or_else(|| std::io::Error::other("Missing app icon"))?;
            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => updater::request_quit(app),
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::DoubleClick { .. } = event {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;
            log::info!("Workspace, window, and tray initialized");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    updater::request_quit(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            updater::check_app_update,
            updater::app_update_selection,
            updater::app_update_versions,
            updater::set_app_update_selection,
            updater::restart_for_update,
            updater::register_app_quit_handler,
            updater::finish_app_quit,
            diagnostics::diagnostics_snapshot,
            diagnostics::diagnostics_write,
            diagnostics::diagnostics_clear,
            diagnostics::diagnostics_export,
            walkthrough_seen,
            finish_walkthrough,
            catalog::list_online_repos,
            catalog::repository_owners,
            catalog::clone_repository,
            catalog::clone_repository_url,
            catalog::create_repository,
            workspace::workspace_settings,
            appearance::appearance_settings,
            appearance::save_appearance,
            appearance::advance_setup,
            registry_theme::preview_registry_theme,
            launch_settings::fetch_settings,
            launch_settings::save_fetch_settings,
            launch_settings::save_repo_fetch_mode,
            launch_settings::launch_fetch,
            launch_settings::autostart_status,
            launch_settings::set_autostart,
            workflow::workflow_app,
            workflow::save_workflow_app,
            workflow::list_repo_workflow,
            workflow::save_repo_workflow,
            workflow::visit_repo,
            workflow::move_pin,
            workflow::read_workflow_draft,
            workflow::save_workflow_draft,
            repo_actions::repo_context,
            repo_actions::set_git_identity,
            repo_actions::open_repo_location,
            repo_actions::pick_editor,
            repo_actions::inspect_repo_location,
            repo_actions::relink_repo,
            repo_actions::undo_local_commit,
            workspace::save_workspace_settings,
            workspace::discover_repositories,
            workspace::import_repositories,
            list_repos,
            repo_summary,
            add_repo,
            remove_repo,
            rename_repo,
            pin_repo,
            organize_repo,
            assign_collection,
            collection_order,
            manage_collections,
            refresh_repos,
            repo_details,
            git_action,
            get_activity,
            open_activity_in_browser,
            list_accounts,
            gitflow::history_page,
            gitflow::commit_details,
            gitflow::commit_diff,
            gitflow::conflict_details,
            pulls::pull_context,
            pulls::list_pull_requests,
            pulls::pull_request_details,
            pulls::open_pull_check,
            pulls::pull_request_files,
            pulls::pull_request_commits,
            pulls::compare_pull_branches,
            pulls::create_pull_request,
            pulls::review_pull_request,
            pulls::merge_pull_request,
            pick_repo_folder,
            oauth::browser_login_available,
            oauth::start_github_login,
            oauth::open_github_login,
            oauth::poll_github_login,
            oauth::cancel_github_login,
            connect_account,
            disconnect_account,
            sync_account
        ])
        .run(tauri::generate_context!())
        .expect("Reponiq failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_sqlite_includes_the_wal_memory_disclosure_fix() {
        assert!(
            rusqlite::version_number() >= 3_054_000,
            "Expected a patched SQLite bundle, got {}",
            rusqlite::version()
        );
    }

    #[test]
    fn github_remote_formats() {
        assert_eq!(
            full_name("git@github.com:org/repo.git"),
            Some("org/repo".into())
        );
        assert_eq!(
            full_name("https://github.com/org/repo.git"),
            Some("org/repo".into())
        );
        assert_eq!(full_name("https://example.com/org/repo"), None);
        assert_eq!(full_name("git@github.com:org/.."), None);
        assert_eq!(full_name("git@github.com:./repo"), None);
    }
    #[test]
    fn reject_unsafe_branches() {
        for b in ["-foo", "a b", "foo..bar", "foo.lock", "x@{y"] {
            assert!(branch_name(b).is_err())
        }
        assert!(branch_name("feature/test-1").is_ok());
    }
    #[test]
    fn host_validation() {
        assert!(validate_host("https://api.github.com").is_ok());
        assert!(validate_host("http://api.github.com").is_err());
        assert!(validate_host("https://evil.example").is_err());
    }
}

#[cfg(test)]
mod release_tests {
    use super::*;
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let result = Command::new("git")
            .args(["init", "--initial-branch=main"])
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(result.status.success());
        dir
    }
    #[test]
    fn repository_management_preserves_registration_until_folder_removal_succeeds() {
        let repo = fixture();
        let path = root(repo.path().to_str().unwrap()).unwrap();
        let database_dir = tempfile::tempdir().unwrap();
        let db = initialize(&database_dir.path().join("management.sqlite")).unwrap();
        db.execute(
            "INSERT INTO repos(id,path,name,pinned,collection) VALUES(1,?1,'Old name',1,'Projects')",
            [path.to_str().unwrap()],
        )
        .unwrap();
        let target = workflow::Target {
            id: 1,
            path: path.to_str().unwrap().into(),
            registration: workflow::registration(&db, 1).unwrap(),
        };
        let state = State {
            database: std::sync::Arc::new(std::sync::Mutex::new(db)),
            credential_service: "fixture",
        };

        rename_repo_impl(target.clone(), " New name ".into(), &state).unwrap();
        {
            let db = state.database.lock().unwrap();
            let row = db
                .query_row(
                    "SELECT name,pinned,collection FROM repos WHERE id=1",
                    [],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, bool>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    },
                )
                .unwrap();
            assert_eq!(row, ("New name".into(), true, "Projects".into()));
            assert!(target.is_current_registration(&db).unwrap());
            db.execute(
                "INSERT INTO repos(id,path,name) VALUES(2,?1,'Nested')",
                [path.join("nested").to_str().unwrap()],
            )
            .unwrap();
        }

        std::fs::write(path.join("untracked.txt"), "Keep until confirmed").unwrap();
        assert!(remove_repo_impl(target.clone(), true, &state)
            .unwrap_err()
            .contains("another registered repository"));
        assert!(path.join("untracked.txt").is_file());
        let nested = workflow::Target {
            id: 2,
            path: path.join("nested").to_str().unwrap().into(),
            registration: workflow::registration(&state.database.lock().unwrap(), 2).unwrap(),
        };
        assert!(remove_repo_impl(nested.clone(), true, &state).is_err());
        assert!(nested
            .is_current_registration(&state.database.lock().unwrap())
            .unwrap());
        remove_repo_impl(nested, false, &state).unwrap();
        assert!(target
            .is_current_registration(&state.database.lock().unwrap())
            .unwrap());

        remove_repo_impl(target.clone(), true, &state).unwrap();
        assert!(!path.exists());
        assert!(!target
            .is_current_registration(&state.database.lock().unwrap())
            .unwrap());
    }

    #[test]
    fn collection_management_keeps_order_and_repository_metadata() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("collections.sqlite");
        let mut db = initialize(&path).unwrap();
        db.execute_batch(
            "INSERT INTO repos(id,path,name,collection,pinned) VALUES
            (1,'one','One','Alpha',1),(2,'two','Two','Beta',0),(3,'three','Three','Alpha',0);",
        )
        .unwrap();
        assert!(read_collection_order(&db).unwrap().is_empty());
        apply_collection_change(
            &mut db,
            CollectionChange::Reorder {
                names: vec!["Beta".into(), "Alpha".into()],
            },
        )
        .unwrap();
        assert!(apply_collection_change(
            &mut db,
            CollectionChange::Rename {
                current: "Alpha".into(),
                name: "Beta".into(),
            }
        )
        .is_err());
        apply_collection_change(
            &mut db,
            CollectionChange::Rename {
                current: "Alpha".into(),
                name: "Projects".into(),
            },
        )
        .unwrap();
        drop(db);
        let mut db = initialize(&path).unwrap();
        assert_eq!(read_collection_order(&db).unwrap(), ["Beta", "Projects"]);
        apply_collection_change(
            &mut db,
            CollectionChange::Delete {
                current: "Projects".into(),
            },
        )
        .unwrap();
        assert_eq!(read_collection_order(&db).unwrap(), ["Beta"]);
        let row = db
            .query_row(
                "SELECT COUNT(*),SUM(collection IS NULL),SUM(pinned) FROM repos",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(row, (3, 2, 1));
    }

    #[test]
    fn assigning_a_collection_is_atomic_and_preserves_existing_tags() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = initialize(&dir.path().join("test.sqlite")).unwrap();
        db.execute(
            "INSERT INTO repos(id,path,name,collection,tags) VALUES(1,'one','One','Old',?1),(2,'two','Two',NULL,'[]')",
            [r#"["urgent"]"#],
        )
        .unwrap();
        let mut targets = [
            workflow::Target {
                id: 1,
                path: "one".into(),
                registration: None,
            },
            workflow::Target {
                id: 2,
                path: "stale".into(),
                registration: None,
            },
        ];
        let first = |db: &Connection| {
            db.query_row("SELECT collection,tags FROM repos WHERE id=1", [], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
        };
        assert!(assign_collection_impl(&mut db, &targets, "New").is_err());
        assert_eq!(first(&db), ("Old".into(), r#"["urgent"]"#.into()));
        targets[1].path = "two".into();
        assign_collection_impl(&mut db, &targets, " New ").unwrap();
        assert_eq!(first(&db), ("New".into(), r#"["urgent"]"#.into()));
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM repos WHERE collection='New'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }
    #[test]
    fn status_tracks_unborn_dirty_and_missing_repositories() {
        let dir = fixture();
        assert_eq!(status(dir.path()).unwrap(), ("main".into(), 0, 0, 0));
        std::fs::write(dir.path().join("hello.txt"), "hello").unwrap();
        assert_eq!(status(dir.path()).unwrap().1, 1);
        assert!(status(&dir.path().join("missing")).is_err());
    }
    #[test]
    fn shelf_metadata_does_not_wait_for_git_and_status_is_explicit() {
        let dir = tempfile::tempdir().unwrap();
        let db = initialize(&dir.path().join("test.sqlite")).unwrap();
        db.execute(
            "INSERT INTO repos(path,name) VALUES(?1,'missing')",
            [dir.path().join("does-not-exist").to_string_lossy().as_ref()],
        )
        .unwrap();
        let state = State {
            database: Arc::new(Mutex::new(db)),
            credential_service: "reponiq-test",
        };
        let rows = list_repos_impl(&state).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].status_pending);
        assert!(rows[0].status_error.is_none());
        assert!(rows[0].branch.is_empty());
        let summary = repo_summary_impl(rows[0].id, &state).unwrap();
        assert!(summary.status_error.is_some());
    }
    #[test]
    fn remote_credentials_never_enter_storage_or_ui() {
        assert_eq!(
            safe_remote("https://user:dummy-token@github.com/org/repo.git?token=dummy#secret"),
            "https://github.com/org/repo.git"
        );
        assert_eq!(
            safe_remote("git@github.com:org/repo.git"),
            "git@github.com:org/repo.git"
        );
    }
    #[test]
    fn upgrade_preserves_old_schema_unknown_data_and_new_writes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("store.sqlite");
        let old = Connection::open(&file).unwrap();
        old.execute_batch(
            "CREATE TABLE future_data(value TEXT); INSERT INTO future_data VALUES('preserve me');",
        )
        .unwrap();
        drop(old);
        let upgraded = initialize(&file).unwrap();
        upgraded
            .execute(
                "INSERT INTO repos(path,name,tags) VALUES (?1,?2,?3)",
                params!["/fixture", "fixture", "[\"rust\"]"],
            )
            .unwrap();
        assert!(!walkthrough_seen_impl(&upgraded).unwrap());
        finish_walkthrough_impl(&upgraded).unwrap();
        finish_walkthrough_impl(&upgraded).unwrap();
        upgraded
            .execute("INSERT INTO preferences VALUES('future-option','keep')", [])
            .unwrap();
        drop(upgraded);
        // Faithful previous-version queries see subsequent legitimate writes.
        let rollback = Connection::open(&file).unwrap();
        let name: String = rollback
            .query_row("SELECT name FROM repos WHERE path='/fixture'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "fixture");
        let future: String = rollback
            .query_row("SELECT value FROM future_data", [], |r| r.get(0))
            .unwrap();
        assert_eq!(future, "preserve me");
        rollback
            .execute(
                "UPDATE repos SET name='old writer' WHERE path='/fixture'",
                [],
            )
            .unwrap();
        drop(rollback);
        let resumed = initialize(&file).unwrap();
        assert!(walkthrough_seen_impl(&resumed).unwrap());
        assert_eq!(
            resumed
                .query_row(
                    "SELECT value FROM preferences WHERE key='future-option'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "keep"
        );
        resumed
            .execute("UPDATE repos SET pinned=1 WHERE path='/fixture'", [])
            .unwrap();
        drop(resumed);
        assert_eq!(
            initialize(&file)
                .unwrap()
                .query_row("SELECT pinned FROM repos", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn nul_status_preserves_unicode_spaces_and_rename_paths() {
        let changes =
            parse_changes(" M file name.txt\0R  new\nname.txt\0old name.txt\0?? café.txt\0");
        assert_eq!(changes.len(), 3);
        assert_eq!(changes[0].path, "file name.txt");
        assert_eq!(changes[1].path, "new\nname.txt");
        assert_eq!(changes[2].path, "café.txt");
        assert!(parse_changes("bad\0?? \0").is_empty());
    }
    #[test]
    fn commit_paths_includes_untracked_files_and_preserves_excluded_staging() {
        let dir = fixture();
        let path = dir.path();
        git(path, &["config", "user.name", "Fixture"]).unwrap();
        git(path, &["config", "user.email", "test@example.invalid"]).unwrap();
        git(path, &["config", "commit.gpgsign", "false"]).unwrap();
        let dbdir = tempfile::tempdir().unwrap();
        let db = initialize(&dbdir.path().join("test.sqlite")).unwrap();
        db.execute(
            "INSERT INTO repos(path,name) VALUES(?1,'fixture')",
            [path.to_string_lossy().as_ref()],
        )
        .unwrap();
        let state = State {
            database: Arc::new(Mutex::new(db)),
            credential_service: "reponiq-test",
        };
        let commit = |args: &[&str]| {
            git_action_impl(
                1,
                "commit_paths".into(),
                args.iter().map(ToString::to_string).collect(),
                None,
                &state,
            )
        };

        std::fs::write(path.join("initial"), "initial").unwrap();
        std::fs::write(path.join("leave staged"), "staged").unwrap();
        git(path, &["add", "--", "leave staged"]).unwrap();
        commit(&["Initial selection", "initial"]).unwrap();
        assert_eq!(
            git(path, &["ls-tree", "--name-only", "HEAD"]).unwrap(),
            "initial"
        );
        assert_eq!(
            git(path, &["diff", "--cached", "--name-only"]).unwrap(),
            "leave staged"
        );

        std::fs::create_dir(path.join("nested")).unwrap();
        std::fs::write(path.join("nested/new.txt"), "nested").unwrap();
        std::fs::write(path.join("initial"), "updated").unwrap();
        commit(&["All files", "initial", "leave staged", "nested/new.txt"]).unwrap();
        assert!(git(path, &["status", "--porcelain"]).unwrap().is_empty());
        assert_eq!(
            git(path, &["show", "HEAD:nested/new.txt"]).unwrap(),
            "nested"
        );

        git(path, &["mv", "initial", "renamed file"]).unwrap();
        std::fs::write(path.join("--literal"), "new").unwrap();
        assert!(commit(&["Stale selection", "missing"]).is_err());
        git(path, &["config", "commit.gpgsign", "true"]).unwrap();
        git(path, &["config", "gpg.format", "openpgp"]).unwrap();
        git(
            path,
            &["config", "gpg.program", "nonexistent-reponiq-fixture-gpg"],
        )
        .unwrap();
        let before = git(path, &["diff", "--cached"]).unwrap();
        assert!(commit(&["Failed commit", "renamed file", "--literal"]).is_err());
        assert_eq!(git(path, &["diff", "--cached"]).unwrap(), before);
        assert_eq!(
            std::fs::read_to_string(path.join("--literal")).unwrap(),
            "new"
        );
        git(path, &["config", "commit.gpgsign", "false"]).unwrap();
        commit(&["Rename and new file", "renamed file", "--literal"]).unwrap();
        assert!(git(path, &["status", "--porcelain"]).unwrap().is_empty());
        assert_eq!(
            git(path, &["show", "HEAD:renamed file"]).unwrap(),
            "updated"
        );
        assert!(git(path, &["cat-file", "-e", "HEAD:initial"]).is_err());

        std::fs::create_dir(path.join("layout")).unwrap();
        std::fs::write(path.join("layout/item.txt"), "nested").unwrap();
        commit(&["Add layout", "layout/item.txt"]).unwrap();
        std::fs::remove_file(path.join("layout/item.txt")).unwrap();
        std::fs::remove_dir(path.join("layout")).unwrap();
        std::fs::write(path.join("layout"), "replacement").unwrap();
        git(path, &["add", "-A"]).unwrap();
        let before = git(path, &["rev-parse", "HEAD"]).unwrap();
        assert!(commit(&["Incomplete replacement", "layout"])
            .unwrap_err()
            .contains("affected folder"));
        assert_eq!(git(path, &["rev-parse", "HEAD"]).unwrap(), before);
        commit(&["Replace layout", "layout", "layout/item.txt"]).unwrap();
    }
    #[test]
    fn selected_diff_preserves_literal_flag_filename_and_does_not_write_activity() {
        let dir = fixture();
        std::fs::write(dir.path().join("--cached"), "initial\n").unwrap();
        std::fs::write(dir.path().join("other.txt"), "other initial\n").unwrap();
        git(dir.path(), &["add", "--all"]).unwrap();
        git(
            dir.path(),
            &[
                "-c",
                "user.name=Reponiq Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "fixture",
            ],
        )
        .unwrap();
        std::fs::write(dir.path().join("--cached"), "stage selected\n").unwrap();
        git(dir.path(), &["add", "--", "--cached"]).unwrap();
        std::fs::write(dir.path().join("--cached"), "unstaged selected\n").unwrap();
        std::fs::write(dir.path().join("other.txt"), "never include this content\n").unwrap();
        let dbdir = tempfile::tempdir().unwrap();
        let db = initialize(&dbdir.path().join("test.sqlite")).unwrap();
        db.execute(
            "INSERT INTO repos(path,name) VALUES(?1,'fixture')",
            [dir.path().to_string_lossy().as_ref()],
        )
        .unwrap();
        let state = State {
            database: Arc::new(Mutex::new(db)),
            credential_service: "reponiq-test",
        };
        let diff =
            git_action_impl(1, "diff".into(), vec!["--cached".into()], None, &state).unwrap();
        assert!(diff.contains("Staged changes") && diff.contains("Working tree changes"));
        assert!(diff.contains("stage selected") && diff.contains("unstaged selected"));
        assert!(!diff.contains("other.txt") && !diff.contains("never include"));
        let events: i64 = state
            .database
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM activity", [], |r| r.get(0))
            .unwrap();
        assert_eq!(events, 0);
        let stale = GitExpectation {
            branch: "other-branch".into(),
            head: Some("a".repeat(40)),
            path: None,
            author: None,
            registration: None,
        };
        let error =
            git_action_impl(1, "stage_all".into(), vec![], Some(stale), &state).unwrap_err();
        assert!(error.contains("changed"));
        assert!(git(dir.path(), &["diff", "--name-only"])
            .unwrap()
            .contains("other.txt"));

        for invalid in ["../private", "a/../../private", "", "/absolute", "\0"] {
            assert!(repository_diff(dir.path(), Some(invalid)).is_err());
        }
    }
}

fn walkthrough_seen_impl(db: &Connection) -> Result<bool, String> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM preferences WHERE key='walkthrough-v1' AND value='seen')",
        [],
        |row| row.get(0),
    )
    .map_err(|_| "Could not read walkthrough preferences.".into())
}
fn finish_walkthrough_impl(db: &Connection) -> Result<(), String> {
    db.execute("INSERT INTO preferences(key,value) VALUES('walkthrough-v1','seen') ON CONFLICT(key) DO UPDATE SET value=excluded.value", [])
        .map(|_| ())
        .map_err(|_| "Could not save walkthrough preferences. Try again.".into())
}
#[tauri::command]
async fn walkthrough_seen(state: tauri::State<'_, State>) -> Result<bool, String> {
    background(state, |state| {
        walkthrough_seen_impl(&*state.database.lock().map_err(err)?)
    })
    .await
}
#[tauri::command]
async fn finish_walkthrough(state: tauri::State<'_, State>) -> Result<(), String> {
    background(state, |state| {
        finish_walkthrough_impl(&*state.database.lock().map_err(err)?)
    })
    .await
}

static ACTIVE_JOBS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
async fn background<T: Send + 'static>(
    state: tauri::State<'_, State>,
    job: impl FnOnce(&State) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    background_job(state, job, false).await
}
async fn background_job<T: Send + 'static>(
    state: tauri::State<'_, State>,
    job: impl FnOnce(&State) -> Result<T, String> + Send + 'static,
    low_priority: bool,
) -> Result<T, String> {
    use std::sync::atomic::Ordering;
    ACTIVE_JOBS
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
            if n < 32 {
                Some(n + 1)
            } else {
                None
            }
        })
        .map_err(|_| "Workspace is busy. Try again in a moment.".to_string())?;
    static WORKERS: std::sync::LazyLock<Arc<tokio::sync::Semaphore>> =
        std::sync::LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(4)));
    static PASSIVE: std::sync::LazyLock<Arc<tokio::sync::Semaphore>> =
        std::sync::LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(3)));
    struct Admission;
    impl Drop for Admission {
        fn drop(&mut self) {
            ACTIVE_JOBS.fetch_sub(1, Ordering::SeqCst);
        }
    }
    let admission = Admission;
    let update_guard = updater::workspace_job()?;
    // Shelf status and launch fetches leave a worker available for user actions.
    let passive = if low_priority {
        Some(
            PASSIVE
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| "Background workers are unavailable.")?,
        )
    } else {
        None
    };
    let permit = WORKERS
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| "Background workers are unavailable.")?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _admission = admission;
        let _update_guard = update_guard;
        let _passive = passive;
        let _permit = permit;
        let result = job(&state);
        if let Err(error) = &result {
            log::error!(target: "reponiq::worker", "{error}");
        }
        result
    })
    .await
    .map_err(|_| "Background operation could not finish".to_string())?
}
#[tauri::command]
async fn list_repos(state: tauri::State<'_, State>) -> Result<Vec<Repo>, String> {
    background(state, move |state| list_repos_impl(state)).await
}
#[tauri::command]
async fn repo_summary(id: i64, state: tauri::State<'_, State>) -> Result<RepoSummary, String> {
    background_job(state, move |s| repo_summary_impl(id, s), true).await
}
#[tauri::command]
async fn add_repo(path: String, state: tauri::State<'_, State>) -> Result<Repo, String> {
    let prepared = workspace::prepare_import(path).await?;
    background(state, move |state| register_repo(prepared, state)).await
}
#[tauri::command]
async fn remove_repo(
    target: workflow::Target,
    delete_folder: bool,
    state: tauri::State<'_, State>,
) -> Result<(), String> {
    background(state, move |state| {
        remove_repo_impl(target, delete_folder, state)
    })
    .await
}
#[tauri::command]
async fn rename_repo(
    target: workflow::Target,
    name: String,
    state: tauri::State<'_, State>,
) -> Result<(), String> {
    background(state, move |state| rename_repo_impl(target, name, state)).await
}
#[tauri::command]
async fn pin_repo(id: i64, pinned: bool, state: tauri::State<'_, State>) -> Result<(), String> {
    background(state, move |state| pin_repo_impl(id, pinned, state)).await
}
#[tauri::command]
async fn organize_repo(
    id: i64,
    collection: String,
    tags: Vec<String>,
    state: tauri::State<'_, State>,
) -> Result<(), String> {
    background(state, move |state| {
        organize_repo_impl(id, collection, tags, state)
    })
    .await
}
fn assign_collection_impl(
    db: &mut Connection,
    targets: &[workflow::Target],
    collection: &str,
) -> Result<(), String> {
    let name = collection.trim();
    if name.is_empty() || name.len() > 100 || targets.is_empty() {
        return Err("Choose repositories and a collection name of up to 100 bytes.".into());
    }
    let tx = db.transaction().map_err(err)?;
    for target in targets {
        target.verify(&tx)?;
        tx.execute(
            "UPDATE repos SET collection=?1 WHERE id=?2",
            params![name, target.id],
        )
        .map_err(err)?;
    }
    tx.commit().map_err(err)
}

#[tauri::command]
async fn assign_collection(
    targets: Vec<workflow::Target>,
    collection: String,
    state: tauri::State<'_, State>,
) -> Result<(), String> {
    background(state, move |state| {
        let mut db = state.database.lock().map_err(err)?;
        assign_collection_impl(&mut db, &targets, &collection)
    })
    .await
}
const COLLECTION_ORDER_KEY: &str = "workspace-collection-order";

fn read_collection_order(db: &Connection) -> Result<Vec<String>, String> {
    use rusqlite::OptionalExtension;
    let saved: Option<String> = db
        .query_row(
            "SELECT value FROM preferences WHERE key=?1",
            [COLLECTION_ORDER_KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(err)?;
    saved.map_or_else(
        || Ok(Vec::new()),
        |value| serde_json::from_str(&value).map_err(err),
    )
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum CollectionChange {
    Rename { current: String, name: String },
    Delete { current: String },
    Reorder { names: Vec<String> },
}

fn apply_collection_change(
    db: &mut Connection,
    change: CollectionChange,
) -> Result<Vec<String>, String> {
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let mut order = read_collection_order(&tx)?;
    let exists = |name: &str| {
        tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM repos WHERE collection=?1)",
            [name],
            |row| row.get::<_, bool>(0),
        )
        .map_err(err)
    };
    match change {
        CollectionChange::Rename { current, name } => {
            let name = name.trim();
            if name.is_empty() || name.len() > 100 || name.chars().any(char::is_control) {
                return Err("Enter a collection name of up to 100 bytes.".into());
            }
            if !exists(&current)? {
                return Err("The collection no longer exists.".into());
            }
            if current != name && exists(name)? {
                return Err("A collection already has that name.".into());
            }
            tx.execute(
                "UPDATE repos SET collection=?1 WHERE collection=?2",
                params![name, current],
            )
            .map_err(err)?;
            order
                .iter_mut()
                .filter(|entry| **entry == current)
                .for_each(|entry| *entry = name.to_owned());
        }
        CollectionChange::Delete { current } => {
            let changed = tx
                .execute(
                    "UPDATE repos SET collection=NULL WHERE collection=?1",
                    [&current],
                )
                .map_err(err)?;
            if changed == 0 {
                return Err("The collection no longer exists.".into());
            }
            order.retain(|name| name != &current);
        }
        CollectionChange::Reorder { names } => order = names,
    }
    let mut seen = std::collections::HashSet::new();
    let mut saved = Vec::new();
    for name in order {
        if exists(&name)? && seen.insert(name.clone()) {
            saved.push(name);
        }
    }
    tx.execute(
        "INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![COLLECTION_ORDER_KEY, serde_json::to_string(&saved).map_err(err)?],
    ).map_err(err)?;
    tx.commit().map_err(err)?;
    Ok(saved)
}

#[tauri::command]
async fn collection_order(state: tauri::State<'_, State>) -> Result<Vec<String>, String> {
    background(state, |state| {
        read_collection_order(&*state.database.lock().map_err(err)?)
    })
    .await
}

#[tauri::command]
async fn manage_collections(
    change: CollectionChange,
    state: tauri::State<'_, State>,
) -> Result<Vec<String>, String> {
    background(state, move |state| {
        apply_collection_change(&mut *state.database.lock().map_err(err)?, change)
    })
    .await
}

#[tauri::command]
async fn refresh_repos(state: tauri::State<'_, State>) -> Result<Vec<Repo>, String> {
    background(state, move |state| refresh_repos_impl(state)).await
}
#[tauri::command]
async fn repo_details(id: i64, state: tauri::State<'_, State>) -> Result<Details, String> {
    background(state, move |state| repo_details_impl(id, state)).await
}
#[tauri::command]
async fn git_action(
    id: i64,
    action: String,
    args: Vec<String>,
    expected: Option<GitExpectation>,
    state: tauri::State<'_, State>,
) -> Result<String, repo_actions::GitFailure> {
    background(state, move |state| {
        git_action_impl(id, action, args, expected, state)
    })
    .await
    .map_err(repo_actions::GitFailure::new)
}
#[tauri::command]
async fn get_activity(state: tauri::State<'_, State>) -> Result<Vec<Activity>, String> {
    background(state, move |state| get_activity_impl(state)).await
}
#[tauri::command]
async fn open_activity_in_browser(
    app: tauri::AppHandle,
    id: i64,
    state: tauri::State<'_, State>,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    background(state, move |state| {
        let detail: Option<String> = state
            .database
            .lock()
            .map_err(err)?
            .query_row(
                "SELECT detail FROM activity WHERE id=?1 AND kind='ci'",
                [id],
                |r| r.get(0),
            )
            .map_err(|_| "This activity no longer has a GitHub run.")?;
        let url = detail
            .as_deref()
            .and_then(workflow_run_url)
            .ok_or("This activity has no valid GitHub run link.")?;
        app.opener()
            .open_url(url.as_str(), None::<&str>)
            .map_err(err)
    })
    .await
}
#[tauri::command]
async fn list_accounts(state: tauri::State<'_, State>) -> Result<Vec<Account>, String> {
    background(state, move |state| list_accounts_impl(state)).await
}
#[tauri::command]
async fn disconnect_account(id: i64, state: tauri::State<'_, State>) -> Result<(), String> {
    let _account_guard = oauth::lock_account().await?;
    background(state, move |state| disconnect_account_impl(id, state)).await
}

#[tauri::command]
async fn pick_repo_folder(
    window: tauri::WebviewWindow,
    purpose: Option<String>,
) -> Result<Option<String>, String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use tauri_plugin_dialog::DialogExt;
    static OPEN: AtomicBool = AtomicBool::new(false);
    if OPEN
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("A folder picker is already open.".into());
    }
    struct DialogGuard;
    impl Drop for DialogGuard {
        fn drop(&mut self) {
            OPEN.store(false, Ordering::Release);
        }
    }
    let _guard = DialogGuard;
    let (send, receive) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title(if purpose.as_deref() == Some("parent") {
            "Choose where to save the repository"
        } else {
            "Select a Git repository"
        })
        .pick_folder(move |folder| {
            let _ = send.send(folder);
        });
    receive
        .await
        .map_err(|_| "The folder picker closed unexpectedly.".to_string())?
        .map(|folder| {
            folder
                .into_path()
                .map_err(|_| "Select a local folder.".to_string())?
                .into_os_string()
                .into_string()
                .map_err(|_| "This folder path cannot be displayed. Enter it manually.".to_string())
        })
        .transpose()
}

fn safe_remote(remote: &str) -> String {
    if let Ok(mut url) = url::Url::parse(remote) {
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_query(None);
        url.set_fragment(None);
        url.to_string()
    } else if remote.contains("://") {
        "[invalid remote URL]".into()
    } else {
        remote.to_string()
    }
}
