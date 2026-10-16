use dirpick::session::{Command, SearchSession};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
use tempfile::tempdir;

fn settled(session: &mut SearchSession) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while session.state().scanning
        || session.state().preview.loading
        || session.state().scope_loading
    {
        assert!(
            Instant::now() < deadline,
            "unsettled: {:?}",
            session.state()
        );
        session.wait(Duration::from_millis(20));
    }
}

fn write(root: &Path, path: &str, text: &str) {
    fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
    fs::write(root.join(path), text).unwrap();
}

fn paths(session: &SearchSession) -> Vec<&str> {
    session
        .state()
        .results
        .iter()
        .map(|row| row.relative.as_str())
        .collect()
}

fn git(root: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn filters_match_relative_paths_with_independent_smart_case_and_exclude_wins() {
    let root = tempdir().unwrap();
    for path in [
        "src/Keep.rs",
        "src/keep.rs",
        "src/private/keep.rs",
        "assets/keep.rs",
    ] {
        write(root.path(), path, "source\n");
    }
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetInclude(r"^src/.*\.rs$".into()));
    session.command(Command::SetExclude(r"^src/private$".into()));
    settled(&mut session);
    assert_eq!(paths(&session), ["src/Keep.rs", "src/keep.rs"]);

    session.command(Command::SetQuery("Keep".into()));
    settled(&mut session);
    assert_eq!(paths(&session), ["src/Keep.rs"]);
    session.command(Command::SetQuery("keep".into()));
    session.command(Command::SetExclude("Keep|private".into()));
    settled(&mut session);
    assert_eq!(paths(&session), ["src/keep.rs"]);
    session.command(Command::SetExclude("keep|private".into()));
    settled(&mut session);
    assert!(paths(&session).is_empty());
    session.command(Command::SetExclude(String::new()));
    session.command(Command::SetInclude(r"^SRC/.*\.rs$".into()));
    settled(&mut session);
    assert!(paths(&session).is_empty());
}

#[test]
fn invalid_filters_clear_stale_results_validate_empty_trees_and_recover() {
    let root = tempdir().unwrap();
    write(root.path(), "kept.rs", "source\n");
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    assert_eq!(paths(&session), ["kept.rs"]);
    session.command(Command::SetInclude("[".into()));
    assert!(session.state().results.is_empty());
    assert!(session.state().preview.lines.is_empty());
    settled(&mut session);
    assert!(session.state().include_error.is_some());
    assert!(session.state().exclude_error.is_none());
    assert_eq!(session.state().visited, 0);

    session.command(Command::SetExclude("(".into()));
    settled(&mut session);
    assert!(session.state().include_error.is_some());
    assert!(session.state().exclude_error.is_some());
    session.command(Command::SetInclude(r"\.rs$".into()));
    settled(&mut session);
    assert!(session.state().include_error.is_none());
    assert!(session.state().exclude_error.is_some());
    session.command(Command::SetExclude(String::new()));
    settled(&mut session);
    assert_eq!(paths(&session), ["kept.rs"]);
    assert!(session.state().exclude_error.is_none());

    fs::remove_file(root.path().join("kept.rs")).unwrap();
    session.command(Command::SetInclude("[".into()));
    settled(&mut session);
    assert!(session.state().include_error.is_some());
    assert!(session.state().results.is_empty());
}

#[test]
fn hidden_and_ignored_are_independent_even_for_whitelists_and_skip_metadata_and_links() {
    let root = tempdir().unwrap();
    git(root.path(), &["init", "-q"]);
    for path in [
        "visible.txt",
        ".hidden-visible.txt",
        ".hidden-ignored.txt",
        ".private/visible.txt",
        "git-ignored.txt",
        "ordinary-ignored.txt",
        "rg-ignored.txt",
        "nested/.git",
        ".git/secret.txt",
    ] {
        write(root.path(), path, "source\n");
    }
    write(
        root.path(),
        ".gitignore",
        "git-ignored.txt\n.hidden-ignored.txt\n",
    );
    write(
        root.path(),
        ".ignore",
        "ordinary-ignored.txt\n!.hidden-visible.txt\n",
    );
    write(root.path(), ".rgignore", "rg-ignored.txt\n");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.path(), root.path().join("cycle")).unwrap();
        std::os::unix::fs::symlink(
            root.path().join("visible.txt"),
            root.path().join("linked.txt"),
        )
        .unwrap();
    }
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetInclude(r"\.txt$".into()));
    settled(&mut session);
    assert_eq!(paths(&session), ["visible.txt"]);
    session.command(Command::ToggleHidden);
    settled(&mut session);
    assert_eq!(
        paths(&session),
        [".hidden-visible.txt", ".private/visible.txt", "visible.txt"]
    );
    session.command(Command::ToggleIgnored);
    settled(&mut session);
    assert_eq!(
        paths(&session),
        [
            ".hidden-ignored.txt",
            ".hidden-visible.txt",
            ".private/visible.txt",
            "git-ignored.txt",
            "ordinary-ignored.txt",
            "rg-ignored.txt",
            "visible.txt",
        ]
    );
    session.command(Command::SetInclude(String::new()));
    settled(&mut session);
    assert!(
        !paths(&session)
            .iter()
            .any(|path| path.split('/').any(|part| part == ".git")
                || path.starts_with("cycle")
                || *path == "linked.txt")
    );
    assert!(!session.state().entry_limit);
    session.command(Command::SetInclude(r"\.txt$".into()));
    session.command(Command::ToggleHidden);
    settled(&mut session);
    assert_eq!(
        paths(&session),
        [
            "git-ignored.txt",
            "ordinary-ignored.txt",
            "rg-ignored.txt",
            "visible.txt"
        ]
    );
    session.command(Command::SetExclude("ordinary|rg".into()));
    settled(&mut session);
    assert_eq!(paths(&session), ["git-ignored.txt", "visible.txt"]);
    session.command(Command::ToggleIgnored);
    settled(&mut session);
    assert_eq!(paths(&session), ["visible.txt"]);
}

