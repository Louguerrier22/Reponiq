use super::{
    background, branch_name, err, full_name, git, git_capture_impl, git_exact, gitflow, root,
    safe_remote, workflow, GitExpectation, State,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tauri_plugin_opener::OpenerExt;

#[derive(Serialize)]
pub struct GitFailure {
    message: String,
    code: &'static str,
}
impl GitFailure {
    pub(super) fn new(message: String) -> Self {
        let text = message.to_ascii_lowercase();
        let code = if text.contains("another worktree") {
            "worktree"
        } else if text.contains("upstream branch was deleted") || text.contains("no upstream") {
            "upstream"
        } else if text.contains("authentication")
            || text.contains("authenticate")
            || text.contains("ssh key")
        {
            "auth"
        } else if text.contains("connect to the git remote")
            || text.contains("time or output limit")
        {
            "network"
        } else if text.contains("folder is missing") {
            "missing"
        } else if text.contains("changed. refresh") || text.contains("location changed") {
            "stale"
        } else if text.contains("conflict") {
            "conflict"
        } else {
            "git"
        };
        Self { message, code }
    }
}

#[derive(Serialize)]
pub struct RepoContext {
    default_branch: Option<String>,
    author: Option<String>,
    local_name: Option<String>,
    local_email: Option<String>,
}

pub(super) fn default_branch(path: &Path) -> Option<String> {
    let remotes = git(path, &["remote"]).ok()?;
    let mut names = remotes.lines();
    let first = names.next()?;
    let remote = if remotes.lines().any(|name| name == "origin") {
        "origin"
    } else if names.next().is_none() {
        first
    } else {
        return None;
    };
    let prefix = format!("refs/remotes/{remote}/");
    git(path, &["symbolic-ref", "--quiet", &format!("{prefix}HEAD")])
        .ok()?
        .strip_prefix(&prefix)
        .filter(|name| branch_name(name).is_ok())
        .map(str::to_owned)
}

pub(super) fn effective_author(path: &Path) -> Option<String> {
    git(path, &["var", "GIT_AUTHOR_IDENT"])
        .ok()
        .and_then(|value| {
            value
                .rsplit_once("> ")
                .map(|(ident, _)| format!("{ident}>"))
        })
}

fn context(path: &Path) -> RepoContext {
    RepoContext {
        default_branch: default_branch(path),
        author: effective_author(path),
        local_name: git(path, &["config", "--local", "--get", "user.name"]).ok(),
        local_email: git(path, &["config", "--local", "--get", "user.email"]).ok(),
    }
}

#[tauri::command]
pub async fn repo_context(
    target: workflow::Target,
    state: tauri::State<'_, State>,
) -> Result<RepoContext, String> {
    background(state, move |s| {
        let path = target.verify(&*s.database.lock().map_err(err)?)?;
        Ok(context(&path))
    })
    .await
}

#[derive(Deserialize)]
pub struct Identity {
    name: String,
    email: String,
}

fn write_identity(path: &Path, input: Option<Identity>) -> Result<(), String> {
    let common = gitflow::common_directory(path)?;
    let config = common.join("config");
    if !fs::symlink_metadata(&config)
        .map_err(err)?
        .file_type()
        .is_file()
    {
        return Err("Git's repository configuration is not an ordinary file. Edit it with Git outside Reponiq.".into());
    }
    let lock_path = common.join("config.lock");
    let lock = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|_| "Git configuration is busy. Try again after the other operation finishes.")?;
    struct ConfigLock(PathBuf, Option<fs::File>);
    impl Drop for ConfigLock {
        fn drop(&mut self) {
            drop(self.1.take());
            let _ = fs::remove_file(&self.0);
        }
    }
    let _guard = ConfigLock(lock_path, Some(lock));
    let updated = tempfile::NamedTempFile::new_in(&common)
        .map_err(err)?
        .into_temp_path();
    fs::copy(&config, &updated).map_err(err)?;
    let file = updated
        .to_str()
        .ok_or("The Git config path is not UTF-8.")?;
    #[cfg(windows)]
    let git_path = if let Some(unc) = file.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        file.strip_prefix(r"\\?\").unwrap_or(file).to_owned()
    };
    #[cfg(windows)]
    let file = git_path.as_str();
    let pairs = [
        ("user.name", input.as_ref().map(|value| value.name.as_str())),
        (
            "user.email",
            input.as_ref().map(|value| value.email.as_str()),
        ),
    ];
    for (key, value) in pairs {
        if let Some(value) = value {
            git(
                path,
                &["config", "--file", file, "--replace-all", key, value],
            )?;
        } else if git(path, &["config", "--file", file, "--get-all", key]).is_ok() {
            git(path, &["config", "--file", file, "--unset-all", key])?;
        }
    }
    updated.persist(&config).map_err(err)
}

