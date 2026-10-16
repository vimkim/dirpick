use dirpick::session::{Command, SearchMode, SearchSession};
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
fn contents_searches_literal_lines_and_previews_every_raw_utf8_match() {
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("source.rs"),
        "header\n한 a.b A.B a.b\nno regex: axb\n",
    )
    .unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetMode(SearchMode::Contents));
    session.command(Command::SetQuery("a.b".into()));
    settled(&mut session);
    assert_eq!(session.state().results.len(), 1);
    let row = session.state().selected().unwrap();
    assert_eq!(row.line, 2);
    assert_eq!(row.text, "한 a.b A.B a.b");
    assert_eq!(row.matches, vec![4..7, 8..11, 12..15]);
    assert_eq!(
        session.state().preview.lines[1].matches,
        vec![4..7, 8..11, 12..15]
    );
    session.command(Command::SetQuery("A.B".into()));
    settled(&mut session);
    assert_eq!(session.state().selected().unwrap().matches, vec![8..11]);
    session.command(Command::SetQuery("a.b".into()));
    session.command(Command::ToggleRegex);
    settled(&mut session);
    assert_eq!(
        session
            .state()
            .results
            .iter()
            .map(|row| row.line)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
}

#[test]
fn contents_validates_empty_fields_and_retains_shared_settings_across_modes() {
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    for (name, text) in [
        ("src/kept.rs", "needle\n"),
        ("src/no.txt", "needle\n"),
        ("src/.hidden.rs", "needle\n"),
        ("ignored.rs", "needle\n"),
    ] {
        fs::write(root.path().join(name), text).unwrap();
    }
    fs::write(root.path().join(".ignore"), "ignored.rs\n").unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetMode(SearchMode::Contents));
    settled(&mut session);
    assert_eq!(session.state().visited, 0);
    assert!(session.state().results.is_empty());
    session.command(Command::SetInclude("[".into()));
    settled(&mut session);
    assert!(session.state().include_error.is_some());
    session.command(Command::SetInclude("\\.rs$".into()));
    session.command(Command::SetQuery("[".into()));
    session.command(Command::ToggleRegex);
    settled(&mut session);
    assert!(session.state().query_error.is_some());
    assert!(session.state().results.is_empty());
    session.command(Command::SetQuery("needle".into()));
    settled(&mut session);
    assert_eq!(
        session
            .state()
            .results
            .iter()
            .map(|row| row.relative.as_str())
            .collect::<Vec<_>>(),
        vec!["src/kept.rs"]
    );
    session.command(Command::ToggleHidden);
    session.command(Command::ToggleIgnored);
    session.command(Command::SetExclude("kept".into()));
    settled(&mut session);
    assert_eq!(
        session
            .state()
            .results
            .iter()
            .map(|row| row.relative.as_str())
            .collect::<Vec<_>>(),
        vec!["ignored.rs", "src/.hidden.rs"]
    );
    session.command(Command::SetMode(SearchMode::Files));
    settled(&mut session);
    session.command(Command::SetMode(SearchMode::Contents));
    settled(&mut session);
    assert_eq!(session.state().query, "needle");
    assert_eq!(session.state().include, "\\.rs$");
    assert_eq!(session.state().exclude, "kept");
    assert!(session.state().regex && session.state().hidden && session.state().ignored);
    assert_eq!(session.state().results.len(), 2);
}

#[test]
fn content_limits_skip_unsupported_files_and_keep_path_discovery_available() {
    use dirpick::session::{Limits, Options};
    let root = tempdir().unwrap();
    for (name, bytes) in [
        ("binary", vec![0, b'n']),
        ("invalid", vec![255]),
        ("oversized", vec![b'n'; 2 * 1024 * 1024 + 1]),
        ("valid", b"needle\n".to_vec()),
    ] {
        fs::write(root.path().join(name), bytes).unwrap();
    }
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    assert_eq!(session.state().results.len(), 4);
    session.command(Command::SetMode(SearchMode::Contents));
    session.command(Command::SetQuery("needle".into()));
    settled(&mut session);
    assert_eq!(session.state().results.len(), 1);
    assert_eq!(session.state().skipped, 3);
    fs::write(root.path().join("a-many"), "needle\n".repeat(5_001)).unwrap();
    session.command(Command::SetQuery("need".into()));
    settled(&mut session);
    assert_eq!(session.state().results.len(), 5_000);
    assert!(session.state().results.iter().all(|row| row.line <= 5_000));
    assert!(session.state().result_limit);
    let mut bounded = SearchSession::with_options(
        root.path(),
        Options {
            limits: Limits {
                entries: 1,
                results: 2,
            },
            ..Options::default()
        },
    )
    .unwrap();
    bounded.command(Command::SetMode(SearchMode::Contents));
    bounded.command(Command::SetQuery("unmatched".into()));
    settled(&mut bounded);
    assert_eq!(bounded.state().visited, 1);
    assert!(bounded.state().entry_limit);
}

