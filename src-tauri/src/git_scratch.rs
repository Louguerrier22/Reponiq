use std::{ffi::OsString, fs::File, path::Path, process::Command};

const MAX_BYTES: u64 = 1024 * 1024 * 1024;
const INVALID_ROOT: &str = "Managed Git scratch is unavailable. Check the configured local storage volume and restart Reponiq.";

pub(super) struct Scratch {
    // The Windows operation handle must close before TempDir removes it.
    // Keep the volume root open through that cleanup.
    _operation: File,
    directory: tempfile::TempDir,
    _root: File,
}

impl Scratch {
    pub(super) fn path(&self) -> &Path {
        self.directory.path()
    }
}

pub(super) fn prepare(command: &mut Command) -> Result<Option<Scratch>, String> {
    let Some(root) = std::env::var_os("REPONIQ_GIT_SCRATCH_DIR") else {
        return Ok(None);
    };
    let root = Path::new(&root);
    if !root.is_absolute() {
        return Err(INVALID_ROOT.into());
    }
    let canonical = root.canonicalize().map_err(|_| INVALID_ROOT)?;
    let root = git_path(canonical.into_os_string());
    let root = Path::new(&root);
    let (root_handle, mounted) = volume(root).ok_or(INVALID_ROOT)?;
    let mut builder = tempfile::Builder::new();
    builder.prefix("reponiq-git-");
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    let scratch = builder.tempdir_in(root).map_err(|_| {
        "Could not open managed Git scratch. Check its permissions and available space."
    })?;
    let (operation, actual) = volume(scratch.path()).ok_or(INVALID_ROOT)?;
    if actual != mounted {
        return Err(INVALID_ROOT.into());
    }
    command
        .env("TMPDIR", scratch.path())
        .env("TMP", scratch.path())
        .env("TEMP", scratch.path());
    Ok(Some(Scratch {
        _operation: operation,
        directory: scratch,
        _root: root_handle,
    }))
}

#[cfg(windows)]
fn git_path(path: OsString) -> OsString {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let units: Vec<_> = path.encode_wide().collect();
    // Git for Windows helpers expect a conventional drive-letter path.
    let prefix = [b'\\', b'\\', b'?', b'\\'].map(u16::from);
    if units.starts_with(&prefix) && units.get(5) == Some(&u16::from(b':')) {
        return OsString::from_wide(&units[4..]);
    }
    path
}

#[cfg(not(windows))]
fn git_path(path: OsString) -> OsString {
    path
}

#[cfg(target_os = "linux")]
fn volume(path: &Path) -> Option<(File, u64)> {
    use std::os::{fd::AsRawFd, unix::fs::MetadataExt};

    let directory = super::repository_identity::open_directory(path)
        .ok()?
        .into_std_file();
    let metadata = directory.metadata().ok()?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return None;
    }
    let mut raw = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::fstatfs(directory.as_raw_fd(), raw.as_mut_ptr()) } != 0 {
        return None;
    }
    let raw = unsafe { raw.assume_init() };
    let bytes = raw.f_blocks.checked_mul(raw.f_bsize.try_into().ok()?)?;
    (raw.f_type == libc::TMPFS_MAGIC
        && raw.f_files > 0
        && raw.f_files <= 131_072
        && bytes > 0
        && bytes <= MAX_BYTES)
        .then_some((directory, metadata.dev()))
}

#[cfg(windows)]
fn volume(path: &Path) -> Option<(File, u32)> {
    use std::{
        mem::size_of,
        os::windows::{
            fs::{MetadataExt, OpenOptionsExt},
            io::AsRawHandle,
        },
        ptr::null_mut,
    };
    use windows_sys::{
        Wdk::{
            Storage::FileSystem::{
                FileFsDeviceInformation, FileFsFullSizeInformationEx, NtQueryVolumeInformationFile,
            },
            System::SystemServices::{
                FILE_FS_DEVICE_INFORMATION, FILE_FS_FULL_SIZE_INFORMATION_EX, FILE_REMOTE_DEVICE,
            },
        },
        Win32::{
            Storage::FileSystem::{
                GetVolumeInformationByHandleW, FILE_ATTRIBUTE_REPARSE_POINT,
                FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
                FILE_SHARE_WRITE,
            },
            System::IO::IO_STATUS_BLOCK,
        },
    };

    let directory = File::options()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .ok()?;
    let metadata = directory.metadata().ok()?;
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return None;
    }
    let handle = directory.as_raw_handle();
    let mut device = FILE_FS_DEVICE_INFORMATION::default();
    let mut status = IO_STATUS_BLOCK::default();
    let result = unsafe {
        NtQueryVolumeInformationFile(
            handle,
            &mut status,
            (&mut device as *mut FILE_FS_DEVICE_INFORMATION).cast(),
            size_of::<FILE_FS_DEVICE_INFORMATION>() as u32,
            FileFsDeviceInformation,
        )
    };
    if result != 0 || device.Characteristics & FILE_REMOTE_DEVICE != 0 {
        return None;
    }
    let mut size = FILE_FS_FULL_SIZE_INFORMATION_EX::default();
    let result = unsafe {
        NtQueryVolumeInformationFile(
            handle,
            &mut status,
            (&mut size as *mut FILE_FS_FULL_SIZE_INFORMATION_EX).cast(),
            size_of::<FILE_FS_FULL_SIZE_INFORMATION_EX>() as u32,
            FileFsFullSizeInformationEx,
        )
    };
    if result != 0 {
        return None;
    }
    let bytes = size
        .ActualTotalAllocationUnits
        .checked_mul(u64::from(size.SectorsPerAllocationUnit))?
        .checked_mul(u64::from(size.BytesPerSector))?;
    if bytes == 0 || bytes > MAX_BYTES {
        return None;
    }
    let mut serial = 0;
    let queried = unsafe {
        GetVolumeInformationByHandleW(
            handle,
            null_mut(),
            0,
            &mut serial,
            null_mut(),
            null_mut(),
            null_mut(),
            0,
        )
    };
    (queried != 0).then_some((directory, serial))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn volume(_path: &Path) -> Option<(File, ())> {
    None
}

#[cfg(all(test, windows))]
#[test]
fn git_helpers_use_conventional_local_drive_paths() {
    assert_eq!(
        git_path(OsString::from(r"\\?\R:\scratch")),
        OsString::from(r"R:\scratch")
    );
    let unc = OsString::from(r"\\?\UNC\server\share");
    assert_eq!(git_path(unc.clone()), unc);
}
