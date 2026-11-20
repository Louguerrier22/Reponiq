use super::*;
use std::mem::{offset_of, size_of};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::Child;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{ERROR_BROKEN_PIPE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
use windows_sys::Win32::System::Threading::{
    OpenThread, ResumeThread, CREATE_NO_WINDOW, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME,
};

pub(super) struct Process {
    child: Child,
    job: Option<OwnedHandle>,
}

impl Process {
    pub(super) fn spawn(
        mut command: Command,
        _owner: Option<PipeWriter>,
        _retain_worker_on_cleanup: bool,
    ) -> io::Result<Self> {
        let job = create_job()?;
        // Stable Rust doesn't expose the primary thread handle from spawn.
        // Keep the process suspended while we assign it and open that thread.
        let mut child = command
            .creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED)
            .spawn()?;
        let assigned =
            unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) };
        let result = if assigned == 0 {
            Err(io::Error::last_os_error())
        } else {
            resume_primary_thread(child.id())
        };
        if let Err(error) = result {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok(Self {
            child,
            job: Some(job),
        })
    }

    pub(super) fn exited(&mut self) -> io::Result<bool> {
        Ok(self.child.try_wait()?.is_some())
    }

    pub(super) fn finish(&mut self) -> io::Result<ExitStatus> {
        drop(self.job.take());
        self.child.wait()
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if self.job.is_some() {
            let _ = self.finish();
        }
    }
}

fn create_job() -> io::Result<OwnedHandle> {
    let handle = unsafe { CreateJobObjectW(null(), null()) };
    if handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    let job = unsafe { OwnedHandle::from_raw_handle(handle) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let applied = unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if applied == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(job)
}

fn resume_primary_thread(process_id: u32) -> io::Result<()> {
    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let snapshot = unsafe { OwnedHandle::from_raw_handle(handle) };
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut available = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
    while available != 0 {
        let owner_field_end = offset_of!(THREADENTRY32, th32OwnerProcessID) + size_of::<u32>();
        if entry.dwSize as usize >= owner_field_end && entry.th32OwnerProcessID == process_id {
            let handle = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let thread = unsafe { OwnedHandle::from_raw_handle(handle) };
            return match unsafe { ResumeThread(thread.as_raw_handle()) } {
                1 => Ok(()),
                u32::MAX => Err(io::Error::last_os_error()),
                _ => Err(io::Error::other("Unexpected Git thread suspension state")),
            };
        }
        entry.dwSize = size_of::<THREADENTRY32>() as u32;
        available = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
    }
    Err(io::Error::last_os_error())
}

pub(super) fn prepare_pipe(_: &PipeReader) -> io::Result<()> {
    Ok(())
}

pub(super) fn pipe() -> io::Result<(PipeReader, io::PipeWriter)> {
    let mut read = null_mut();
    let mut write = null_mut();
    // A larger kernel buffer keeps Git moving between readiness checks.
    if unsafe { CreatePipe(&mut read, &mut write, null(), 64 * 1024) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let read = unsafe { OwnedHandle::from_raw_handle(read) };
    let write = unsafe { OwnedHandle::from_raw_handle(write) };
    Ok((read.into(), write.into()))
}

pub(super) fn wait_for_output(_: &PipeReader, _: &PipeReader) -> io::Result<()> {
    std::thread::sleep(Duration::from_millis(5));
    Ok(())
}

pub(super) fn read_ready(reader: &mut PipeReader, buffer: &mut [u8]) -> io::Result<usize> {
    let mut available = 0;
    let result = unsafe {
        PeekNamedPipe(
            reader.as_raw_handle(),
            null_mut(),
            0,
            null_mut(),
            &mut available,
            null_mut(),
        )
    };
    if result == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) {
            return Ok(0);
        }
        return Err(error);
    }
    let length = buffer.len().min(available as usize);
    if length == 0 {
        return Ok(0);
    }
    reader.read(&mut buffer[..length])
}