#[tauri::command]
pub async fn set_git_identity(
    target: workflow::Target,
    input: Option<Identity>,
    state: tauri::State<'_, State>,
) -> Result<RepoContext, String> {
    if let Some(value) = &input {
        if value.name.trim().is_empty()
            || value.name.len() > 200
            || value.email.len() > 254
            || !value.email.split_once('@').is_some_and(|(user, host)| {
                !user.is_empty() && !host.is_empty() && !host.contains('@')
            })
            || value.email.chars().any(char::is_whitespace)
            || [&value.name, &value.email].into_iter().any(|text| {
                text.starts_with('-')
                    || text.contains(['<', '>'])
                    || text.chars().any(char::is_control)
            })
        {
            return Err("Enter a Git author name and email address.".into());
        }
    }
    background(state, move |s| {
        let _mutation = gitflow::MUTATIONS.lock().map_err(err)?;
        let path = target.verify(&*s.database.lock().map_err(err)?)?;
        let repository = gitflow::repository_lock(&path)?;
        let _guard = repository
            .try_lock()
            .map_err(|_| "This repository is busy.")?;
        write_identity(&path, input)?;
        Ok(context(&path))
    })
    .await
}

#[tauri::command]
pub async fn pick_editor(window: tauri::WebviewWindow) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Choose installed editor executable")
        .pick_file(move |result| {
            let _ = sender.send(result);
        });
    receiver
        .await
        .map_err(err)?
        .map(|file| {
            file.into_path().map_err(err).and_then(|path| {
                path.into_os_string()
                    .into_string()
                    .map_err(|_| "The editor path is not UTF-8.".into())
            })
        })
        .transpose()
}

fn worktree(path: &Path, reference: &str) -> Result<PathBuf, String> {
    let output = git_exact(path, &["worktree", "list", "--porcelain", "-z"])?;
    output
        .split("\0\0")
        .find_map(|entry| {
            let mut records = entry.split('\0');
            let folder = records.next()?.strip_prefix("worktree ")?;
            records
                .any(|record| record.strip_prefix("branch ") == Some(reference))
                .then(|| PathBuf::from(folder))
                .filter(|folder| folder.is_absolute() && folder.is_dir())
        })
        .ok_or("Git could not find an existing worktree for that branch.".into())
}

fn terminal(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    let choices = ["System32/cmd.exe"];
    #[cfg(target_os = "linux")]
    let choices = [
        "xdg-terminal-exec",
        "gnome-terminal",
        "konsole",
        "xfce4-terminal",
        "xterm",
    ];
    #[cfg(not(any(windows, target_os = "linux")))]
    let choices: [&str; 0] = [];
    for name in choices {
        #[cfg(windows)]
        let executable = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .map(|folder| folder.join(name));
        #[cfg(not(windows))]
        let executable = std::env::var_os("PATH")
            .into_iter()
            .flat_map(|list| std::env::split_paths(&list).collect::<Vec<_>>())
            .filter(|folder| folder.is_absolute())
            .map(|folder| folder.join(name))
            .find(|file| file.is_file());
        if let Some(executable) = executable {
            let mut command = Command::new(executable);
            command.current_dir(path);
            #[cfg(windows)]
            command.arg("/D");
            #[cfg(target_os = "linux")]
            match name {
                "gnome-terminal" | "xfce4-terminal" => {
                    command.arg("--working-directory").arg(path);
                }
                "konsole" => {
                    command.arg("--workdir").arg(path);
                }
                _ => {}
            }
            if command.spawn().is_ok() {
                return Ok(());
            }
        }
    }
    Err("Could not open a supported terminal. You can open the repository folder instead.".into())
}

