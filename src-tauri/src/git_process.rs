use std::io::{self, PipeReader, Read};
use std::process::{Command, ExitStatus};
use std::time::{Duration, Instant};

const OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
const DIAGNOSTIC_LIMIT: usize = 64 * 1024;

pub(super) struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub(super) enum Error {
    Start(io::Error),
    Io(io::Error),
    Timeout,
    OutputLimit,
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

struct Stream {
    reader: PipeReader,
    bytes: Vec<u8>,
    count: usize,
    retain: usize,
}

impl Stream {
    fn new(reader: PipeReader, retain: usize) -> io::Result<Self> {
        platform::prepare_pipe(&reader)?;
        Ok(Self {
            reader,
            bytes: Vec::new(),
            count: 0,
            retain,
        })
    }

    fn read_ready(&mut self) -> Result<bool, Error> {
        let mut buffer = [0; 8192];
        let max = buffer.len().min(OUTPUT_LIMIT - self.count + 1);
        let count = platform::read_ready(&mut self.reader, &mut buffer[..max])?;
        self.count += count;
        if self.count > OUTPUT_LIMIT {
            return Err(Error::OutputLimit);
        }
        let keep = count.min(self.retain - self.bytes.len());
        self.bytes.extend_from_slice(&buffer[..keep]);
        Ok(count > 0)
    }
}

pub(super) fn capture(mut command: Command, timeout: Duration) -> Result<Output, Error> {
    let (stdout, stdout_write) = platform::pipe()?;
    let (stderr, stderr_write) = platform::pipe()?;
    let mut stdout = Stream::new(stdout, OUTPUT_LIMIT)?;
    let mut stderr = Stream::new(stderr, DIAGNOSTIC_LIMIT)?;
    command.stdout(stdout_write).stderr(stderr_write);
    let mut process = platform::Process::spawn(command).map_err(Error::Start)?;
    let start = Instant::now();

    let status = loop {
        if process.exited()? {
            // Stop remaining helpers before draining. A helper retaining a pipe
            // must not prevent an otherwise completed Git command from returning.
            break process.finish()?;
        }
        if start.elapsed() >= timeout {
            return Err(Error::Timeout);
        }
        if !(stdout.read_ready()? | stderr.read_ready()?) {
            platform::wait_for_output(&stdout.reader, &stderr.reader)?;
        }
    };
    while stdout.read_ready()? | stderr.read_ready()? {}
    Ok(Output {
        status,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
    })
}

#[cfg(unix)]
mod platform {
    use super::*;
    pub(super) use std::io::pipe;
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::Child;

    pub(super) struct Process(Option<Child>);

    impl Process {
        pub(super) fn spawn(mut command: Command) -> io::Result<Self> {
            Ok(Self(Some(command.process_group(0).spawn()?)))
        }

        pub(super) fn exited(&mut self) -> io::Result<bool> {
            // Keep the leader unreaped until cleanup, so its process group ID
            // cannot be reused between checking its status and signalling it.
            let child = self.0.as_ref().expect("running child");
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.id(),
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result == -1 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    return Ok(false);
                }
                return Err(error);
            }
            Ok(unsafe { info.si_pid() } != 0)
        }

        pub(super) fn finish(&mut self) -> io::Result<ExitStatus> {
            let child = self.0.as_mut().expect("running child");
            if unsafe { libc::killpg(child.id() as libc::pid_t, libc::SIGKILL) } == -1 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
            }
            let status = child.wait()?;
            self.0 = None;
            Ok(status)
        }
    }

    impl Drop for Process {
        fn drop(&mut self) {
            if self.0.is_some() {
                let _ = self.finish();
            }
        }
    }

    pub(super) fn prepare_pipe(reader: &PipeReader) -> io::Result<()> {
        let fd = reader.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn read_ready(reader: &mut PipeReader, buffer: &mut [u8]) -> io::Result<usize> {
        match reader.read(buffer) {
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(0)
            }
            result => result,
        }
    }

    pub(super) fn wait_for_output(stdout: &PipeReader, stderr: &PipeReader) -> io::Result<()> {
        let mut pipes = [stdout, stderr].map(|reader| libc::pollfd {
            fd: reader.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });
        if unsafe { libc::poll(pipes.as_mut_ptr(), pipes.len() as libc::nfds_t, 5) } == -1 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        // A pipe whose writer is closed stays ready even when empty.
        if pipes.iter().any(|pipe| pipe.revents & libc::POLLHUP != 0) {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
}

#[cfg(windows)]
#[path = "git_process/windows.rs"]
mod platform;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write, path::Path};

    fn fixture(mode: &str, directory: &Path) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "git_process::tests::fixture_child",
                "--nocapture",
            ])
            .env("REPONIQ_TEST_PROCESS_MODE", mode)
            .env("REPONIQ_TEST_PROCESS_DIR", directory);
        command
    }

    #[test]
    #[ignore = "runs in a supervised subprocess"]
    fn fixture_child() {
        let Ok(mode) = std::env::var("REPONIQ_TEST_PROCESS_MODE") else {
            return;
        };
        let directory = std::env::var_os("REPONIQ_TEST_PROCESS_DIR").unwrap();
        let directory = Path::new(&directory);
        match mode.as_str() {
            "stdout" | "stderr" => {
                let mut stream: Box<dyn Write> = if mode == "stdout" {
                    Box::new(io::stdout().lock())
                } else {
                    Box::new(io::stderr().lock())
                };
                for _ in 0..=OUTPUT_LIMIT / 8192 {
                    stream.write_all(&[b'a'; 8192]).unwrap();
                }
            }
            "helper" => {
                fs::write(directory.join("ready"), b"ready").unwrap();
                std::thread::sleep(Duration::from_secs(4));
                fs::write(directory.join("finished"), b"finished").unwrap();
            }
            "exit" | "timeout" => {
                // The production supervisor owns this intentionally long-running helper.
                let _helper = fixture("helper", directory).spawn().unwrap();
                let start = Instant::now();
                while !directory.join("ready").exists() {
                    assert!(start.elapsed() < Duration::from_secs(2));
                    std::thread::sleep(Duration::from_millis(5));
                }
                if mode == "timeout" {
                    std::thread::sleep(Duration::from_secs(6));
                }
            }
            _ => panic!("unexpected fixture mode"),
        }
    }

    #[test]
    fn capture_limits_each_stream() {
        let directory = tempfile::tempdir().unwrap();
        for mode in ["stdout", "stderr"] {
            let result = capture(fixture(mode, directory.path()), Duration::from_secs(10));
            assert!(
                matches!(result, Err(Error::OutputLimit)),
                "unexpected outcome for {mode}"
            );
        }
    }

    #[test]
    fn helpers_stop_on_normal_completion_and_timeout() {
        let completed = tempfile::tempdir().unwrap();
        let timed_out = tempfile::tempdir().unwrap();
        assert!(
            capture(fixture("exit", completed.path()), Duration::from_secs(6))
                .unwrap()
                .status
                .success()
        );
        assert!(matches!(
            capture(fixture("timeout", timed_out.path()), Duration::from_secs(3)),
            Err(Error::Timeout)
        ));
        for directory in [&completed, &timed_out] {
            assert!(directory.path().join("ready").exists());
        }
        std::thread::sleep(Duration::from_millis(4200));
        for directory in [&completed, &timed_out] {
            assert!(!directory.path().join("finished").exists());
        }
    }
}
