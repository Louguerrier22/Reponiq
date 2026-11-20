use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

const BUSY: &str = "Reponiq is already working with a GitHub account. Try again when it finishes.";
const UNAVAILABLE: &str = "Could not open Reponiq’s GitHub account lock. Check your user folder permissions and try again.";

pub(crate) async fn lock_account() -> Result<File, String> {
    let file = tokio::task::spawn_blocking(open_for_login)
        .await
        .map_err(|_| UNAVAILABLE.to_string())?
        .map_err(|_| UNAVAILABLE.to_string())?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(file),
                Err(TryLockError::WouldBlock) => {
                    tokio::time::sleep(Duration::from_millis(50)).await
                }
                Err(TryLockError::Error(_)) => return Err(UNAVAILABLE.to_string()),
            }
        }
    })
    .await
    .map_err(|_| BUSY.to_string())?
}

fn open_for_login() -> io::Result<File> {
    let home = lock_directory()?;
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        let directory =
            crate::repository_identity::open_directory(&home).map_err(io::Error::other)?;
        return open(
            &PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()))
                .join(".reponiq-credentials.lock"),
        );
    }
    #[cfg(not(target_os = "linux"))]
    open(&home.join(".reponiq-credentials.lock"))
}

fn open(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
            || metadata.nlink() != 1
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
    }
    Ok(file)
}

#[cfg(unix)]
fn lock_directory() -> io::Result<PathBuf> {
    use std::{
        ffi::{CStr, OsStr},
        os::unix::ffi::OsStrExt,
    };
    // The keyring namespace is per login even when XDG_DATA_HOME changes.
    let mut storage = [0u8; 16384];
    let mut account = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut result = std::ptr::null_mut();
    let status = unsafe {
        libc::getpwuid_r(
            libc::geteuid(),
            account.as_mut_ptr(),
            storage.as_mut_ptr().cast(),
            storage.len(),
            &mut result,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status));
    }
    if result.is_null() {
        return Err(io::ErrorKind::NotFound.into());
    }
    let account = unsafe { account.assume_init() };
    if account.pw_dir.is_null() {
        return Err(io::ErrorKind::NotFound.into());
    }
    let path = PathBuf::from(OsStr::from_bytes(
        unsafe { CStr::from_ptr(account.pw_dir) }.to_bytes(),
    ));
    if !path.is_absolute() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Instant;

    #[test]
    fn two_processes_share_a_lock_that_releases_on_termination() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lock");
        let mut second = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "oauth::account_lock::tests::lock_owner",
            ])
            .env("REPONIQ_TEST_ACCOUNT_LOCK", &path)
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !path.with_extension("ready").is_file() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let contender = open(&path).unwrap();
        let blocked = matches!(contender.try_lock(), Err(TryLockError::WouldBlock));
        second.kill().unwrap();
        second.wait().unwrap();
        assert!(blocked);
        contender.try_lock().unwrap();
    }

    #[test]
    #[ignore = "holds a disposable lock in a subprocess"]
    fn lock_owner() {
        let Some(path) = std::env::var_os("REPONIQ_TEST_ACCOUNT_LOCK") else {
            return;
        };
        let path = Path::new(&path);
        let lock = open(path).unwrap();
        lock.try_lock().unwrap();
        std::fs::write(path.with_extension("ready"), []).unwrap();
        std::thread::sleep(Duration::from_secs(10));
    }
}

#[cfg(windows)]
fn lock_directory() -> io::Result<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt, ptr::null_mut};
    use windows_sys::Win32::UI::Shell::{
        SHGetFolderPathW, CSIDL_LOCAL_APPDATA, SHGFP_TYPE_CURRENT,
    };
    let mut path = [0u16; 260];
    let status = unsafe {
        SHGetFolderPathW(
            null_mut(),
            CSIDL_LOCAL_APPDATA as i32,
            null_mut(),
            SHGFP_TYPE_CURRENT as u32,
            path.as_mut_ptr(),
        )
    };
    if status < 0 {
        return Err(io::Error::other("Windows Local AppData is unavailable"));
    }
    let end = path
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(path.len());
    let path = PathBuf::from(OsString::from_wide(&path[..end]));
    if !path.is_absolute() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(path)
}
