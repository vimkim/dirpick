#[test]
fn complete_workflows_resize_and_restore_the_real_terminal() {
    let status = std::process::Command::new("python3")
        .arg("-B")
        .arg("tests/pty_workflow.py")
        .arg(env!("CARGO_BIN_EXE_dirpick"))
        .status()
        .expect("Python 3 is required for the release PTY journeys");
    assert!(status.success());
}