#[test]
fn content_streaming_tracks_path_and_line_and_rejects_old_events_and_actions() {
    use dirpick::{
        action::Action,
        session::{SearchEvent, SearchResult},
    };
    let root = tempdir().unwrap();
    let path = root.path().join("source.rs");
    fs::write(
        &path,
        format!(
            "{}needle first\n{}needle second\n",
            "context\n".repeat(200),
            "context\n".repeat(59)
        ),
    )
    .unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetMode(SearchMode::Contents));
    session.command(Command::SetQuery("needle".into()));
    settled(&mut session);
    session.command(Command::Select(1));
    settled(&mut session);
    assert_eq!(session.state().selected().unwrap().line, 261);
    session.command(Command::ScrollPreview(5));
    settled(&mut session);
    let rows = session.state().results.clone();
    let top = session.state().preview.top;
    let obsolete = session.state().generation;
    let update = |generation, rows| SearchEvent::Results {
        generation,
        rows,
        visited: 1,
        matched: 2,
        errors: 0,
        entry_limit: false,
        done: true,
    };
    let mut inserted = rows.clone();
    inserted.insert(
        0,
        SearchResult {
            path: path.clone(),
            relative: "source.rs".into(),
            line: 10,
            text: "needle inserted".into(),
            matches: std::iter::once(0..6).collect(),
        },
    );
    session.apply_event(update(obsolete, inserted));
    assert_eq!(session.state().selected().unwrap().line, 261);
    assert_eq!(session.state().selected_index, 2);
    assert_eq!(session.state().preview.top, top);
    session.command(Command::OpenSelected);
    assert_eq!(
        session.take_action(),
        Some(Action::OpenEditor {
            path: path.clone(),
            line: 261
        })
    );
    session.command(Command::CopyPath);
    session.command(Command::SetQuery("absent".into()));
    settled(&mut session);
    assert!(
        session.take_action().is_none(),
        "Changed search must not retain an obsolete requested action"
    );
    session.apply_event(update(obsolete, rows));
    session.apply_event(SearchEvent::InvalidQuery {
        generation: obsolete,
        message: "obsolete".into(),
    });
    session.apply_event(SearchEvent::ContentStats {
        generation: obsolete,
        skipped: 99,
        result_limit: true,
    });
    assert!(session.state().results.is_empty());
    assert!(session.state().query_error.is_none());
    assert!(!session.state().result_limit);
    assert_eq!(session.state().skipped, 0);
}

#[test]
fn clipped_preview_keeps_original_line_regex_positions_and_handles_missing_sources() {
    let root = tempdir().unwrap();
    let path = root.path().join("long.txt");
    fs::write(&path, format!("{}x\n", "x".repeat(70_000))).unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetMode(SearchMode::Contents));
    session.command(Command::ToggleRegex);
    session.command(Command::SetQuery("x$".into()));
    settled(&mut session);
    assert_eq!(
        session.state().selected().unwrap().matches,
        vec![70_000..70_001]
    );
    assert!(session.state().preview.status.contains("clipped"));
    assert!(
        session.state().preview.lines[0].matches.is_empty(),
        "An anchor must match the original line, not the clipped prefix"
    );
    fs::remove_file(path).unwrap();
    session.command(Command::ScrollPreview(0));
    settled(&mut session);
    assert!(session.state().preview.status.contains("vanished"));
    assert_eq!(session.state().selected().unwrap().line, 1);
}

#[test]
fn frequent_matches_in_long_lines_have_an_honest_aggregate_memory_bound() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("minified.txt"), "x".repeat(1024 * 1024)).unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetMode(SearchMode::Contents));
    session.command(Command::SetQuery("x".into()));
    settled(&mut session);
    assert!(
        session.state().result_limit,
        "Match ranges as well as source bytes must fit the retained result budget"
    );
    assert!(
        session
            .state()
            .scan_error
            .as_ref()
            .is_some_and(|message| message.contains("16 MiB"))
    );
    assert!(
        session.state().results.is_empty(),
        "Do not present a partially retained matching line as complete"
    );
    session.command(Command::SetQuery("xxx$".into()));
    session.command(Command::ToggleRegex);
    settled(&mut session);
    assert_eq!(session.state().results.len(), 1);
    assert_eq!(session.state().selected().unwrap().line, 1);
    assert!(!session.state().result_limit);
}

