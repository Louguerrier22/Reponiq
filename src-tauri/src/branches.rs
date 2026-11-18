use super::{branch_name, git, git_exact, gitflow};
use serde::Serialize;
use std::{collections::HashSet, fs, io::ErrorKind, path::Path};

const STASH_MESSAGE: &str = "Reponiq: saved before switching branches";

#[derive(Serialize)]
pub(super) struct Branch {
    reference: String,
    name: String,
    remote: Option<String>,
    in_worktree: bool,
}

#[derive(Serialize)]
pub(super) struct SavedChanges {
    pub oid: String,
    timestamp: i64,
}

pub(super) fn list(path: &Path, remotes: &[String]) -> Result<Vec<Branch>, String> {
    let refs = git_exact(
        path,
        &[
            "for-each-ref",
            "--format=%(refname)%09%(symref)%09%(worktreepath)%00",
            "refs/heads",
            "refs/remotes",
        ],
    )?;
    Ok(refs
        .split_terminator('\0')
        .filter_map(|line| {
            let mut fields = line.trim_start_matches('\n').splitn(3, '\t');
            let reference = fields.next()?;
            let symbolic = fields.next().unwrap_or_default();
            let in_worktree = fields
                .next()
                .is_some_and(|path| !path.is_empty() && Path::new(path).is_dir());
            if !symbolic.is_empty() {
                return None;
            }
            let (name, remote) = if let Some(name) = reference.strip_prefix("refs/heads/") {
                (name, None)
            } else {
                let (remote, name) = remote_branch(reference, remotes)?;
                (name, Some(remote.to_owned()))
            };
            Some(Branch {
                reference: reference.into(),
                name: name.into(),
                remote,
                in_worktree,
            })
        })
        .collect())
}

fn remote_branch<'a>(reference: &'a str, remotes: &'a [String]) -> Option<(&'a str, &'a str)> {
    let suffix = reference.strip_prefix("refs/remotes/")?;
    remotes
        .iter()
        .filter_map(|remote| {
            Some((
                remote.as_str(),
                suffix.strip_prefix(remote)?.strip_prefix('/')?,
            ))
        })
        .max_by_key(|(remote, _)| remote.len())
}

pub(super) fn saved_changes(path: &Path, branch: &str) -> Result<Vec<SavedChanges>, String> {
    let message = format!("On {branch}: {STASH_MESSAGE}");
    let mut seen = HashSet::new();
    Ok(git(path, &["stash", "list", "--format=%H%x09%ct%x09%gs"])?
        .lines()
        .filter_map(|line| {
            let (oid, rest) = line.split_once('\t')?;
            let (timestamp, subject) = rest.split_once('\t')?;
            (subject == message && seen.insert(oid.to_owned())).then_some(SavedChanges {
                oid: oid.to_owned(),
                timestamp: timestamp.parse().ok()?,
            })
        })
        .collect())
}

