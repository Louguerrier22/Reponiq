use std::ffi::{OsStr, OsString};
use std::fs::File;
#[cfg(not(test))]
use std::io::PipeWriter;
use std::io::{self, Read, Seek, SeekFrom};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
#[cfg(not(test))]
use std::os::unix::process::CommandExt;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::time::Duration;

const SUPERVISE: &str = "--internal-git-supervisor";
const CHILDREN: &str = "/proc/thread-self/children";

#[cfg(not(test))]
pub(super) fn wrap(command: Command) -> io::Result<(Command, Option<PipeWriter>)> {
    let (reader, owner) = io::pipe()?;
    let mut supervisor = Command::new("/proc/self/exe");
    supervisor
        .arg(SUPERVISE)
        .arg(reader.as_raw_fd().to_string())
        .arg(command.get_program())
        .args(command.get_args());
    if let Some(cwd) = command.get_current_dir() {
        supervisor.current_dir(cwd);
    }
    for (key, value) in command.get_envs() {
        match value {
            Some(value) => {
                supervisor.env(key, value);
            }
            None => {
                supervisor.env_remove(key);
            }
        }
    }
    unsafe {
        supervisor.pre_exec(move || set_close_on_exec(reader.as_raw_fd(), false));
    }
    Ok((supervisor, Some(owner)))
}

/// Dispatch the Linux Git supervisor before initializing any desktop state.
pub fn run_git_supervisor() {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(OsStr::new(SUPERVISE)) {
        return;
    }
    let result = (|| {
        let fd = args
            .next()
            .and_then(|arg| arg.to_str()?.parse::<RawFd>().ok())
            .filter(|fd| *fd >= 3)
            .ok_or(io::ErrorKind::InvalidInput)?;
        let program = args.next().ok_or(io::ErrorKind::InvalidInput)?;
        set_close_on_exec(fd, true)?;
        // The caller passes the read end of an anonymous control pipe.
        let control = unsafe { File::from_raw_fd(fd) };
        if !control.metadata()?.file_type().is_fifo()
            || unsafe { libc::fcntl(fd, libc::F_GETFL) } & libc::O_ACCMODE != libc::O_RDONLY
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        supervise(&control, Command::new(program).args(args))
    })();
    let code = match result {
        Ok(Some(status)) => status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)),
        Ok(None) => 125,
        Err(error) => {
            eprintln!("Reponiq could not supervise Git: {error}");
            125
        }
    };
    std::process::exit(code);
}

fn set_close_on_exec(fd: RawFd, enabled: bool) -> io::Result<()> {
    let flag = if enabled { libc::FD_CLOEXEC } else { 0 };
    if unsafe { libc::fcntl(fd, libc::F_SETFD, flag) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn supervise(control: &File, command: &mut Command) -> io::Result<Option<ExitStatus>> {
    // A supervisor reaps orphaned Git descendants regardless of their process
    // group. The application holds the only writer of the control pipe.
    require_local_procfs()?;
    let children = File::open(CHILDREN)?;
    let unused: libc::c_ulong = 0;
    let enable: libc::c_ulong = 1;
    if unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, enable, unused, unused, unused) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, enable, unused, unused, unused) } != 0 {
        return Err(io::Error::last_os_error());
    }
    unsafe {
        libc::prctl(
            libc::PR_SET_NAME,
            c"reponiq-git".as_ptr(),
            unused,
            unused,
            unused,
        )
    };
    if owner_gone(control, 0)? {
        return Ok(None);
    }
    if !crate::workspace::is_probe(command) {
        use_system_git_libraries(command)?;
    }
    let mut child = command.spawn()?;
    let result = (|| loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if owner_gone(control, 20)? {
            return Ok(None);
        }
    })();
    reap_git_tree(children)?;
    result
}

