use base64::{Engine, engine::general_purpose::STANDARD};
use dirpick::session::{Command, SearchSession};
use std::{
    fs,
    time::{Duration, Instant},
};
use tempfile::tempdir;

fn settled(session: &mut SearchSession) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while session.state().scanning
        || session.state().preview.loading
        || session.state().scope_loading
    {
        assert!(
            Instant::now() < deadline,
            "session did not settle: {:?}",
            session.state()
        );
        session.wait(Duration::from_millis(20));
    }
}

#[test]
fn scrolling_source_keeps_multiline_syntax_context_and_selection() {
    let root = tempdir().unwrap();
    let source = format!(
        "/* open comment\n{}still in comment\n*/\nfn main() {{}}\n",
        "comment body\n".repeat(220)
    );
    fs::write(root.path().join("source.rs"), source).unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    let path = session.state().selected().unwrap().path.clone();
    session.command(Command::ScrollPreview(221));
    settled(&mut session);
    let preview = &session.state().preview;
    assert_eq!(preview.lines[0].number, 222);
    assert_eq!(preview.lines[0].text, "still in comment");
    assert!(!preview.lines[0].spans.is_empty());
    assert!(
        preview.lines[0]
            .spans
            .iter()
            .all(|span| span.foreground == [108, 112, 134]),
        "multiline comments use Mocha overlay0: {:?}",
        preview.lines[0]
    );
    assert!(!preview.lines[2].spans.is_empty());
    assert!(
        preview.lines[2]
            .spans
            .iter()
            .any(|span| span.foreground != [108, 112, 134])
    );
    assert_eq!(session.state().selected().unwrap().path, path);
}

#[test]
fn long_lines_remain_readable_and_scroll_past_the_bounded_window() {
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("long.rs"),
        format!("{}\nnext useful line\n", "한".repeat(30_000)),
    )
    .unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    let preview = &session.state().preview;
    assert!(!preview.lines.is_empty());
    assert!(preview.lines[0].text.len() <= 64 * 1024);
    assert!(preview.status.contains("clipped"));
    session.command(Command::ScrollPreview(1));
    settled(&mut session);
    assert_eq!(session.state().preview.lines[0].text, "next useful line");
    assert!(session.state().preview.lines[0].spans.is_empty());
    assert!(session.state().preview.status.contains("plain text"));
}

#[test]
fn copy_emits_the_exact_absolute_path_and_preserves_the_search() {
    let root = tempdir().unwrap();
    let filename = "-space 'quote' \"한글\"\t \n";
    fs::write(root.path().join(filename), "preview\n").unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    session.command(Command::SetQuery("space".into()));
    settled(&mut session);
    let before = session.state().clone();
    session.command(Command::CopyPath);
    let action = session
        .take_action()
        .expect("selected path requests an action");
    let mut terminal = Vec::new();
    let result = action.write_to(&mut terminal);
    session.complete_action(action, result);
    assert!(terminal.starts_with(b"\x1b]52;c;"));
    assert_eq!(terminal.last(), Some(&7));
    let decoded = STANDARD.decode(&terminal[7..terminal.len() - 1]).unwrap();
    assert_eq!(
        decoded,
        root.path().join(filename).to_str().unwrap().as_bytes()
    );
    assert_eq!(session.state().generation, before.generation);
    assert_eq!(session.state().settings.query, before.settings.query);
    assert_eq!(session.state().settings.scope, before.settings.scope);
    assert_eq!(session.state().selected(), before.selected());
    assert_eq!(session.state().preview, before.preview);
    assert!(session.state().message.contains("payload sent"));
    assert!(session.state().message.contains("acceptance unknown"));
}

#[test]
fn failed_clipboard_flush_keeps_selection_and_allows_another_action() {
    struct FailedTerminal {
        fail_write: bool,
    }
    impl std::io::Write for FailedTerminal {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.fail_write {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "terminal write failed",
                ));
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "terminal disconnected",
            ))
        }
    }
    let root = tempdir().unwrap();
    fs::write(root.path().join("kept.txt"), "kept\n").unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    session.command(Command::CopyPath);
    let action = session.take_action().unwrap();
    let result = action.write_to(&mut FailedTerminal { fail_write: false });
    session.complete_action(action, result);
    assert!(session.state().message.contains("Copy failed"));
    assert!(!session.state().message.contains("payload sent"));
    assert_eq!(session.state().selected().unwrap().relative, "kept.txt");
    session.command(Command::CopyPath);
    let action = session.take_action().unwrap();
    let result = action.write_to(&mut FailedTerminal { fail_write: true });
    session.complete_action(action, result);
    assert!(session.state().message.contains("Copy failed"));
    session.command(Command::CopyPath);
    assert!(session.take_action().is_some());
}

