use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn workspace_probe_returns_bounded_discovery_without_desktop_state() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("project/.git")).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_reponiq"))
        .arg("--internal-workspace-probe")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    serde_json::to_writer(
        child.stdin.take().unwrap(),
        &serde_json::json!({"Discover":{"source":"folder","folder":root.path(),"default":null}}),
    )
    .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let response: Result<serde_json::Value, String> =
        serde_json::from_slice(&output.stdout).unwrap();
    let data = response.unwrap();
    assert_eq!(data["items"][0]["name"], "project");
    assert_eq!(data["limited"], false);
}

#[test]
fn workspace_probe_rejects_an_oversized_request() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_reponiq"))
        .arg("--internal-workspace-probe")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&vec![b' '; 512 * 1024 + 1])
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let response: Result<serde_json::Value, String> =
        serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response, Err("Workspace request is too large.".into()));
}

#[test]
fn private_clone_worker_preserves_a_nonempty_destination() {
    let root = tempfile::tempdir().unwrap();
    let existing = root.path().join("existing.txt");
    std::fs::write(&existing, "keep this").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_reponiq"))
        .arg("--internal-private-clone")
        .current_dir(root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    serde_json::to_writer(
        child.stdin.take().unwrap(),
        &serde_json::json!({"full_name":"fixture/project","token":"dummy-test-token"}),
    )
    .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let response: Result<(), String> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        response,
        Err("The clone folder is no longer empty. Existing files were preserved.".into())
    );
    assert_eq!(std::fs::read_to_string(existing).unwrap(), "keep this");
    assert!(!root.path().join(".git").exists());
}

#[test]
fn private_clone_worker_rejects_missing_managed_scratch_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_reponiq"))
        .arg("--internal-private-clone")
        .current_dir(root.path())
        .env(
            "REPONIQ_GIT_SCRATCH_DIR",
            root.path().join("missing-volume"),
        )
        .env("GIT_ALLOW_PROTOCOLS", "file")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    serde_json::to_writer(
        child.stdin.take().unwrap(),
        &serde_json::json!({"full_name":"fixture/project","token":"dummy-test-token"}),
    )
    .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let response: Result<(), String> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(response
        .unwrap_err()
        .starts_with("Managed Git scratch is unavailable."));
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
