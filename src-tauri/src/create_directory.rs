use cap_std::fs::Dir;

#[cfg(windows)]
struct LocalAllocation(*mut std::ffi::c_void);

#[cfg(windows)]
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // Both SID text and security descriptors below are LocalAlloc-owned.
            unsafe { windows_sys::Win32::Foundation::LocalFree(self.0) };
        }
    }
}

#[cfg(windows)]
fn private_security_descriptor() -> Result<LocalAllocation, String> {
    use std::{
        mem,
        os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
        ptr,
    };
    use windows_sys::Win32::{
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                SDDL_REVISION_1,
            },
            GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    let fail = || "Could not protect the new repository folder.".to_string();
    let mut raw_token = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) } == 0 {
        return Err(fail());
    }
    if raw_token.is_null() || raw_token as isize == -1 {
        return Err(fail());
    }
    // GetCurrentProcess is borrowed; only the newly opened token is owned.
    let token = unsafe { OwnedHandle::from_raw_handle(raw_token) };
    // TokenUser consists of TOKEN_USER plus its bounded SID. usize provides
    // native pointer alignment; fail closed if this fixed buffer is insufficient.
    let mut storage = [0usize; 128];
    let mut returned = 0u32;
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            storage.as_mut_ptr().cast(),
            mem::size_of_val(&storage) as u32,
            &mut returned,
        )
    } == 0
        || returned < mem::size_of::<TOKEN_USER>() as u32
        || returned as usize > mem::size_of_val(&storage)
    {
        return Err(fail());
    }
    // The successful API wrote a TOKEN_USER into suitably aligned live storage;
    // its SID remains borrowed from that storage throughout the conversion.
    let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
    let mut sid_text = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid_text) } == 0 {
        return Err(fail());
    }
    let sid_owner = LocalAllocation(sid_text.cast());
    if sid_text.is_null() {
        return Err(fail());
    }
    // ConvertSidToStringSidW guarantees an allocated NUL-terminated SID string.
    let mut length = 0usize;
    unsafe {
        while *sid_text.add(length) != 0 {
            length += 1;
        }
    }
    let sid = String::from_utf16(unsafe { std::slice::from_raw_parts(sid_text, length) })
        .map_err(|_| fail())?;
    // P protects against inheriting a shared parent's DACL. OI/CI propagate
    // only these explicit full-access grants to Git-created files/directories.
    let sddl: Vec<u16> = format!("D:P(A;OICI;FA;;;{sid})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut descriptor = ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(fail());
    }
    let descriptor = LocalAllocation(descriptor);
    if descriptor.0.is_null() {
        return Err(fail());
    }
    drop(sid_owner);
    Ok(descriptor)
}
/// Create and retain a new leaf without acquiring an attacker-substituted object.
#[cfg(windows)]
pub(super) fn create(parent: &Dir, name: &str) -> Result<Dir, String> {
    use std::{
        mem,
        os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
        ptr,
    };
    use windows_sys::{
        Wdk::{
            Foundation::OBJECT_ATTRIBUTES,
            Storage::FileSystem::{
                NtCreateFile, FILE_CREATE, FILE_DIRECTORY_FILE, FILE_OPEN_REPARSE_POINT,
                FILE_SYNCHRONOUS_IO_NONALERT,
            },
        },
        Win32::{
            Foundation::{RtlNtStatusToDosError, OBJ_CASE_INSENSITIVE, UNICODE_STRING},
            Storage::FileSystem::{
                FILE_ATTRIBUTE_DIRECTORY, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
                FILE_SHARE_WRITE, FILE_TRAVERSE,
            },
            System::IO::IO_STATUS_BLOCK,
        },
    };
    let mut wide: Vec<u16> = name.encode_utf16().collect();
    if wide.is_empty() || wide.len() > 100 || wide.contains(&0) || name.contains(['/', '\\', ':']) {
        return Err("Choose a simple new folder name.".into());
    }
    let length = (wide.len() * 2) as u16;
    let mut unicode = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: wide.as_mut_ptr(),
    };
    let security = private_security_descriptor()?;
    let attributes = OBJECT_ATTRIBUTES {
        Length: mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &mut unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: security.0.cast(),
        SecurityQualityOfService: ptr::null_mut(),
    };
    let mut handle = ptr::null_mut();
    // All pointers refer to live stack/storage, the operation is synchronous, the
    // name is one validated relative leaf, and FILE_CREATE rejects every collision.
    let mut io: IO_STATUS_BLOCK = unsafe { mem::zeroed() };
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_TRAVERSE,
            &attributes,
            &mut io,
            ptr::null(),
            FILE_ATTRIBUTE_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_CREATE,
            FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT | FILE_OPEN_REPARSE_POINT,
            ptr::null(),
            0,
        )
    };
    if status != 0 {
        return Err(format!("Could not create a new repository folder (OS error {}). Existing folders are preserved.",unsafe{RtlNtStatusToDosError(status)}));
    }
    if handle.is_null() || handle as isize == -1 {
        return Err("Could not retain the new repository folder.".into());
    }
    // NtCreateFile returned a new owned, non-inheritable directory handle. Delete
    // sharing was omitted, as required by Dir::from_std_file on Windows.
    let owned = unsafe { OwnedHandle::from_raw_handle(handle) };
    if io.Information != 2 {
        return Err(
            "Could not verify a newly created folder. Existing folders are preserved.".into(),
        );
    }
    Ok(Dir::from_std_file(std::fs::File::from(owned)))
}
#[cfg(target_os = "linux")]
pub(super) fn create(parent: &Dir, name: &str) -> Result<Dir, String> {
    use cap_fs_ext::DirExt;
    use cap_std::fs::{DirBuilder, DirBuilderExt, MetadataExt};
    use std::os::unix::fs::MetadataExt as StdMetadataExt;
    let metadata = parent
        .dir_metadata()
        .map_err(|_| "Could not check the destination folder.")?;
    let user = std::fs::metadata("/proc/self")
        .map_err(|_| "Could not verify the current user.")?
        .uid();
    if metadata.uid() != user || metadata.mode() & 0o022 != 0 {
        return Err(
            "Choose a repository folder owned by your account that other users cannot modify."
                .into(),
        );
    }
    let mut options = DirBuilder::new();
    options.mode(0o700);
    parent
        .create_dir_with(name, &options)
        .map_err(|_| "The destination exists or cannot be created. Choose a new folder name.")?;
    parent
        .open_dir_nofollow(name)
        .map_err(|_| "Could not retain the new repository folder.".into())
}
#[cfg(not(any(windows, target_os = "linux")))]
pub(super) fn create(_parent: &Dir, _name: &str) -> Result<Dir, String> {
    Err("Repository creation is supported on Windows and Linux.".into())
}
