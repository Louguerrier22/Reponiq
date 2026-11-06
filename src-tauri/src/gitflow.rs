use super::{branch_name, err, git, parse_changes, repo_path, Commit, State};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub(super) static MUTATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Serialize)]
pub struct GitState {
    pub has_head: bool,
    pub branch: String,
    pub upstream: Option<String>,
    pub upstream_gone: bool,
    pub remote: Option<String>,
    pub remote_ref: Option<String>,
    pub remotes: Vec<String>,
    pub operation: Option<String>,
    pub conflicts: Vec<String>,
    pub staged: usize,
    pub dirty: usize,
    pub ahead: i64,
    pub behind: i64,
    pub head: Option<String>,
}
pub(super) fn conflict(x: &str, y: &str) -> bool {
    x == "U" || y == "U" || matches!((x, y), ("A", "A") | ("D", "D"))
}
pub(super) fn state(path: &Path) -> Result<GitState, String> {
    let (branch, _, ahead, behind, head) = super::status_with_head(path)?;
    let raw = git(
        path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=normal"],
    )?;
    let changes = parse_changes(&raw);
    let conflicts = changes
        .iter()
        .filter(|f| conflict(&f.x, &f.y))
        .map(|f| f.path.clone())
        .collect::<Vec<_>>();
    let staged = changes
        .iter()
        .filter(|f| f.x != " " && f.x != "?" && !conflict(&f.x, &f.y))
        .count();
    let remotes = git(path, &["remote"])?
        .lines()
        .map(String::from)
        .collect::<Vec<_>>();
    let tracking = if head.is_some() && branch != "(detached)" {
        git(
            path,
            &[
                "for-each-ref",
                "--format=%(upstream:short)%1f%(upstream:remotename)%1f%(upstream:remoteref)%1f%(upstream:track)",
                &format!("refs/heads/{branch}"),
            ],
        )?
    } else {
        String::new()
    };
    let fields = tracking.split('\x1f').collect::<Vec<_>>();
    let field = |i: usize| {
        fields
            .get(i)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };
    let upstream = field(0);
    let mut remote = field(1)
        .filter(|s| s != ".")
        .or_else(|| remotes.iter().find(|s| s.as_str() == "origin").cloned())
        .or_else(|| {
            if remotes.len() == 1 {
                Some(remotes[0].clone())
            } else {
                None
            }
        });
    if field(1).as_deref() == Some(".") {
        remote = None;
    }
    let directory = PathBuf::from(git(path, &["rev-parse", "--absolute-git-dir"])?);
    let exists = |name: &str| directory.join(name).try_exists().map_err(err);
    let operation = if exists("MERGE_HEAD")? {
        Some("merge")
    } else if exists("rebase-merge")? || exists("rebase-apply")? {
        Some("rebase")
    } else if exists("CHERRY_PICK_HEAD")? {
        Some("cherry-pick")
    } else if exists("REVERT_HEAD")? || exists("sequencer")? {
        Some("revert")
    } else {
        None
    }
    .map(String::from);
    Ok(GitState {
        has_head: head.is_some(),
        head,
        branch,
        upstream,
        upstream_gone: field(3).as_deref() == Some("[gone]"),
        remote,
        remote_ref: field(2),
        remotes,
        operation,
        conflicts,
        staged,
        dirty: changes.len(),
        ahead,
        behind,
    })
}
pub(super) fn require_clean(state: &GitState) -> Result<(), String> {
    if state.operation.is_some() || !state.conflicts.is_empty() {
        return Err("Finish or abort the current Git operation first.".into());
    }
    if state.dirty > 0 {
        return Err("Commit all changes before continuing. Untracked files must also be committed or ignored.".into());
    }
    if !state.has_head {
        return Err("Create your first commit before continuing.".into());
    }
    if state.branch == "(detached)" {
        return Err("Switch to a branch before continuing.".into());
    }
    Ok(())
}
fn remote<'a>(state: &'a GitState) -> Result<&'a str, String> {
    state
        .remote
        .as_deref()
        .filter(|r| !r.starts_with('-') && state.remotes.iter().any(|name| name == r))
        .ok_or("Configure an origin or tracking remote for this branch first.".into())
}
pub(super) fn fetch(path: &Path) -> Result<String, String> {
    let s = state(path)?;
    if s.remotes.is_empty() {
        return Err("This repository has no remote. Add one with Git first.".into());
    }
    git(path, &["fetch", "--all", "--prune"])?;
    Ok("Fetched all remotes. Your working files were not changed.".into())
}
pub(super) fn pull(path: &Path, merge: bool) -> Result<String, String> {
    let s = state(path)?;
    require_clean(&s)?;
    if s.upstream_gone {
        return Err("The upstream branch was deleted. Create a new branch or reconfigure tracking before pulling.".into());
    }
    if s.upstream.is_none() {
        return Err("This branch has no upstream. Publish it first, or configure its tracking branch with Git.".into());
    }
    git(path, &["fetch", "--prune", remote(&s)?])?;
    let s = state(path)?;
    require_clean(&s)?;
    if s.upstream_gone {
        return Err("The upstream branch was deleted. Create a new branch or reconfigure tracking before pulling.".into());
    }
    if s.behind == 0 {
        return Ok("This branch is up to date with its upstream.".into());
    }
    if s.ahead > 0 && !merge {
        return Err("Branches have diverged. Choose Merge upstream to combine them without rewriting history.".into());
    }
    let args = if merge {
        vec!["merge", "--no-edit", "@{upstream}"]
    } else {
        vec!["merge", "--ff-only", "@{upstream}"]
    };
    match git(path, &args) {
        Ok(output) => Ok(output),
        Err(error) => {
            if !state(path)?.conflicts.is_empty() {
                Err("Merge paused: resolve the conflicting files, then continue the merge or abort it.".into())
            } else {
                Err(error)
            }
        }
    }
}
pub(super) fn push(path: &Path) -> Result<String, String> {
    let s = state(path)?;
    require_clean(&s)?;
    if s.upstream_gone {
        return Err("The upstream branch was deleted. Create a new branch before publishing; Reponiq will not recreate it silently.".into());
    }
    let remote = remote(&s)?.to_string();
    git(path, &["fetch", "--prune", &remote])?;
    let s = state(path)?;
    require_clean(&s)?;
    if s.upstream_gone {
        return Err("The upstream branch was deleted. Create a new branch before publishing; Reponiq will not recreate it silently.".into());
    }
    if s.behind > 0 {
        return Err(
            "The remote has changes you do not have. Pull or merge upstream before pushing.".into(),
        );
    }
    branch_name(&s.branch)?;
    let target = s.remote_ref.as_deref().unwrap_or("");
    if s.upstream.is_some() {
        if !target.starts_with("refs/heads/") {
            return Err(
                "This upstream is not a remote branch. Configure a remote tracking branch first."
                    .into(),
            );
        }
        git(path, &["push", "--", &remote, &format!("HEAD:{target}")])?;
    } else {
        git(
            path,
            &[
                "push",
                "--set-upstream",
                "--",
                &remote,
                &format!("{}:refs/heads/{}", s.branch, s.branch),
            ],
        )?;
    }
    Ok(format!("Pushed {} to {remote}.", s.branch))
}
pub(super) fn file_path(file: &str) -> Result<(), String> {
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
        return Err("Choose a file inside the selected repository.".into());
    }
    Ok(())
}
pub(super) fn stage_file(path: &Path, file: &str, stage: bool) -> Result<String, String> {
    file_path(file)?;
    let changes = parse_changes(&git(
        path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=normal"],
    )?);
    let change = changes
        .iter()
        .find(|c| c.path == file)
        .ok_or("This file no longer has changes. Refresh the repository.")?;
    if conflict(&change.x, &change.y) {
        return Err("Resolve the conflict before staging this file.".into());
    }
    let mut names = vec![file];
    if let Some(old) = change.original_path.as_deref() {
        file_path(old)?;
        names.push(old);
    }
    let has_head = git(path, &["rev-parse", "--verify", "HEAD"]).is_ok();
    let mut args = if stage {
        vec!["--literal-pathspecs", "add", "-A", "--"]
    } else if has_head {
        vec!["--literal-pathspecs", "restore", "--staged", "--"]
    } else {
        vec!["--literal-pathspecs", "rm", "--cached", "--"]
    };
    args.extend(names);
    git(path, &args)
}
pub(super) fn continue_operation(path: &Path, abort: bool) -> Result<String, String> {
    let s = state(path)?;
    let op = s
        .operation
        .as_deref()
        .ok_or("No Git operation is in progress.")?;
    if !abort && !s.conflicts.is_empty() {
        return Err("Resolve every conflicting file before continuing.".into());
    }
    let mut args = vec![
        "-c",
        "core.editor=true",
        op,
        if abort { "--abort" } else { "--continue" },
    ];
    if op == "merge" && !abort {
        args = vec!["commit", "--no-edit"];
    }
    git(path, &args)
}
fn valid_hash(hash: &str) -> Result<(), String> {
    if !matches!(hash.len(), 40 | 64) || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        Err("Choose a valid commit.".into())
    } else {
        Ok(())
    }
}
fn parents(path: &Path, hash: &str) -> Result<Vec<String>, String> {
    valid_hash(hash)?;
    if git(path, &["cat-file", "-t", hash])? != "commit" {
        return Err("Choose a commit object.".into());
    }
    Ok(git(path, &["show", "-s", "--format=%P", hash])?
        .split_whitespace()
        .map(String::from)
        .collect())
}
pub(super) fn revert(path: &Path, hash: &str) -> Result<String, String> {
    require_clean(&state(path)?)?;
    let parents = parents(path, hash)?;
    git(path, &["merge-base", "--is-ancestor", hash, "HEAD"])
        .map_err(|_| "This commit is not in the current branch's history.".to_string())?;
    let args = if parents.len() > 1 {
        vec!["revert", "--no-edit", "-m", "1", hash]
    } else {
        vec!["revert", "--no-edit", hash]
    };
    git(path, &args).map_err(|e| {
        if state(path).is_ok_and(|s| !s.conflicts.is_empty()) {
            "Revert paused: resolve the conflicts, then continue or abort the revert.".into()
        } else {
            e
        }
    })
}
#[derive(Serialize)]
pub struct CommitFile {
    path: String,
    status: String,
}
#[derive(Serialize)]
pub struct CommitDetails {
    pub parents: Vec<String>,
    body: String,
    files: Vec<CommitFile>,
    truncated: bool,
}
fn commit_details_impl(path: &Path, hash: &str) -> Result<CommitDetails, String> {
    let parents = parents(path, hash)?;
    let raw = if let Some(parent) = parents.first() {
        git(
            path,
            &[
                "diff",
                "--name-status",
                "-z",
                "--no-renames",
                parent,
                hash,
                "--",
            ],
        )?
    } else {
        git(
            path,
            &[
                "diff-tree",
                "--root",
                "--no-commit-id",
                "--name-status",
                "-z",
                "--no-renames",
                "-r",
                hash,
                "--",
            ],
        )?
    };
    let fields = raw
        .split('\0')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let files = fields
        .chunks_exact(2)
        .take(1000)
        .map(|f| CommitFile {
            status: f[0].into(),
            path: f[1].into(),
        })
        .collect();
    Ok(CommitDetails {
        parents,
        body: git(path, &["show", "-s", "--format=%B", hash])?,
        files,
        truncated: fields.len() / 2 > 1000,
    })
}
#[tauri::command]
pub async fn commit_details(
    id: i64,
    hash: String,
    state: tauri::State<'_, State>,
) -> Result<CommitDetails, String> {
    super::background(state, move |s| {
        commit_details_impl(&repo_path(&*s.database.lock().map_err(err)?, id)?, &hash)
    })
    .await
}
#[tauri::command]
pub async fn commit_diff(
    id: i64,
    hash: String,
    file: Option<String>,
    state: tauri::State<'_, State>,
) -> Result<String, String> {
    super::background(state, move |s| {
        let path = repo_path(&*s.database.lock().map_err(err)?, id)?;
        let parents = parents(&path, &hash)?;
        if let Some(file) = file.as_deref() {
            file_path(file)?;
        }
        let mut args = vec![
            "--literal-pathspecs",
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
        ];
        if let Some(parent) = parents.first() {
            args.extend([parent.as_str(), &hash]);
        } else {
            args = vec![
                "--literal-pathspecs",
                "show",
                "--format=",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                &hash,
            ];
        }
        args.push("--");
        if let Some(file) = file.as_deref() {
            args.push(file);
        }
        git(&path, &args)
    })
    .await
}
#[tauri::command]
pub async fn history_page(
    id: i64,
    skip: usize,
    state: tauri::State<'_, State>,
) -> Result<Vec<Commit>, String> {
    super::background(state, move |s| {
        if skip > 10000 {
            return Err(
                "History browsing is limited to 10,000 commits. Use system Git for older history."
                    .into(),
            );
        }
        let path = repo_path(&*s.database.lock().map_err(err)?, id)?;
        if !state_has_head(&path) {
            return Ok(vec![]);
        }
        let raw = git(
            &path,
            &[
                "log",
                "-50",
                &format!("--skip={skip}"),
                "--format=%H%x1f%s%x1f%an%x1f%ct",
            ],
        )?;
        Ok(raw
            .lines()
            .filter_map(|line| {
                let p = line.split('\x1f').collect::<Vec<_>>();
                if p.len() != 4 {
                    return None;
                }
                Some(Commit {
                    hash: p[0].into(),
                    subject: p[1].into(),
                    author: p[2].into(),
                    timestamp: p[3].parse().ok()?,
                })
            })
            .collect())
    })
    .await
}
fn state_has_head(path: &Path) -> bool {
    git(path, &["rev-parse", "--verify", "HEAD"]).is_ok()
}

