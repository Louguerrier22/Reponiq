use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions, Permissions};
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::{Component, Path},
    sync::atomic::{AtomicU64, Ordering},
};
const LIMIT: u64 = 1_048_576;
const UNSAFE: &str = "This conflict cannot be edited safely here. Use your external Git tools.";
/// Every file operation stays relative to this retained parent directory handle.
pub(super) struct ConflictFile {
    dir: Dir,
    name: OsString,
}
impl ConflictFile {
    pub(super) fn open(root: &Path, relative: &str) -> Result<Self, String> {
        let file = Path::new(relative);
        if !file
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err(UNSAFE.into());
        }
        let name = file.file_name().ok_or(UNSAFE)?.to_owned();
        let root = Dir::open_ambient_dir(root, cap_std::ambient_authority()).map_err(|_| UNSAFE)?;
        let parent = file
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let dir = root.open_dir(parent).map_err(|_| UNSAFE)?;
        Ok(Self { dir, name })
    }
    fn snapshot(&self) -> Result<(String, Option<Permissions>), String> {
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        #[cfg(unix)]
        {
            use cap_fs_ext::OpenOptionsSyncExt;
            options.nonblock(true);
        }
        let mut file = match self.dir.open_with(&self.name, &options) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((String::new(), None)),
            Err(_) => return Err(UNSAFE.into()),
        };
        let metadata = file.metadata().map_err(|_| UNSAFE)?;
        if !metadata.is_file() || metadata.len() > LIMIT {
            return Err(UNSAFE.into());
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| UNSAFE)?;
        if bytes.len() > LIMIT as usize || bytes.contains(&0) {
            return Err(UNSAFE.into());
        }
        Ok((
            String::from_utf8(bytes).map_err(|_| UNSAFE)?,
            Some(metadata.permissions()),
        ))
    }
    pub(super) fn read(&self) -> Result<String, String> {
        self.snapshot().map(|s| s.0)
    }
    pub(super) fn delete(&self, expected: &str) -> Result<(), String> {
        if self.read() != Ok(expected.to_owned()) {
            return Err(
                "This file changed after you opened it. Reopen the conflict before deleting it."
                    .into(),
            );
        }
        match self.dir.remove_file(&self.name) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(UNSAFE.into()),
        }
    }
    pub(super) fn replace(&self, content: &str, expected: &str) -> Result<(), String> {
        if content.len() > LIMIT as usize || content.contains('\0') {
            return Err(UNSAFE.into());
        }
        let (current, permissions) = self.snapshot()?;
        if current != expected {
            return Err("This file changed after you opened it. Reopen the conflict to preserve newer edits.".into());
        }
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut created = None;
        for _ in 0..8 {
            let name = format!(
                ".reponiq-resolution-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            );
            match self.dir.open_with(&name, &options) {
                Ok(file) => {
                    created = Some((name, file));
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(UNSAFE.into()),
            }
        }
        let (temporary, mut file) = created.ok_or(UNSAFE)?;
        let result = (|| {
            file.write_all(content.as_bytes()).map_err(|_| UNSAFE)?;
            if let Some(permissions) = permissions {
                file.set_permissions(permissions).map_err(|_| UNSAFE)?;
            }
            file.sync_all().map_err(|_| UNSAFE)?;
            drop(file);
            self.dir
                .rename(&temporary, &self.dir, &self.name)
                .map_err(|_| UNSAFE)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = self.dir.remove_file(&temporary);
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bound_save_preserves_newer_edits_and_replaces_only_selected_file() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("file"), "before").unwrap();
        let file = ConflictFile::open(root.path(), "file").unwrap();
        assert_eq!(file.read().unwrap(), "before");
        std::fs::write(root.path().join("file"), "external").unwrap();
        assert!(file.replace("result", "before").is_err());
        file.replace("result", "external").unwrap();
        assert_eq!(file.read().unwrap(), "result");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn fifo_is_rejected_without_waiting_for_a_writer() {
        let root = tempfile::tempdir().unwrap();
        assert!(std::process::Command::new("mkfifo")
            .arg(root.path().join("pipe"))
            .status()
            .unwrap()
            .success());
        let file = ConflictFile::open(root.path(), "pipe").unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = send.send(file.read());
        });
        assert!(receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("FIFO open must not block")
            .is_err());
    }
    #[cfg(unix)]
    #[test]
    fn swapping_an_ancestor_alias_cannot_redirect_a_retained_conflict_save() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("stable")).unwrap();
        std::fs::write(root.path().join("stable/file"), "before").unwrap();
        std::fs::write(outside.path().join("file"), "private").unwrap();
        symlink("stable", root.path().join("alias")).unwrap();
        let file = ConflictFile::open(root.path(), "alias/file").unwrap();
        std::fs::remove_file(root.path().join("alias")).unwrap();
        symlink(outside.path(), root.path().join("alias")).unwrap();
        file.replace("resolved", "before").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.path().join("stable/file")).unwrap(),
            "resolved"
        );
        assert_eq!(
            std::fs::read_to_string(outside.path().join("file")).unwrap(),
            "private"
        );
        assert!(ConflictFile::open(root.path(), "alias/file").is_err());
        symlink(outside.path().join("file"), root.path().join("leaf")).unwrap();
        assert!(ConflictFile::open(root.path(), "leaf")
            .unwrap()
            .read()
            .is_err());
    }
}
