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

#[test]
fn query_redraw_keeps_intermediate_cursor_movements_hidden() {
    let status = std::process::Command::new("python3")
        .arg("-B")
        .arg("tests/pty_cursor.py")
        .arg(env!("CARGO_BIN_EXE_dirpick"))
        .status()
        .expect("python3 is required for the Linux PTY cursor check");
    assert!(status.success());
}

#[test]
fn shell_editing_shortcuts_update_fields_and_search_results() {
    let status = std::process::Command::new("python3")
        .arg("-B")
        .arg("tests/pty_editing.py")
        .arg(env!("CARGO_BIN_EXE_dirpick"))
        .status()
        .expect("python3 is required for the Linux PTY field editing check");
    assert!(status.success());
}

#[test]
fn navigation_shortcuts_select_browse_and_restore_without_alt() {
    let status = std::process::Command::new("python3")
        .arg("-B")
        .arg("tests/pty_navigation.py")
        .arg(env!("CARGO_BIN_EXE_dirpick"))
        .status()
        .expect("python3 is required for the Linux PTY navigation check");
    assert!(status.success());
}