#[test]
fn back_restores_content_mode_regex_exact_line_and_preview_position() {
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("source.rs"),
        format!(
            "{}needle one\n{}needle two\n",
            "context\n".repeat(200),
            "context\n".repeat(59)
        ),
    )
    .unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetMode(SearchMode::Contents));
    session.command(Command::ToggleRegex);
    session.command(Command::SetQuery("needle.*".into()));
    session.command(Command::SetInclude("\\.rs$".into()));
    session.command(Command::SetExclude("excluded".into()));
    session.command(Command::ToggleHidden);
    session.command(Command::ToggleIgnored);
    settled(&mut session);
    session.command(Command::Select(1));
    settled(&mut session);
    session.command(Command::ScrollPreview(5));
    settled(&mut session);
    let before = session.state().clone();
    session.command(Command::ToggleScope);
    settled(&mut session);
    session.command(Command::SetMode(SearchMode::Directories));
    session.command(Command::ToggleRegex);
    settled(&mut session);
    session.command(Command::Back);
    settled(&mut session);
    assert_eq!(session.state().mode, SearchMode::Contents);
    assert!(session.state().regex && session.state().hidden && session.state().ignored);
    assert_eq!(session.state().selected(), before.selected());
    assert_eq!(session.state().query, before.query);
    assert_eq!(session.state().include, before.include);
    assert_eq!(session.state().exclude, before.exclude);
    assert_eq!(session.state().preview, before.preview);
}

#[test]
fn preview_centers_a_selected_match_for_its_viewport_and_preserves_manual_scroll_history() {
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join("other")).unwrap();
    let source = (1..=300)
        .map(|number| {
            if matches!(number, 260 | 280) {
                format!("needle {number}\n")
            } else {
                format!("context {number}\n")
            }
        })
        .collect::<String>();
    fs::write(root.path().join("source.rs"), source).unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetMode(SearchMode::Contents));
    session.command(Command::SetQuery("needle".into()));
    session.command(Command::SetPreviewRows(5));
    settled(&mut session);
    assert_eq!(session.state().preview.lines[2].text, "needle 260");
    let generation = session.state().generation;
    session.command(Command::SetPreviewRows(11));
    settled(&mut session);
    assert_eq!(session.state().preview.lines[5].text, "needle 260");
    assert_eq!(session.state().generation, generation);
    session.command(Command::ScrollPreview(-10));
    settled(&mut session);
    let manually_scrolled_top = session.state().preview.top;
    session.command(Command::SetPreviewRows(3));
    settled(&mut session);
    assert_eq!(session.state().preview.top, manually_scrolled_top);
    session.command(Command::ApplyScope("other".into()));
    settled(&mut session);
    session.command(Command::Back);
    settled(&mut session);
    session.command(Command::SetPreviewRows(9));
    settled(&mut session);
    assert_eq!(session.state().selected().unwrap().line, 260);
    assert_eq!(session.state().preview.top, manually_scrolled_top);
    session.command(Command::MoveSelection(1));
    settled(&mut session);
    assert_eq!(session.state().preview.lines[4].text, "needle 280");
    // The 64 KiB window can contain fewer lines than a tall viewport. The
    // selected source location must still appear in that bounded window.
    let long_line = format!("{}\n", "x".repeat(5000));
    fs::write(
        root.path().join("source.rs"),
        format!(
            "{}needle only260\n{}",
            long_line.repeat(259),
            long_line.repeat(40)
        ),
    )
    .unwrap();
    session.command(Command::SetPreviewRows(100));
    session.command(Command::SetQuery("only260".into()));
    settled(&mut session);
    assert!(
        session
            .state()
            .preview
            .lines
            .iter()
            .any(|line| line.text == "needle only260")
    );
}

#[test]
fn zero_width_regex_spans_and_content_read_errors_remain_observable() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempdir().unwrap();
    fs::write(root.path().join("empty-lines"), "start\n\n\nend\n").unwrap();
    let private = root.path().join("unreadable");
    fs::write(&private, "private needle\n").unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o0)).unwrap();
    let denied = fs::File::open(&private).is_err();
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetMode(SearchMode::Contents));
    session.command(Command::ToggleRegex);
    session.command(Command::SetQuery("^$".into()));
    settled(&mut session);
    assert_eq!(
        session
            .state()
            .results
            .iter()
            .map(|row| row.line)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(session.state().selected().unwrap().matches, vec![0..0]);
    assert_eq!(session.state().preview.lines[1].matches, vec![0..0]);
    if denied {
        assert_eq!(session.state().errors, 1);
        assert!(
            session
                .state()
                .scan_error
                .as_ref()
                .unwrap()
                .contains("unreadable")
        );
    }
    fs::set_permissions(&private, fs::Permissions::from_mode(0o600)).unwrap();
}