pub(super) fn switch(path: &Path, reference: &str, strategy: &str) -> Result<String, String> {
    let before = gitflow::state(path)?;
    if before.operation.is_some() || !before.conflicts.is_empty() {
        return Err(
            "Finish the current Git operation or resolve conflicts before switching.".into(),
        );
    }
    let target = list(path, &before.remotes)?
        .into_iter()
        .find(|branch| branch.reference == reference)
        .ok_or("This branch is no longer available. Fetch branches or refresh and try again.")?;
    branch_name(&target.name)?;
    if target.remote.is_none() && target.name == before.branch {
        return Ok(format!("Already on {}.", target.name));
    }
    if target.remote.is_some()
        && git(
            path,
            &[
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{}", target.name),
            ],
        )
        .is_ok()
    {
        return Err("A local branch with this name already exists. Select the local branch, or use Git to create a differently named tracking branch.".into());
    }
    if !matches!(strategy, "clean" | "bring" | "stash") {
        return Err("Choose how to handle your changes before switching.".into());
    }
    if before.dirty > 0 && strategy == "clean" {
        return Err(
            "The working files changed. Choose how to handle your edits before switching.".into(),
        );
    }
    let stashed = before.dirty > 0 && strategy == "stash";
    if stashed {
        gitflow::require_clean_head(&before)?;
        protect_replaced_tracked_paths(path)?;
        git(path, &["stash", "push", "--include-untracked", "--message", STASH_MESSAGE])
            .map_err(|e| format!("Could not finish saving changes. Check Changes and saved stashes before retrying. {e}"))?;
        if gitflow::state(path)?.dirty != 0 {
            return Err("Some changes could not be stashed. Check Changes and saved stashes before switching.".into());
        }
    }
    let options = [
        "switch",
        "--no-guess",
        "--no-overwrite-ignore",
        "--no-recurse-submodules",
    ];
    let mut args = options.to_vec();
    if target.remote.is_some() {
        args.extend(["--track", "-c", &target.name, "--", &target.reference]);
    } else {
        args.extend(["--", &target.name]);
    }
    git(path, &args).map_err(|error| {
        if stashed {
            format!("Changes were saved on {}. Switching to {} failed. Choose Restore changes to recover them. {error}", before.branch, target.name)
        } else if before.dirty > 0 {
            format!("Could not bring changes to {}. Stay here, commit, or try Stash and switch. {error}", target.name)
        } else {
            error
        }
    })?;
    Ok(if stashed {
        format!(
            "Switched to {}. Your changes are saved on {}.",
            target.name, before.branch
        )
    } else {
        format!("Switched to {}.", target.name)
    })
}

