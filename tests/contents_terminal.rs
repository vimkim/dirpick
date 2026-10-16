#[test]
fn contents_and_editor_handoff_work_in_an_isolated_controlling_terminal() {
    let status = std::process::Command::new("python3")
        .arg("tests/pty_editor.py")
        .arg(env!("CARGO_BIN_EXE_dirpick"))
        .status()
        .expect("python3 is required for the Linux PTY editor check");
    assert!(status.success());
}