#[tauri::command]
pub async fn open_repo_location(
    target: workflow::Target,
    action: String,
    reference: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
) -> Result<(), String> {
    background(state, move |s| {
        let (path,editor)={
            let db=s.database.lock().map_err(err)?;
            let path=target.verify(&db)?;
            let editor=if action=="editor" {
                use rusqlite::OptionalExtension;
                let chosen=db.query_row("SELECT editor FROM repo_workflow WHERE repo_id=?1", [target.id], |row|row.get::<_,Option<String>>(0)).optional().map_err(err)?.flatten();
                chosen.or(workflow::app_preferences(&db)?.editor)
            } else {None};
            (path,editor)
        };
        if !path.is_dir() {return Err("The repository folder is missing. Use Locate folder.".into());}
        match action.as_str() {
            "folder" | "worktree" => {
                let folder = if action=="folder" {path} else {
                    let reference=reference.as_deref().and_then(|r|r.strip_prefix("refs/heads/")).ok_or("Choose a local branch.")?;
                    branch_name(reference)?;
                    worktree(&path,&format!("refs/heads/{reference}"))?
                };
                app.opener().open_path(folder.to_string_lossy().into_owned(), None::<String>).map_err(|_|"Could not open the folder.".into())
            }
            "editor" => {
                let executable=workflow::editor_path(editor)?.ok_or("Choose an editor in Settings, or open the repository folder.")?;
                Command::new(executable).arg(&path).current_dir(&path).spawn().map_err(|_|"Could not launch the selected editor. Choose another editor or open the folder.")?;
                Ok(())
            }
            "terminal" => terminal(&path),
            _ => Err("Unknown repository action.".into()),
        }
    }).await
}

#[derive(Serialize)]
pub struct Location {
    path: String,
    branch: String,
    remote: Option<String>,
    matching: &'static str,
}

