#[test]
fn real_terminal_edits_path_filters_and_toggles_traversal_independently() {
    let status = std::process::Command::new("python3")
        .arg("-B")
        .arg("tests/pty_filters.py")
        .arg(env!("CARGO_BIN_EXE_dirpick"))
        .status()
        .expect("python3 is required for the Linux PTY acceptance check");
    assert!(status.success());
}
