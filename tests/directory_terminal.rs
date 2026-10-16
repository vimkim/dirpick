#[test]
fn real_terminal_browses_restores_and_emits_only_explicit_directory_json() {
    let status = std::process::Command::new("python3")
        .arg("tests/pty_directories.py")
        .arg(env!("CARGO_BIN_EXE_dirpick"))
        .status()
        .expect("python3 is required for the Linux PTY acceptance check");
    assert!(status.success());
}
