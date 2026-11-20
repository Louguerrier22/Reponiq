use cap_fs_ext::DirExt;
use cap_std::fs::{Dir, MetadataExt};
use rusqlite::{params, Connection, OptionalExtension};
use std::io::{ErrorKind, Read};
use std::path::{Component, Path};

const UNSAFE_PATH: &str = "This repository path can be changed by another user or contains an alias. Choose a trusted local folder.";
const MOVED: &str = "The repository folder changed. Use Locate folder to review it again.";

pub(super) fn requires_locate(error: &str) -> bool {
    matches!(error, MOVED | UNSAFE_PATH)
}

pub(super) fn initialize(db: &Connection) -> Result<(), String> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS linux_repository_identity (
        repo_id INTEGER PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
        path TEXT NOT NULL, device TEXT NOT NULL, inode TEXT NOT NULL
    );",
    )
    .map_err(super::err)
}

fn trusted(owner: u32, mode: u32, user: u32, ancestor: bool) -> bool {
    (owner == user || owner == 0) && (mode & 0o022 == 0 || (ancestor && mode & 0o1000 != 0))
}

pub(super) fn open_directory(path: &Path) -> Result<Dir, String> {
    walk(path, false)
}

fn walk(path: &Path, ancestor: bool) -> Result<Dir, String> {
    if !path.is_absolute() {
        return Err(UNSAFE_PATH.into());
    }
    let user = unsafe { libc::geteuid() };
    let root = Dir::open_ambient_dir("/", cap_std::ambient_authority()).map_err(super::err)?;
    let metadata = root.dir_metadata().map_err(super::err)?;
    if !trusted(metadata.uid(), metadata.mode(), user, true) {
        return Err(UNSAFE_PATH.into());
    }
    let mut directories = vec![root];
    for part in path.components() {
        let next = match part {
            Component::RootDir | Component::CurDir => continue,
            Component::ParentDir if directories.len() > 1 => {
                directories.pop();
                continue;
            }
            Component::Normal(name) => directories.last().unwrap().open_dir_nofollow(name),
            _ => return Err(UNSAFE_PATH.into()),
        };
        let next = next.map_err(|error| {
            if error.kind() == ErrorKind::NotFound {
                "The repository folder is missing. Use Locate folder."
            } else {
                UNSAFE_PATH
            }
        })?;
        let metadata = next.dir_metadata().map_err(super::err)?;
        if !trusted(metadata.uid(), metadata.mode(), user, true) {
            return Err(UNSAFE_PATH.into());
        }
        directories.push(next);
    }
    let directory = directories.pop().unwrap();
    let metadata = directory.dir_metadata().map_err(super::err)?;
    if !trusted(metadata.uid(), metadata.mode(), user, ancestor) {
        return Err(UNSAFE_PATH.into());
    }
    Ok(directory)
}

fn check_admin_file(directory: &Dir, name: &str) -> Result<bool, String> {
    let metadata = match directory.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(super::err(error)),
    };
    if !metadata.is_file()
        || !trusted(
            metadata.uid(),
            metadata.mode(),
            unsafe { libc::geteuid() },
            false,
        )
    {
        return Err(UNSAFE_PATH.into());
    }
    Ok(true)
}

fn read_admin_file(directory: &Dir, name: &str) -> Result<Option<String>, String> {
    if !check_admin_file(directory, name)? {
        return Ok(None);
    }
    let mut data = String::new();
    directory
        .open(name)
        .map_err(super::err)?
        .take(32769)
        .read_to_string(&mut data)
        .map_err(super::err)?;
    if data.len() > 32768 {
        return Err("Git's repository path is too long.".into());
    }
    Ok(Some(data))
}

pub(super) fn open_for_git(
    path: &Path,
    can_discover: bool,
) -> Result<(Dir, Option<&'static str>), String> {
    let directory = open_directory(path)?;
    if verify_admin(path, &directory)? {
        return Ok((directory, (!can_discover).then_some(".git")));
    }
    if !can_discover {
        // An explicitly selected bare repository has no .git marker.
        if !check_admin_file(&directory, "HEAD")?
            || !directory
                .symlink_metadata("objects")
                .is_ok_and(|metadata| metadata.is_dir())
        {
            return Err(MOVED.into());
        }
        open_directory(&path.join("objects"))?;
        check_admin_file(&directory, "config")?;
        return Ok((directory, Some(".")));
    }
    // The folder picker accepts a subdirectory. Check the same ancestors
    // Git can discover before allowing it to read repository configuration.
    for parent in path.ancestors().skip(1) {
        if verify_admin(parent, &walk(parent, true)?)? {
            break;
        }
    }
    Ok((directory, None))
}

