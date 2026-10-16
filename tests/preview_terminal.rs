#[test]
fn every_preview_row_is_reachable_at_minimum_terminal_size() {
    let status = std::process::Command::new("python3")
        .arg("-B")
        .arg("tests/pty_preview.py")
        .arg(env!("CARGO_BIN_EXE_dirpick"))
        .status()
        .expect("python3 is required for the Linux PTY preview check");
    assert!(status.success());
}

#[test]
fn source_clipping_keeps_whole_graphemes_and_the_text_that_fits_after_them() {
    let status = std::process::Command::new("python3")
        .arg("-B")
        .arg("tests/pty_preview_unicode.py")
        .arg(env!("CARGO_BIN_EXE_dirpick"))
        .status()
        .expect("python3 is required for the Linux PTY preview check");
    assert!(status.success());
}