#[test]
fn scopes_keep_filters_and_toggles_and_back_restores_the_previous_refinement() {
    let root = tempdir().unwrap();
    git(root.path(), &["init", "-q"]);
    write(root.path(), "top.rs", "top\n");
    write(root.path(), "nested/child.rs", "nested\n");
    write(root.path(), "nested/notes.txt", "notes\n");
    let mut session = SearchSession::open(root.path().join("nested")).unwrap();
    session.command(Command::SetQuery("rs".into()));
    session.command(Command::SetInclude(r"\.rs$".into()));
    session.command(Command::SetExclude("private".into()));
    session.command(Command::ToggleHidden);
    session.command(Command::ToggleIgnored);
    settled(&mut session);
    session.command(Command::ToggleScope);
    settled(&mut session);
    assert_eq!(session.state().query, "rs");
    assert_eq!(session.state().include, r"\.rs$");
    assert_eq!(session.state().exclude, "private");
    assert!(session.state().hidden && session.state().ignored);
    assert_eq!(paths(&session), ["child.rs"]);

    session.command(Command::SetInclude(r"\.txt$".into()));
    session.command(Command::SetExclude("other".into()));
    session.command(Command::ToggleHidden);
    session.command(Command::ToggleIgnored);
    session.command(Command::Back);
    settled(&mut session);
    assert_eq!(session.state().include, r"\.rs$");
    assert_eq!(session.state().exclude, "private");
    assert!(session.state().hidden && session.state().ignored);
    assert_eq!(paths(&session), ["nested/child.rs", "top.rs"]);
    session.command(Command::ApplyScope("nested".into()));
    settled(&mut session);
    assert_eq!(session.state().query, "");
    assert_eq!(session.state().include, r"\.rs$");
    assert_eq!(session.state().exclude, "private");
    assert!(session.state().hidden && session.state().ignored);
}

#[test]
fn parent_ignore_rules_and_ripgrep_precedence_apply_in_project_cwd_and_custom_scopes() {
    let fixture = tempdir().unwrap();
    write(fixture.path(), ".gitignore", "git-parent.txt\n");
    write(fixture.path(), ".ignore", "ordinary-parent.txt\n");
    write(fixture.path(), ".rgignore", "rg-parent.txt\n");
    let root = fixture.path().join("repository");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-q"]);
    write(&root, ".gitignore", "from-git.txt\nprefer-rg.txt\n");
    write(&root, ".ignore", "ordinary.txt\n!prefer-rg.txt\n");
    write(&root, ".rgignore", "ripgrep.txt\nprefer-rg.txt\n");
    write(&root, ".git/info/exclude", "info-excluded.txt\n");
    let names = [
        "keep.txt",
        "git-parent.txt",
        "ordinary-parent.txt",
        "rg-parent.txt",
        "from-git.txt",
        "ordinary.txt",
        "ripgrep.txt",
        "prefer-rg.txt",
        "info-excluded.txt",
    ];
    for prefix in ["", "nested/", "nested/custom/"] {
        for name in names {
            write(&root, &format!("{prefix}{name}"), "source\n");
        }
    }
    let mut session = SearchSession::open(root.join("nested")).unwrap();
    session.command(Command::SetInclude(r"\.txt$".into()));
    settled(&mut session);
    assert_eq!(
        paths(&session),
        ["keep.txt", "nested/custom/keep.txt", "nested/keep.txt"]
    );
    session.command(Command::ToggleScope);
    settled(&mut session);
    assert_eq!(paths(&session), ["custom/keep.txt", "keep.txt"]);
    session.command(Command::ApplyScope("custom".into()));
    settled(&mut session);
    assert_eq!(paths(&session), ["keep.txt"]);
    session.command(Command::ToggleIgnored);
    settled(&mut session);
    assert_eq!(paths(&session).len(), names.len());
    assert!(paths(&session).contains(&"prefer-rg.txt"));
    session.command(Command::SetExclude("parent|git".into()));
    settled(&mut session);
    assert_eq!(
        paths(&session),
        [
            "info-excluded.txt",
            "keep.txt",
            "ordinary.txt",
            "prefer-rg.txt",
            "ripgrep.txt"
        ]
    );
}