#[test]
fn unsupported_paths_are_reported_without_lossy_clipboard_substitution() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempdir().unwrap();
    let name = std::ffi::OsString::from_vec(b"invalid-\xff.txt".to_vec());
    fs::write(root.path().join(&name), "source\n").unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    session.command(Command::CopyPath);
    assert!(session.take_action().is_none());
    assert!(session.state().message.contains("UTF-8"));
    assert_eq!(
        session.state().selected().unwrap().path,
        root.path().join(name)
    );
    session.command(Command::SetQuery("missing".into()));
    settled(&mut session);
    session.command(Command::CopyPath);
    assert!(session.take_action().is_none());
    assert!(session.state().message.contains("Nothing selected"));
}

#[test]
fn a_selected_source_location_opens_with_nearby_context() {
    use dirpick::session::{SearchEvent, SearchResult};
    let root = tempdir().unwrap();
    let path = root.path().join("source.rs");
    fs::write(root.path().join("a.txt"), "first selection\n").unwrap();
    fs::write(&path, "fn source() {}\n".repeat(300)).unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    session.apply_event(SearchEvent::Results {
        generation: session.state().generation,
        rows: vec![SearchResult {
            path,
            relative: "source.rs".into(),
            line: 250,
            text: String::new(),
            matches: Vec::new(),
        }],
        visited: 1,
        matched: 1,
        errors: 0,
        entry_limit: false,
        done: true,
    });
    settled(&mut session);
    assert_eq!(session.state().preview.lines[0].number, 240);
    assert_eq!(session.state().selected().unwrap().line, 250);
}

#[test]
fn unsupported_source_has_clear_status_and_unknown_syntax_stays_readable() {
    let root = tempdir().unwrap();
    for (name, bytes) in [
        ("binary.dat", vec![0, 1]),
        ("invalid.txt", vec![255]),
        ("oversized.txt", vec![b'x'; 2 * 1024 * 1024 + 1]),
        ("empty.txt", Vec::new()),
        (
            "plain.unrecognized-dirpick",
            b"unfamiliar source \x1b[31m\n".to_vec(),
        ),
        ("gone.txt", b"temporary\n".to_vec()),
        ("unreadable.txt", b"private\n".to_vec()),
    ] {
        fs::write(root.path().join(name), bytes).unwrap();
    }
    let mut session = SearchSession::open(root.path()).unwrap();
    for (query, status) in [
        ("binary.dat", "Binary"),
        ("invalid.txt", "Invalid UTF-8"),
        ("oversized.txt", "2 MiB"),
        ("empty.txt", "Empty file"),
    ] {
        session.command(Command::SetQuery(query.into()));
        settled(&mut session);
        assert!(
            session.state().preview.status.contains(status),
            "{:?}",
            session.state().preview
        );
        assert!(session.state().preview.lines.is_empty());
    }
    session.command(Command::SetQuery("plain.unrecognized-dirpick".into()));
    settled(&mut session);
    assert_eq!(
        session.state().preview.lines[0].text,
        "unfamiliar source \x1b[31m"
    );
    assert!(session.state().preview.lines[0].spans.is_empty());
    session.command(Command::SetQuery("gone.txt".into()));
    settled(&mut session);
    fs::remove_file(root.path().join("gone.txt")).unwrap();
    session.command(Command::ScrollPreview(0));
    settled(&mut session);
    assert!(session.state().preview.status.contains("vanished"));
    // Real mode-bit denial on the supported non-root Linux account.
    use std::os::unix::fs::PermissionsExt;
    session.command(Command::SetQuery("unreadable.txt".into()));
    settled(&mut session);
    let private = root.path().join("unreadable.txt");
    fs::set_permissions(&private, fs::Permissions::from_mode(0o0)).unwrap();
    let denied = fs::File::open(&private).is_err();
    session.command(Command::ScrollPreview(0));
    settled(&mut session);
    fs::set_permissions(&private, fs::Permissions::from_mode(0o600)).unwrap();
    if denied {
        assert!(session.state().preview.status.contains("Cannot read file"));
    }
}