fn use_system_git_libraries(command: &mut Command) -> io::Result<()> {
    let (Some(_), Some(bundle), Some(paths)) = (
        std::env::var_os("APPIMAGE"),
        std::env::var_os("APPDIR"),
        std::env::var_os("LD_LIBRARY_PATH"),
    ) else {
        return Ok(());
    };
    let bundle = Path::new(&bundle);
    if !bundle.is_absolute() {
        return Ok(());
    }
    // The supervisor needs AppImage libraries during exec. System Git and its
    // helpers need the distribution's matching libraries.
    match host_library_paths(bundle, &paths)? {
        Some(paths) => {
            command.env("LD_LIBRARY_PATH", paths);
        }
        None => {
            command.env_remove("LD_LIBRARY_PATH");
        }
    }
    Ok(())
}

fn host_library_paths(bundle: &Path, paths: &OsStr) -> io::Result<Option<OsString>> {
    let host: Vec<_> = paths
        .as_bytes()
        .split(|byte| *byte == b':' || *byte == b';')
        .map(|part| Path::new(OsStr::from_bytes(part)))
        .filter(|part| part.is_absolute() && !part.starts_with(bundle))
        .collect();
    if host.is_empty() {
        return Ok(None);
    }
    std::env::join_paths(host)
        .map(Some)
        .map_err(io::Error::other)
}

fn require_local_procfs() -> io::Result<()> {
    let status = std::fs::read_to_string("/proc/thread-self/status")?;
    let mut ids = status
        .lines()
        .find_map(|line| line.strip_prefix("NSpid:"))
        .unwrap_or_default()
        .split_ascii_whitespace();
    let tid = unsafe { libc::gettid() };
    if ids.next().and_then(|id| id.parse().ok()) != Some(tid) || ids.next().is_some() {
        return Err(io::Error::other(
            "Linux procfs must match the Git supervisor's PID namespace",
        ));
    }
    Ok(())
}

fn owner_gone(control: &File, timeout: i32) -> io::Result<bool> {
    let mut pipe = libc::pollfd {
        fd: control.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let result = unsafe { libc::poll(&mut pipe, 1, timeout) };
    if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
        return Err(io::Error::last_os_error());
    }
    Ok(result > 0)
}

fn reap_git_tree(mut children_file: File) -> io::Result<()> {
    const CHILD_LIST_LIMIT: usize = 64 * 1024;
    loop {
        let result = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
        if result < 0 {
            let error = io::Error::last_os_error();
            match error.raw_os_error() {
                Some(libc::ECHILD) => return Ok(()),
                Some(libc::EINTR) => continue,
                _ => return Err(error),
            }
        }
        // Only signal unreaped direct children owned by this supervisor. The
        // kernel reassigns their children here as they exit. A concurrent exit
        // can hide entries from procfs, so only waitpid proves cleanup is done.
        let mut children = String::new();
        children_file.seek(SeekFrom::Start(0))?;
        (&mut children_file)
            .take(CHILD_LIST_LIMIT as u64)
            .read_to_string(&mut children)?;
        if children.len() == CHILD_LIST_LIMIT {
            // Skip a partial PID at the end of the bounded snapshot.
            children.truncate(children.rfind(' ').unwrap_or(0));
        }
        for child in children.split_ascii_whitespace() {
            if let Ok(pid) = child.parse::<libc::pid_t>() {
                if pid > 1 {
                    unsafe { libc::kill(pid, libc::SIGKILL) };
                }
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_git_keeps_host_library_paths_and_drops_packaged_ones() {
        let bundle = Path::new("/tmp/mounted AppImage");
        let mixed = OsStr::new("/tmp/mounted AppImage/usr/lib;:/usr/local/lib:relative:/usr/lib");
        assert_eq!(
            host_library_paths(bundle, mixed).unwrap().as_deref(),
            Some(OsStr::new("/usr/local/lib:/usr/lib"))
        );
        assert_eq!(
            host_library_paths(bundle, OsStr::new("/tmp/mounted AppImage/usr/lib:")).unwrap(),
            None
        );
    }
}