fn protect_replaced_tracked_paths(path: &Path) -> Result<(), String> {
    let deleted = git_exact(
        path,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            "--diff-filter=D",
            "-z",
            "HEAD",
            "--",
        ],
    )?;
    let blocked = "Stashing would overwrite a file or folder that replaced a tracked path. Move it somewhere safe before stashing.";
    for file in deleted.split_terminator('\0') {
        let mut components = file.split('/').peekable();
        let mut current = path.to_owned();
        while let Some(component) = components.next() {
            current.push(component);
            match fs::symlink_metadata(&current) {
                Ok(metadata)
                    if components.peek().is_none() || !metadata.is_dir() || is_link(&metadata) =>
                {
                    return Err(blocked.into())
                }
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => break,
                Err(_) => return Err(
                    "Could not check working files. Inspect the working folder before stashing."
                        .into(),
                ),
            }
        }
    }
    Ok(())
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        return metadata.file_attributes() & 0x400 != 0; // FILE_ATTRIBUTE_REPARSE_POINT
    }
    #[cfg(not(windows))]
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(path: &Path, message: &str) {
        git(
            path,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.test",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                message,
            ],
        )
        .unwrap();
    }

    #[test]
    fn stashing_preserves_ignored_contents_under_a_deleted_tracked_path() {
        let repo = tempfile::tempdir().unwrap();
        let path = repo.path();
        git(path, &["init", "-b", "main"]).unwrap();
        fs::write(path.join(".gitignore"), "local/\n").unwrap();
        fs::write(path.join("local"), "tracked file").unwrap();
        git(path, &["add", "."]).unwrap();
        commit(path, "track initial files");
        git(path, &["branch", "topic"]).unwrap();
        fs::remove_file(path.join("local")).unwrap();
        fs::create_dir(path.join("local")).unwrap();
        fs::write(path.join("local/ignored.txt"), "keep me").unwrap();
        let result = switch(path, "refs/heads/topic", "stash").unwrap_err();
        assert!(result.contains("Stashing would overwrite"), "{result}");
        assert_eq!(
            fs::read_to_string(path.join("local/ignored.txt")).unwrap(),
            "keep me"
        );
        assert!(saved_changes(path, "main").unwrap().is_empty());
    }

    #[test]
    fn linked_worktree_blocks_switch_and_keeps_the_saved_edits() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("repo");
        std::fs::create_dir(&path).unwrap();
        git(&path, &["init", "-b", "main"]).unwrap();
        std::fs::write(path.join("README.md"), "base\n").unwrap();
        git(&path, &["add", "README.md"]).unwrap();
        commit(&path, "initial");
        git(&path, &["branch", "topic"]).unwrap();
        let linked = root.path().join("linked");
        git(
            &path,
            &["worktree", "add", linked.to_str().unwrap(), "topic"],
        )
        .unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &gitflow::repository_lock(&path).unwrap(),
            &gitflow::repository_lock(&linked).unwrap()
        ));
        std::fs::write(path.join("README.md"), "work in progress\n").unwrap();
        let error = switch(&path, "refs/heads/topic", "stash").unwrap_err();
        assert!(
            error.contains("Changes were saved") && error.contains("another worktree"),
            "{error}"
        );
        assert_eq!(gitflow::state(&path).unwrap().branch, "main");
        assert_eq!(gitflow::state(&path).unwrap().dirty, 0);
        let saved = saved_changes(&path, "main").unwrap();
        std::fs::write(path.join("README.md"), "new base\n").unwrap();
        git(&path, &["add", "README.md"]).unwrap();
        commit(&path, "new base");
        let error = restore(&path, &saved[0].oid).unwrap_err();
        assert!(error.contains("saved copy is still available"), "{error}");
        assert_eq!(saved_changes(&path, "main").unwrap()[0].oid, saved[0].oid);
    }

    #[test]
    fn explicit_remote_owner_is_used_for_duplicate_branch_names() {
        let repo = tempfile::tempdir().unwrap();
        let path = repo.path();
        git(path, &["init", "-b", "main"]).unwrap();
        git(
            path,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.test",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                "initial",
            ],
        )
        .unwrap();
        for remote in ["origin", "fork"] {
            git(
                path,
                &["remote", "add", remote, "https://example.test/repo"],
            )
            .unwrap();
            git(
                path,
                &[
                    "update-ref",
                    &format!("refs/remotes/{remote}/feature/topic"),
                    "HEAD",
                ],
            )
            .unwrap();
            git(
                path,
                &[
                    "symbolic-ref",
                    &format!("refs/remotes/{remote}/HEAD"),
                    &format!("refs/remotes/{remote}/feature/topic"),
                ],
            )
            .unwrap();
        }
        let branches = list(path, &["origin".into(), "fork".into()]).unwrap();
        assert_eq!(
            branches
                .iter()
                .filter(|branch| branch.remote.is_some())
                .count(),
            2
        );
        assert!(switch(path, "refs/remotes/origin/missing", "clean").is_err());
        switch(path, "refs/remotes/fork/feature/topic", "clean").unwrap();
        assert_eq!(
            git(path, &["config", "--get", "branch.feature/topic.remote"]).unwrap(),
            "fork"
        );
        assert_eq!(gitflow::state(path).unwrap().branch, "feature/topic");
    }

    #[test]
    fn switch_from_detached_head_keeps_an_ignored_file_that_would_be_overwritten() {
        let repo = tempfile::tempdir().unwrap();
        let path = repo.path();
        git(path, &["init", "-b", "main"]).unwrap();
        std::fs::write(path.join(".gitignore"), "local.txt\n").unwrap();
        git(path, &["add", ".gitignore"]).unwrap();
        commit(path, "ignore local file");
        git(path, &["switch", "-c", "topic"]).unwrap();
        std::fs::write(path.join("local.txt"), "tracked on topic").unwrap();
        git(path, &["add", "-f", "local.txt"]).unwrap();
        commit(path, "track that file on topic");
        git(path, &["switch", "--detach", "main"]).unwrap();
        std::fs::write(path.join("local.txt"), "keep my ignored copy").unwrap();
        let error = switch(path, "refs/heads/topic", "clean").unwrap_err();
        assert!(error.contains("would overwrite"), "{error}");
        assert_eq!(
            std::fs::read_to_string(path.join("local.txt")).unwrap(),
            "keep my ignored copy"
        );
        switch(path, "refs/heads/main", "clean").unwrap();
        assert_eq!(gitflow::state(path).unwrap().branch, "main");
    }

    #[test]
    fn restore_refuses_to_overwrite_an_ignored_working_copy() {
        let repo = tempfile::tempdir().unwrap();
        let path = repo.path();
        git(path, &["init", "-b", "main"]).unwrap();
        std::fs::write(path.join(".gitignore"), "local.txt\n").unwrap();
        git(path, &["add", ".gitignore"]).unwrap();
        commit(path, "ignore local file");
        git(path, &["branch", "topic"]).unwrap();
        std::fs::write(path.join("local.txt"), "staged for Git").unwrap();
        git(path, &["add", "-f", "local.txt"]).unwrap();
        switch(path, "refs/heads/topic", "stash").unwrap();
        switch(path, "refs/heads/main", "clean").unwrap();
        std::fs::write(path.join("local.txt"), "keep this ignored copy").unwrap();
        let entries = saved_changes(path, "main").unwrap();
        assert!(restore(path, &entries[0].oid)
            .unwrap_err()
            .contains("ignored"));
        assert_eq!(
            std::fs::read_to_string(path.join("local.txt")).unwrap(),
            "keep this ignored copy"
        );
    }
}

