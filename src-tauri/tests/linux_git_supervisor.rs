#![cfg(target_os = "linux")]

use std::fs;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn force_quit_releases_the_supervisor_and_its_helpers() {
    let directory = tempfile::tempdir().unwrap();
    let mut application = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "application_fixture"])
        .env("REPONIQ_SUPERVISOR_TEST_DIR", directory.path())
        .spawn()
        .unwrap();
    let result = wait_for_pid(&directory.path().join("helper"));
    application.kill().unwrap();
    application.wait().unwrap();
    let helper = result.expect("the Git fixture should start its helper");
    let worker = wait_for_pid(&directory.path().join("supervisor")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while [worker, helper].into_iter().any(is_running) {
        assert!(
            Instant::now() < deadline,
            "a Git fixture process survived forced quit"
        );
        std::thread::sleep(Duration::from_millis(15));
    }
}

#[test]
#[ignore = "holds a supervisor control pipe in a disposable child"]
fn application_fixture() {
    let Some(dir) = std::env::var_os("REPONIQ_SUPERVISOR_TEST_DIR") else {
        return;
    };
    let path = Path::new(&dir);
    let (reader, writer) = io::pipe().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_reponiq"));
    command
        .args(["--internal-git-supervisor", &reader.as_raw_fd().to_string()])
        .args([
            "/bin/sh",
            "-c",
            r#"/usr/bin/setsid /bin/sh -c 'printf "%s\n" "$$" > "$1"; exec /bin/sleep 30' helper "$1" & wait"#,
            "reponiq-test",
        ])
        .arg(path.join("helper"))
        .stdin(Stdio::null());
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(reader.as_raw_fd(), libc::F_SETFD, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut worker = command.spawn().unwrap();
    fs::write(path.join("supervisor"), worker.id().to_string()).unwrap();
    let _ = worker.wait();
    drop(writer);
}

fn wait_for_pid(path: &Path) -> Option<u32> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(value) = fs::read_to_string(path) {
            if let Ok(pid) = value.trim().parse() {
                return Some(pid);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

fn is_running(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|entry| {
            entry
                .rsplit_once(") ")
                .and_then(|(_, tail)| tail.chars().next())
        })
        .is_some_and(|state| state != 'Z')
}