#[derive(Serialize)]
pub struct ConflictDetails {
    path: String,
    ours: Option<String>,
    theirs: Option<String>,
    current: String,
}
fn conflict_details_impl(path: &Path, file: &str) -> Result<ConflictDetails, String> {
    file_path(file)?;
    let s = state(path)?;
    if !s.conflicts.iter().any(|p| p == file) {
        return Err("This file is no longer conflicted. Refresh the repository.".into());
    }
    let index = git(
        path,
        &["--literal-pathspecs", "ls-files", "-u", "-z", "--", file],
    )?;
    let stages = index
        .split('\0')
        .filter_map(|line| {
            line.split_once('\t')
                .and_then(|(header, _)| header.split_whitespace().nth(2))
        })
        .collect::<Vec<_>>();
    let text = |stage: &str| -> Result<Option<String>, String> {
        if !stages.contains(&stage) {
            return Ok(None);
        }
        super::git_exact(
            path,
            &[
                "show",
                "--no-ext-diff",
                "--no-textconv",
                &format!(":{stage}:{file}"),
            ],
        )
        .map(Some)
    };
    let ours = text("2")?;
    let theirs = text("3")?;
    let current = super::conflict_file::ConflictFile::open(path, file)?.read()?;
    if ours
        .iter()
        .chain(theirs.iter())
        .chain(std::iter::once(&current))
        .any(|s| s.len() > 1_048_576 || s.contains('\0'))
    {
        return Err("This conflict is binary, too large, or not UTF-8. Resolve it with your external Git tools.".into());
    }
    Ok(ConflictDetails {
        path: file.into(),
        ours,
        theirs,
        current,
    })
}
#[tauri::command]
pub async fn conflict_details(
    id: i64,
    file: String,
    state: tauri::State<'_, State>,
) -> Result<ConflictDetails, String> {
    super::background(state, move |s| {
        conflict_details_impl(&repo_path(&*s.database.lock().map_err(err)?, id)?, &file)
    })
    .await
}
pub(super) fn resolve_file(
    path: &Path,
    file: &str,
    content: &str,
    expected: &str,
) -> Result<String, String> {
    if content.len() > 1_048_576
        || content.contains('\0')
        || content
            .lines()
            .any(|l| l.starts_with("<<<<<<<") || l.starts_with(">>>>>>>"))
    {
        return Err("Remove conflict markers before saving. Text must be under 1 MiB.".into());
    }
    let previous = conflict_details_impl(path, file)?;
    if previous.current != expected {
        return Err("This file changed after you opened it. Reopen the conflict to preserve the newer edits.".into());
    }
    super::conflict_file::ConflictFile::open(path, file)?.replace(content, expected)?;
    git(path, &["--literal-pathspecs", "add", "--", file])?;
    Ok(format!("Resolved and staged {file}."))
}
pub(super) fn resolve_delete(path: &Path, file: &str, expected: &str) -> Result<String, String> {
    let c = conflict_details_impl(path, file)?;
    if c.current != expected {
        return Err(
            "This file changed after you opened it. Reopen the conflict before deleting it.".into(),
        );
    }
    if c.ours.is_some() && c.theirs.is_some() {
        return Err("Both sides contain this file. Resolve its contents instead.".into());
    }
    super::conflict_file::ConflictFile::open(path, file)?.delete(expected)?;
    git(
        path,
        &["--literal-pathspecs", "rm", "--cached", "-f", "--", file],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn configure(path: &Path) {
        git(path, &["config", "user.name", "Reponiq fixture"]).unwrap();
        git(path, &["config", "user.email", "test@example.invalid"]).unwrap();
        git(path, &["config", "commit.gpgsign", "false"]).unwrap();
        git(path, &["config", "core.autocrlf", "false"]).unwrap();
        let hooks = path.join(".fixture-hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        git(path, &["config", "core.hooksPath", hooks.to_str().unwrap()]).unwrap();
        // Keep the empty test-hook directory out of the working tree.
        std::fs::write(path.join(".git/info/exclude"), ".fixture-hooks/\n").unwrap();
    }
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-b", "main"]).unwrap();
        configure(dir.path());
        dir
    }
    fn commit(path: &Path, file: &str, text: &str, message: &str) -> String {
        std::fs::write(path.join(file), text).unwrap();
        git(path, &["--literal-pathspecs", "add", "--", file]).unwrap();
        git(path, &["commit", "-m", message]).unwrap();
        git(path, &["rev-parse", "HEAD"]).unwrap()
    }
    #[test]
    fn deletion_resolution_removes_the_selected_working_file_and_only_stages_its_index_deletion() {
        let dir = fixture();
        let p = dir.path();
        commit(p, "file.txt", "base\n", "base");
        git(p, &["checkout", "-b", "incoming"]).unwrap();
        git(p, &["rm", "file.txt"]).unwrap();
        git(p, &["commit", "-m", "delete incoming"]).unwrap();
        git(p, &["checkout", "main"]).unwrap();
        commit(p, "file.txt", "current\n", "modify current");
        assert!(git(p, &["merge", "incoming"]).is_err());
        let c = conflict_details_impl(p, "file.txt").unwrap();
        resolve_delete(p, "file.txt", &c.current).unwrap();
        assert!(!p.join("file.txt").exists());
        assert!(state(p).unwrap().conflicts.is_empty());
        continue_operation(p, false).unwrap();
        assert!(state(p).unwrap().operation.is_none());
    }
    #[test]
    fn separate_file_commits_and_unborn_unstage_preserve_working_data() {
        let dir = fixture();
        let path = dir.path();
        std::fs::write(path.join("first.txt"), "first\n").unwrap();
        std::fs::write(path.join("--cached"), "second\n").unwrap();
        stage_file(path, "first.txt", true).unwrap();
        assert_eq!(state(path).unwrap().staged, 1);
        stage_file(path, "first.txt", false).unwrap();
        assert!(path.join("first.txt").exists());
        assert_eq!(state(path).unwrap().staged, 0);
        stage_file(path, "first.txt", true).unwrap();
        git(path, &["commit", "-m", "first only"]).unwrap();
        let first = git(path, &["rev-parse", "HEAD"]).unwrap();
        assert_eq!(
            commit_details_impl(path, &first).unwrap().files[0].path,
            "first.txt"
        );
        assert_eq!(state(path).unwrap().dirty, 1);
        stage_file(path, "--cached", true).unwrap();
        git(path, &["commit", "-m", "second only"]).unwrap();
        let second = git(path, &["rev-parse", "HEAD"]).unwrap();
        assert_eq!(
            commit_details_impl(path, &second).unwrap().files[0].path,
            "--cached"
        );
        assert_eq!(state(path).unwrap().dirty, 0);
        assert_eq!(git(path, &["rev-list", "--count", "HEAD"]).unwrap(), "2");
    }
    #[test]
    fn unstaging_a_rename_unstages_both_sides() {
        let dir = fixture();
        let p = dir.path();
        commit(p, "old name.txt", "content\n", "initial");
        git(p, &["mv", "old name.txt", "new name.txt"]).unwrap();
        let changes = parse_changes(&git(p, &["status", "--porcelain=v1", "-z"]).unwrap());
        assert_eq!(changes[0].original_path.as_deref(), Some("old name.txt"));
        stage_file(p, "new name.txt", false).unwrap();
        assert_eq!(state(p).unwrap().staged, 0);
        assert!(p.join("new name.txt").exists());
    }
    #[test]
    fn conflicts_require_resolution_preserve_external_edits_and_finish_with_merge_commit() {
        let dir = fixture();
        let p = dir.path();
        commit(p, "file.txt", "base\n", "base");
        git(p, &["switch", "-c", "feature"]).unwrap();
        commit(p, "file.txt", "incoming\n", "feature");
        git(p, &["switch", "main"]).unwrap();
        commit(p, "file.txt", "current\n", "main");
        assert!(git(p, &["merge", "--no-edit", "feature"]).is_err());
        let s = state(p).unwrap();
        assert_eq!(s.operation.as_deref(), Some("merge"));
        assert_eq!(s.conflicts, vec!["file.txt"]);
        assert!(continue_operation(p, false).is_err());
        let conflict = conflict_details_impl(p, "file.txt").unwrap();
        assert_eq!(conflict.ours.as_deref(), Some("current\n"));
        assert_eq!(conflict.theirs.as_deref(), Some("incoming\n"));
        assert!(resolve_file(p, "file.txt", "<<<<<<< unresolved\n", &conflict.current).is_err());
        std::fs::write(p.join("file.txt"), "external edit\n").unwrap();
        assert!(resolve_file(p, "file.txt", "overwrite\n", &conflict.current).is_err());
        assert_eq!(
            std::fs::read_to_string(p.join("file.txt")).unwrap(),
            "external edit\n"
        );
        resolve_file(p, "file.txt", "both\n", "external edit\n").unwrap();
        assert!(state(p).unwrap().conflicts.is_empty());
        continue_operation(p, false).unwrap();
        assert!(state(p).unwrap().operation.is_none());
        let head = git(p, &["rev-parse", "HEAD"]).unwrap();
        assert_eq!(commit_details_impl(p, &head).unwrap().parents.len(), 2);
        revert(p, &head).unwrap();
        assert_eq!(
            std::fs::read_to_string(p.join("file.txt")).unwrap(),
            "current\n"
        );
        assert_eq!(git(p, &["rev-list", "--count", "HEAD"]).unwrap(), "5");
    }
    #[test]
    fn revert_conflicts_can_be_resolved_and_continued_without_rewriting_history() {
        let dir = fixture();
        let p = dir.path();
        commit(p, "file.txt", "one\n", "one");
        let second = commit(p, "file.txt", "two\n", "two");
        commit(p, "file.txt", "three\n", "three");
        assert!(revert(p, &second).is_err());
        assert_eq!(state(p).unwrap().operation.as_deref(), Some("revert"));
        let c = conflict_details_impl(p, "file.txt").unwrap();
        resolve_file(p, "file.txt", "resolved\n", &c.current).unwrap();
        continue_operation(p, false).unwrap();
        assert_eq!(git(p, &["rev-list", "--count", "HEAD"]).unwrap(), "4");
        assert!(state(p).unwrap().operation.is_none());
    }
    #[test]
    fn push_publish_pull_divergence_and_abort_work_with_a_real_local_remote() {
        let dir = fixture();
        let p = dir.path();
        let remote = tempfile::tempdir().unwrap();
        git(remote.path(), &["init", "--bare"]).unwrap();
        git(remote.path(), &["symbolic-ref", "HEAD", "refs/heads/main"]).unwrap();
        git(
            p,
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
        )
        .unwrap();
        std::fs::write(p.join("file.txt"), "initial\n").unwrap();
        assert!(push(p).unwrap_err().contains("Commit all"));
        commit(p, "file.txt", "initial\n", "initial");
        push(p).unwrap();
        assert_eq!(state(p).unwrap().upstream.as_deref(), Some("origin/main"));
        let holder = tempfile::tempdir().unwrap();
        let clone = holder.path().join("clone");
        git(
            holder.path(),
            &[
                "clone",
                remote.path().to_str().unwrap(),
                clone.to_str().unwrap(),
            ],
        )
        .unwrap();
        configure(&clone);
        commit(&clone, "file.txt", "remote change\n", "remote");
        push(&clone).unwrap();
        assert!(push(p).unwrap_err().contains("remote has changes"));
        pull(p, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(p.join("file.txt")).unwrap(),
            "remote change\n"
        );
        commit(p, "file.txt", "local divergent\n", "local");
        commit(&clone, "file.txt", "remote divergent\n", "remote again");
        push(&clone).unwrap();
        assert!(pull(p, false).unwrap_err().contains("diverged"));
        assert!(pull(p, true).unwrap_err().contains("Merge paused"));
        assert!(!state(p).unwrap().conflicts.is_empty());
        continue_operation(p, true).unwrap();
        assert_eq!(
            std::fs::read_to_string(p.join("file.txt")).unwrap(),
            "local divergent\n"
        );
        assert!(state(p).unwrap().operation.is_none());
        git(remote.path(), &["update-ref", "-d", "refs/heads/main"]).unwrap();
        fetch(p).unwrap();
        assert!(state(p).unwrap().upstream_gone);
        assert!(push(p).unwrap_err().contains("deleted"));
        assert!(pull(p, false).unwrap_err().contains("deleted"));
        assert!(git(remote.path(), &["show-ref", "--verify", "refs/heads/main"]).is_err());
    }
}