fn inspect(target: &workflow::Target, selected: &str, state: &State) -> Result<Location, String> {
    let old = {
        let db = state.database.lock().map_err(err)?;
        target.verify(&db)?;
        db.query_row(
            "SELECT remote_url FROM repos WHERE id=?1",
            [target.id],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(err)?
    };
    let selected = root(selected)?;
    let path = selected
        .to_str()
        .ok_or("The repository path is not UTF-8.")?
        .to_owned();
    let remote = git(&selected, &["remote", "get-url", "origin"])
        .ok()
        .map(|remote| safe_remote(&remote));
    let branch = super::status(&selected)?.0;
    let matching = match (&old, &remote) {
        (Some(old), Some(new))
            if old == new
                || full_name(old)
                    .zip(full_name(new))
                    .is_some_and(|(a, b)| a.eq_ignore_ascii_case(&b)) =>
        {
            "match"
        }
        (Some(_), Some(_)) => "mismatch",
        _ => "unknown",
    };
    let db = state.database.lock().map_err(err)?;
    if db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM repos WHERE path=?1 AND id<>?2)",
            rusqlite::params![path, target.id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(err)?
    {
        return Err("This Git folder is already registered in Reponiq.".into());
    }
    Ok(Location {
        path,
        branch,
        remote,
        matching,
    })
}

#[tauri::command]
pub async fn inspect_repo_location(
    target: workflow::Target,
    path: Option<String>,
    state: tauri::State<'_, State>,
) -> Result<Location, String> {
    background(state, move |s| {
        let selected = match path {
            Some(path) => path,
            None => {
                let db = s.database.lock().map_err(err)?;
                target.verify(&db)?;
                let main: String = db
                    .query_row(
                        "SELECT main_path FROM workflow_worktrees WHERE repo_id=?1",
                        [target.id],
                        |row| row.get(0),
                    )
                    .map_err(|_| {
                        "Git could not identify the main worktree. Choose a folder manually."
                    })?;
                drop(db);
                let main_path = Path::new(&main);
                if gitflow::common_directory(main_path)? != main_path.join(".git") {
                    return Err("The saved main worktree moved. Choose a folder manually.".into());
                }
                main
            }
        };
        inspect(&target, &selected, s)
    })
    .await
}

#[tauri::command]
pub async fn relink_repo(
    target: workflow::Target,
    path: String,
    confirm_unknown: bool,
    state: tauri::State<'_, State>,
) -> Result<Location, String> {
    background(state, move |s| relink(&target, &path, confirm_unknown, s)).await
}

fn relink(
    target: &workflow::Target,
    path: &str,
    confirm_unknown: bool,
    state: &State,
) -> Result<Location, String> {
    let _mutation = gitflow::MUTATIONS.lock().map_err(err)?;
    let result = inspect(target, path, state)?;
    if result.matching == "mismatch" || (result.matching == "unknown" && !confirm_unknown) {
        return Err(
            "Confirm the repository identity or add the folder as a new repository.".into(),
        );
    }
    let db = state.database.lock().map_err(err)?;
    target.verify(&db)?;
    db.execute(
        "UPDATE repos SET path=?1,remote_url=?2 WHERE id=?3 AND path=?4",
        rusqlite::params![result.path, result.remote, target.id, target.path],
    )
    .map_err(err)?;
    Ok(result)
}

fn verified_published(
    path: &Path,
    head: &str,
    remotes: &[String],
    known: &[String],
) -> Result<bool, String> {
    let unavailable =
        "Could not verify that this commit is unpublished. Fetch and try again, or use Revert.";
    if remotes.is_empty() && known.is_empty() {
        return Ok(false);
    }
    if known.len() > 500
        || git(path, &["rev-parse", "--is-shallow-repository"]).as_deref() != Ok("false")
        || git(
            path,
            &[
                "config",
                "--get-regexp",
                r"^remote\..*\.promisor$",
                "^true$",
            ],
        )
        .is_ok_and(|config| !config.is_empty())
    {
        return Err(unavailable.into());
    }
    if !git(
        path,
        &[
            "for-each-ref",
            "--contains",
            head,
            "--format=%(refname)",
            "refs/remotes",
            "refs/tags",
        ],
    )?
    .is_empty()
    {
        return Ok(true);
    }
    let mut seen = HashSet::new();
    let mut tips: HashSet<String> = known.iter().cloned().collect();
    for remote in remotes {
        for flags in [
            &["remote", "get-url", "--all", remote.as_str()][..],
            &["remote", "get-url", "--push", "--all", remote.as_str()][..],
        ] {
            for destination in git(path, flags).map_err(|_| unavailable)?.lines() {
                if !seen.insert(destination.to_owned()) {
                    continue;
                }
                // Limits keep this foreground check bounded on large or unusual repos.
                if seen.len() > 4 {
                    return Err(unavailable.into());
                }
                let refs = git_capture_impl(
                    path,
                    &[
                        "ls-remote",
                        "--",
                        destination,
                        "refs/heads/*",
                        "refs/tags/*",
                    ],
                    true,
                    None,
                    None,
                    true,
                    None,
                )
                .map_err(|_| unavailable)?;
                for line in refs.lines() {
                    let (tip, _) = line.split_once('\t').ok_or(unavailable)?;
                    if !matches!(tip.len(), 40 | 64)
                        || !tip.bytes().all(|ch| ch.is_ascii_hexdigit())
                    {
                        return Err(unavailable.into());
                    }
                    tips.insert(tip.to_owned());
                    if tips.len() > 500 {
                        return Err(unavailable.into());
                    }
                }
            }
        }
    }
    if tips.contains(head) {
        return Ok(true);
    }
    let excluded = format!("^{head}");
    let ancestry = format!("--ancestry-path={head}");
    let mut revisions = vec!["rev-list", "--max-count=1", &ancestry, &excluded];
    let mut commits = Vec::new();
    if tips.is_empty() {
        return Ok(false);
    }
    let request = tips
        .iter()
        .map(|tip| format!("{tip}^{{}}\n"))
        .collect::<String>();
    let objects = git_capture_impl(
        path,
        &["cat-file", "--batch-check=%(objectname) %(objecttype)"],
        true,
        None,
        None,
        false,
        Some(request.as_bytes()),
    )
    .map_err(|_| unavailable)?;
    for line in objects.lines() {
        // A ref absent from the local object database makes ancestry unknowable.
        let (tip, kind) = line.split_once(' ').ok_or(unavailable)?;
        if kind == "missing" || kind == "ambiguous" {
            return Err(unavailable.into());
        }
        if kind == "commit" {
            if tip == head {
                return Ok(true);
            }
            commits.push(tip.to_owned());
        }
    }
    if commits.is_empty() {
        return Ok(false);
    }
    revisions.extend(commits.iter().map(String::as_str));
    Ok(!git(path, &revisions).map_err(|_| unavailable)?.is_empty())
}

fn undo(path: &Path, expected: &GitExpectation, known: &[String]) -> Result<String, String> {
    let before = gitflow::state(path)?;
    gitflow::require_clean(&before)?;
    let head = before
        .head
        .as_deref()
        .ok_or("There is no commit to undo.")?;
    if before.branch != expected.branch || before.head != expected.head {
        return Err("The branch or commit changed. Refresh before undoing.".into());
    }
    let parents = git(path, &["show", "-s", "--format=%P", head])?;
    let mut parents = parents.split_whitespace();
    let parent = parents
        .next()
        .ok_or("The first commit cannot be undone here.")?;
    if parents.next().is_some() {
        return Err("Merge commits cannot be undone here. Use Revert.".into());
    }
    if verified_published(path, head, &before.remotes, known)? {
        return Err("This commit is published or referenced by a tag. Use Revert instead.".into());
    }
    let current = gitflow::state(path)?;
    gitflow::require_clean(&current)?;
    if current.head != before.head
        || current.branch != before.branch
        || current.remotes != before.remotes
    {
        return Err("The repository changed. Refresh before undoing.".into());
    }
    let message = git(path, &["show", "-s", "--format=%B", head])?;
    let reference = format!("refs/heads/{}", before.branch);
    branch_name(&before.branch)?;
    if git(path, &["symbolic-ref", "--quiet", "HEAD"])? != reference {
        return Err("The branch changed. Refresh before undoing.".into());
    }
    // The files ref backend updates HEAD's reflog when its target changes.
    // Verifying HEAD in the same transaction would register a duplicate update.
    // Name the exact branch in the compare-and-swap so it cannot follow a new HEAD.
    let input =
        format!("start\noption no-deref\nupdate {reference} {parent} {head}\nprepare\ncommit\n");
    git_capture_impl(path,&["update-ref","--create-reflog","-m","Reponiq: undo unpublished commit","--stdin"],true,None,None,false,Some(input.as_bytes()))
        .map_err(|_|"Could not safely undo this commit. Refresh Git state and confirm your Git version supports ref transactions.")?;
    Ok(message)
}

#[tauri::command]
pub async fn undo_local_commit(
    target: workflow::Target,
    expected: GitExpectation,
    state: tauri::State<'_, State>,
) -> Result<String, String> {
    background(state, move |s| {
        let _mutation = gitflow::MUTATIONS.lock().map_err(err)?;
        let (path, known) = {
            let db = s.database.lock().map_err(err)?;
            (
                target.verify(&db)?,
                workflow::published_tips(&db, target.id)?,
            )
        };
        let repository = gitflow::repository_lock(&path)?;
        let _guard = repository
            .try_lock()
            .map_err(|_| "This repository is busy.")?;
        undo(&path, &expected, &known)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(path: &Path) -> (String, String) {
        git(path, &["init", "-b", "main"]).unwrap();
        git(path, &["config", "user.name", "Fixture"]).unwrap();
        git(path, &["config", "user.email", "fixture@example.test"]).unwrap();
        fs::write(path.join("work.txt"), "first").unwrap();
        git(path, &["add", "work.txt"]).unwrap();
        git(
            path,
            &["-c", "commit.gpgsign=false", "commit", "-m", "First"],
        )
        .unwrap();
        let first = git(path, &["rev-parse", "HEAD"]).unwrap();
        fs::write(path.join("work.txt"), "second").unwrap();
        git(path, &["add", "work.txt"]).unwrap();
        git(
            path,
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "Keep my message",
            ],
        )
        .unwrap();
        (first, git(path, &["rev-parse", "HEAD"]).unwrap())
    }

    #[test]
    fn undo_moves_only_the_expected_branch_and_preserves_index_and_message() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let (parent, head) = fixture(path);
        let expected = GitExpectation {
            branch: "main".into(),
            head: Some(head.clone()),
            path: None,
            author: None,
            registration: None,
        };
        assert_eq!(undo(path, &expected, &[]).unwrap(), "Keep my message");
        assert_eq!(git(path, &["rev-parse", "HEAD"]).unwrap(), parent);
        assert_eq!(git(path, &["show", ":work.txt"]).unwrap(), "second");
        assert_eq!(fs::read_to_string(path.join("work.txt")).unwrap(), "second");
        assert_eq!(git(path, &["rev-parse", "main@{1}"]).unwrap(), head);
        assert!(undo(path, &expected, &[]).is_err());
    }

    #[test]
    fn undo_refuses_published_tip_and_unreachable_remote() {
        let local = tempfile::tempdir().unwrap();
        let remote = tempfile::tempdir().unwrap();
        let path = local.path();
        let (parent, head) = fixture(path);
        let expected = GitExpectation {
            branch: "main".into(),
            head: Some(head.clone()),
            path: None,
            author: None,
            registration: None,
        };
        git(remote.path(), &["init", "--bare", "-b", "main"]).unwrap();
        git(
            path,
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
        )
        .unwrap();
        git(
            path,
            &["push", "origin", &format!("{parent}:refs/heads/main")],
        )
        .unwrap();
        assert!(!verified_published(path, &head, &["origin".into()], &[]).unwrap());
        git(path, &["push", "origin", "HEAD:refs/heads/main"]).unwrap();
        assert!(undo(path, &expected, &[])
            .unwrap_err()
            .contains("published"));
        git(path, &["remote", "remove", "origin"]).unwrap();
        assert!(undo(path, &expected, std::slice::from_ref(&head))
            .unwrap_err()
            .contains("published"));
        let missing = local.path().join("missing.git");
        git(
            path,
            &["remote", "add", "offline", missing.to_str().unwrap()],
        )
        .unwrap();
        assert!(undo(path, &expected, &[])
            .unwrap_err()
            .contains("Could not verify"));
        assert_eq!(git(path, &["rev-parse", "HEAD"]).unwrap(), head);
    }

    #[test]
    fn repository_identity_updates_pair_and_preserves_other_config() {
        let local = tempfile::tempdir().unwrap();
        let path = local.path();
        fixture(path);
        git(path, &["config", "reponiq.fixture", "keep"]).unwrap();
        write_identity(
            path,
            Some(Identity {
                name: "New Name".into(),
                email: "new@example.test".into(),
            }),
        )
        .unwrap();
        assert_eq!(
            context(path).local_email.as_deref(),
            Some("new@example.test")
        );
        write_identity(path, None).unwrap();
        assert!(context(path).local_name.is_none());
        assert_eq!(git(path, &["config", "reponiq.fixture"]).unwrap(), "keep");
    }

    #[test]
    fn relinking_preserves_the_saved_registration_and_rejects_wrong_remote() {
        let temp = tempfile::tempdir().unwrap();
        let moved = temp.path().join("moved");
        fs::create_dir(&moved).unwrap();
        fixture(&moved);
        git(
            &moved,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/example/another.git",
            ],
        )
        .unwrap();
        let old = temp.path().join("old").to_string_lossy().into_owned();
        let db = super::super::initialize(&temp.path().join("fixture.sqlite")).unwrap();
        db.execute("INSERT INTO repos(id,path,name,pinned,collection,remote_url) VALUES(1,?1,'Original',1,'Work','https://github.com/example/owner.git')",[&old]).unwrap();
        db.execute_batch(
            "INSERT INTO repo_workflow(repo_id,fetch_mode) VALUES(1,'off');
            INSERT INTO workflow_drafts VALUES(1,'main','','','draft','',1);",
        )
        .unwrap();
        let state = State {
            database: std::sync::Arc::new(std::sync::Mutex::new(db)),
            credential_service: "fixture",
        };
        let target = workflow::Target {
            id: 1,
            path: old,
            registration: None,
        };
        assert!(relink(&target, moved.to_str().unwrap(), true, &state).is_err());
        git(
            &moved,
            &[
                "remote",
                "set-url",
                "origin",
                "git@github.com:example/owner.git",
            ],
        )
        .unwrap();
        let result = relink(&target, moved.to_str().unwrap(), false, &state).unwrap();
        let db = state.database.lock().unwrap();
        assert_eq!(
            super::super::repo_path(&db, 1).unwrap(),
            PathBuf::from(result.path)
        );
        assert_eq!(
            workflow::fetch_mode(&db, 1).unwrap(),
            workflow::Override::Off
        );
        assert_eq!(
            db.query_row(
                "SELECT title FROM workflow_drafts WHERE repo_id=1",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "draft"
        );
        assert_eq!(
            db.query_row("SELECT pinned FROM repos WHERE id=1", [], |r| r
                .get::<_, bool>(0))
                .unwrap(),
            true
        );
    }
}