#[cfg(unix)]
#[test]
fn unreadable_and_vanished_paths_report_scan_failures_without_losing_other_results() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = tempdir().unwrap();
    let root = fixture.path().join("scope");
    write(&root, "readable.txt", "source\n");
    write(&root, "locked/private.txt", "source\n");
    fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o0)).unwrap();
    let mut session = SearchSession::open(&root).unwrap();
    settled(&mut session);
    fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(paths(&session), ["readable.txt"]);
    assert!(session.state().errors > 0);
    assert!(
        session
            .state()
            .scan_error
            .as_ref()
            .unwrap()
            .contains("locked")
    );

    fs::remove_dir_all(&root).unwrap();
    session.command(Command::SetInclude(r"\.txt$".into()));
    settled(&mut session);
    assert!(session.state().results.is_empty());
    assert!(session.state().errors > 0);
    assert!(
        session
            .state()
            .scan_error
            .as_ref()
            .unwrap()
            .contains("scope")
    );
}

#[test]
fn refinement_replaces_busy_scans_rejects_late_events_and_preserves_limit_status() {
    use dirpick::session::{Limits, Options, SearchEvent, SearchResult};
    let root = tempdir().unwrap();
    for number in 0..600 {
        write(
            root.path(),
            &format!("candidate-{number:03}.rs"),
            "source\n",
        );
    }
    write(root.path(), "target.rs", "target\n");
    write(root.path(), "blocked/target.rs", "excluded\n");
    write(root.path(), ".target.rs", "hidden\n");
    let options = Options {
        limits: Limits {
            entries: 50_000,
            results: 2,
        },
        ..Options::default()
    };
    let mut session = SearchSession::with_options(root.path(), options).unwrap();
    settled(&mut session);
    assert!(session.state().result_limit);
    assert!(!session.state().entry_limit);
    assert_eq!(session.state().matched, 602);
    let old = session.state().generation;
    let start = Instant::now();
    for number in 0..100 {
        session.command(Command::SetQuery(format!("candidate-{number}")));
        session.command(Command::SetInclude(format!("{number}|target")));
        session.command(Command::ToggleHidden);
        session.command(Command::ToggleIgnored);
    }
    session.command(Command::SetQuery("target".into()));
    session.command(Command::SetInclude(r"\.rs$".into()));
    session.command(Command::SetExclude("^blocked$".into()));
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "input commands blocked on a scan"
    );
    assert!(session.state().results.is_empty());
    settled(&mut session);
    assert_eq!(paths(&session), ["target.rs"]);
    assert!(!session.state().result_limit);
    session.apply_event(SearchEvent::Results {
        generation: old,
        rows: vec![SearchResult {
            path: root.path().join("stale"),
            relative: "stale".into(),
            line: 1,
        }],
        visited: 1,
        matched: 1,
        errors: 0,
        entry_limit: true,
        done: true,
    });
    session.apply_event(SearchEvent::InvalidFilters {
        generation: old,
        include: Some("obsolete".into()),
        exclude: None,
    });
    session.apply_event(SearchEvent::ScanError {
        generation: old,
        message: "obsolete".into(),
    });
    assert_eq!(paths(&session), ["target.rs"]);
    assert!(session.state().include_error.is_none());
    assert!(session.state().scan_error.is_none());
    assert!(!session.state().entry_limit);

    let mut limited = SearchSession::with_options(
        root.path(),
        Options {
            limits: Limits {
                entries: 50,
                results: 2,
            },
            ..Options::default()
        },
    )
    .unwrap();
    settled(&mut limited);
    assert_eq!(limited.state().visited, 50);
    assert!(limited.state().entry_limit && limited.state().result_limit);
}

#[cfg(unix)]
#[test]
fn explicit_hidden_and_symlink_scopes_resolve_but_git_metadata_remains_excluded() {
    let root = tempdir().unwrap();
    write(root.path(), ".private/visible.rs", "source\n");
    write(root.path(), ".private/.nested/inside.rs", "hidden\n");
    write(root.path(), ".private/.git/metadata.rs", "metadata\n");
    std::os::unix::fs::symlink(
        root.path().join(".private"),
        root.path().join("chosen-link"),
    )
    .unwrap();
    std::os::unix::fs::symlink(root.path(), root.path().join(".private/cycle")).unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    assert!(paths(&session).is_empty());
    session.command(Command::ApplyScope("chosen-link".into()));
    settled(&mut session);
    assert_eq!(session.state().scope, root.path().join(".private"));
    assert_eq!(paths(&session), ["visible.rs"]);
    session.command(Command::ToggleHidden);
    session.command(Command::ToggleIgnored);
    settled(&mut session);
    assert_eq!(paths(&session), [".nested/inside.rs", "visible.rs"]);
    session.command(Command::ApplyScope(".git".into()));
    settled(&mut session);
    assert!(
        paths(&session).is_empty(),
        "Git metadata must remain excluded for an explicit scope"
    );
}