pub(super) fn restore(path: &Path, oid: &str) -> Result<String, String> {
    let before = gitflow::state(path)?;
    gitflow::require_clean(&before)?;
    if !saved_changes(path, &before.branch)?
        .iter()
        .any(|entry| entry.oid == oid)
    {
        return Err(
            "This saved entry is no longer available on the current branch. Refresh and try again."
                .into(),
        );
    }
    if restore_would_overwrite_ignored(path, oid)? {
        return Err("Restoring would overwrite ignored files. Move the obstructing files somewhere safe and try again. The saved copy is still available.".into());
    }
    git(path, &["stash", "apply", "--index", oid])
        .map_err(|e| format!("Could not fully restore the changes. Review the working files and resolve any conflicts before retrying. The saved copy is still available. {e}"))?;
    Ok("Restored your changes. The stash is still available as a recovery copy.".into())
}

fn restore_would_overwrite_ignored(path: &Path, oid: &str) -> Result<bool, String> {
    let ignored = git_exact(
        path,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
        ],
    )?;
    if ignored.is_empty() {
        return Ok(false);
    }
    let base = format!("{oid}^1");
    let index = format!("{oid}^2");
    let at_worktree = git_exact(
        path,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            "-z",
            &base,
            oid,
            "--",
        ],
    )?;
    let at_index = git_exact(
        path,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            "-z",
            &base,
            &index,
            "--",
        ],
    )?;
    let ancestry = git(path, &["rev-list", "--parents", "-n", "1", oid])?;
    let untracked = if let Some(tree) = ancestry.split_whitespace().nth(3) {
        git_exact(path, &["ls-tree", "-r", "--name-only", "-z", tree])?
    } else {
        String::new()
    };
    fn parents(path: &str) -> impl Iterator<Item = &str> {
        path.match_indices('/').map(|(i, _)| &path[..i])
    }
    fn comparable(path: &str) -> String {
        let path = path.trim_end_matches('/');
        if cfg!(windows) {
            path.to_lowercase()
        } else {
            path.to_owned()
        }
    }
    let restored: HashSet<_> = [&at_worktree, &at_index, &untracked]
        .into_iter()
        .flat_map(|paths| paths.split_terminator('\0'))
        .map(comparable)
        .collect();
    let restored_parents: HashSet<_> = restored.iter().flat_map(|file| parents(file)).collect();
    Ok(ignored.split_terminator('\0').map(comparable).any(|file| {
        restored.contains(&file)
            || restored_parents.contains(file.as_str())
            || parents(&file).any(|parent| restored.contains(parent))
    }))
}
