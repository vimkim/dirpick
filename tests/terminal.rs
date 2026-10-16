#[test]
fn real_terminal_starts_accepts_input_and_restores_flags() {
    let status = std::process::Command::new("python3")
        .arg("tests/pty_smoke.py")
        .arg(env!("CARGO_BIN_EXE_dirpick"))
        .status()
        .expect("python3 is required for the Linux PTY acceptance check");
    assert!(status.success());
}

#[test]
fn space_shortcuts_preserve_editing_and_remain_discoverable_across_sizes() {
    let status = std::process::Command::new("python3")
        .arg("-B")
        .arg("tests/pty_shortcuts.py")
        .arg(env!("CARGO_BIN_EXE_dirpick"))
        .status()
        .expect("python3 is required for the Linux PTY shortcut check");
    assert!(status.success());
}