fn verify_admin(path: &Path, directory: &Dir) -> Result<bool, String> {
    let admin = match directory.symlink_metadata(".git") {
        Ok(metadata) if metadata.is_dir() => path.join(".git"),
        Ok(_) => {
            let data = read_admin_file(directory, ".git")?.ok_or(UNSAFE_PATH)?;
            let target = data
                .strip_prefix("gitdir: ")
                .map(|target| target.trim_end_matches(['\r', '\n']))
                .filter(|s| !s.is_empty())
                .ok_or(UNSAFE_PATH)?;
            path.join(target)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(super::err(error)),
    };
    let git_dir = open_directory(&admin)?;
    if !check_admin_file(&git_dir, "HEAD")? {
        return Err(MOVED.into());
    }
    check_admin_file(&git_dir, "config")?;
    check_admin_file(&git_dir, "config.worktree")?;
    if let Some(common) = read_admin_file(&git_dir, "commondir")? {
        let common = common.trim_end();
        if common.is_empty() {
            return Err(UNSAFE_PATH.into());
        }
        let common_path = admin.join(common);
        let common = open_directory(&common_path)?;
        open_directory(&common_path.join("objects"))?;
        check_admin_file(&common, "config")?;
    } else {
        open_directory(&admin.join("objects"))?;
    }
    Ok(true)
}

pub(super) fn check(db: &Connection, id: i64, path: &Path, rebind: bool) -> Result<(), String> {
    let directory = open_directory(path)?;
    if !verify_admin(path, &directory)? {
        return Err(MOVED.into());
    }
    let metadata = directory.dir_metadata().map_err(super::err)?;
    let label = path.to_str().ok_or("The repository path is not UTF-8.")?;
    let identity = (
        label.to_owned(),
        metadata.dev().to_string(),
        metadata.ino().to_string(),
    );
    let saved = db
        .query_row(
            "SELECT path,device,inode FROM linux_repository_identity WHERE repo_id=?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(super::err)?;
    if !rebind && saved.as_ref().is_some_and(|saved| saved != &identity) {
        return Err(MOVED.into());
    }
    if rebind && saved.as_ref() != Some(&identity) {
        db.execute("INSERT INTO linux_repository_identity(repo_id,path,device,inode) VALUES(?1,?2,?3,?4)
            ON CONFLICT(repo_id) DO UPDATE SET path=excluded.path, device=excluded.device, inode=excluded.inode",
            params![id,identity.0,identity.1,identity.2]).map_err(super::err)?;
    } else if saved.is_none() {
        let inserted = db.execute("INSERT OR IGNORE INTO linux_repository_identity(repo_id,path,device,inode) VALUES(?1,?2,?3,?4)",
            params![id, identity.0, identity.1, identity.2]).map_err(super::err)?;
        if inserted != 1 {
            return Err(MOVED.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damaged_nested_repository_does_not_use_the_outer_checkout() {
        let parent = tempfile::tempdir().unwrap();
        let nested = parent.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        for folder in [parent.path(), &nested] {
            super::super::git(folder, &["init", "--initial-branch=main", "--template="]).unwrap();
        }
        assert!(super::super::git(&nested, &["status", "--porcelain"]).is_ok());
        std::fs::write(nested.join(".git/HEAD"), "incomplete\n").unwrap();
        assert!(super::super::git(&nested, &["status", "--porcelain"]).is_err());
        assert!(super::super::git(parent.path(), &["status", "--porcelain"]).is_ok());
    }

    #[test]
    fn ownership_and_permissions_allow_only_trusted_paths() {
        assert!(trusted(1000, 0o755, 1000, false));
        assert!(trusted(0, 0o1777, 1000, true));
        assert!(!trusted(0, 0o1777, 1000, false));
        assert!(!trusted(1000, 0o775, 1000, true));
        assert!(!trusted(1001, 0o755, 1000, true));
    }

    #[test]
    fn changing_folders_requires_explicit_relink_and_leaves_old_rows_readable() {
        let fixture = tempfile::tempdir().unwrap();
        let first = fixture.path().join("first");
        let second = fixture.path().join("second");
        for directory in [&first, &second] {
            std::fs::create_dir_all(directory.join(".git/objects")).unwrap();
            std::fs::write(directory.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        }
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE repos(id INTEGER PRIMARY KEY, path TEXT, name TEXT); INSERT INTO repos VALUES(1,'old-path','repo');").unwrap();
        initialize(&db).unwrap();
        check(&db, 1, &first, false).unwrap();
        check(&db, 1, &first, false).unwrap();
        assert_eq!(check(&db, 1, &second, false).unwrap_err(), MOVED);
        check(&db, 1, &second, true).unwrap();
        check(&db, 1, &second, false).unwrap();
        std::fs::rename(second.join(".git"), second.join("saved-metadata")).unwrap();
        assert_eq!(check(&db, 1, &second, false).unwrap_err(), MOVED);
        assert_eq!(
            db.query_row("SELECT name FROM repos WHERE id=1", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            "repo"
        );
        db.execute("DELETE FROM repos WHERE id=1", []).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM linux_repository_identity",
                [],
                |row| row.get::<_, usize>(0)
            )
            .unwrap(),
            0
        );
    }
}
