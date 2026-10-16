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
fn paths(session: &SearchSession) -> Vec<&str> {
    session
        .state()
        .results
        .iter()
        .map(|row| row.relative.as_str())
        .collect()
}

#[test]
fn directories_share_fuzzy_filters_and_independent_traversal_settings() {
    let root = tempdir().unwrap();
    for name in [
        "Src",
        "src",
        "src-more",
        "other/source",
        "blocked/src",
        ".secret/src",
        "ignored/src",
    ] {
        fs::create_dir_all(root.path().join(name)).unwrap();
    }
    fs::write(root.path().join(".ignore"), "ignored/\n").unwrap();
    fs::write(root.path().join("src-file"), "file").unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetMode(SearchMode::Directories));
    session.command(Command::SetExclude("^blocked$".into()));
    settled(&mut session);
    assert_eq!(
        paths(&session),
        ["Src", "other", "other/source", "src", "src-more"]
    );
    session.command(Command::SetQuery("src".into()));
    settled(&mut session);
    assert_eq!(paths(&session), ["Src", "src", "src-more", "other/source"]);
    session.command(Command::SetQuery("Src".into()));
    settled(&mut session);
    assert_eq!(paths(&session), ["Src"]);
    session.command(Command::SetQuery("src".into()));
    session.command(Command::SetInclude("/src$".into()));
    session.command(Command::ToggleIgnored);
    settled(&mut session);
    assert_eq!(paths(&session), ["ignored/src"]);
    session.command(Command::ToggleHidden);
    settled(&mut session);
    assert_eq!(paths(&session), [".secret/src", "ignored/src"]);
    session.command(Command::SetMode(SearchMode::Files));
    settled(&mut session);
    assert_eq!(session.state().query, "src");
    assert_eq!(session.state().include, "/src$");
    assert!(session.state().hidden && session.state().ignored);
}

#[test]
fn directory_preview_lists_immediate_children_with_scope_relative_visibility() {
    let root = tempdir().unwrap();
    for name in ["dest/keep-sub/nested", "dest/drop-sub", "dest/keep-ignored"] {
        fs::create_dir_all(root.path().join(name)).unwrap();
    }
    for name in [
        "dest/keep.txt",
        "dest/drop.txt",
        "dest/.keep-hidden",
        "dest/.git",
    ] {
        fs::write(root.path().join(name), "source").unwrap();
    }
    fs::write(root.path().join(".ignore"), "keep-ignored/\n").unwrap();
    std::os::unix::fs::symlink(root.path(), root.path().join("dest/keep-link")).unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetMode(SearchMode::Directories));
    session.command(Command::SetQuery("dest".into()));
    session.command(Command::SetInclude(r"^dest($|/\.?keep)".into()));
    settled(&mut session);
    let children = |session: &SearchSession| {
        session
            .state()
            .preview
            .lines
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(children(&session), ["keep-sub/", "keep.txt"]);
    session.command(Command::ToggleHidden);
    session.command(Command::ToggleIgnored);
    settled(&mut session);
    assert_eq!(
        children(&session),
        [".keep-hidden", "keep-ignored/", "keep-sub/", "keep.txt"]
    );
    session.command(Command::SetExclude("^dest/keep-sub$".into()));
    settled(&mut session);
    assert_eq!(
        children(&session),
        [".keep-hidden", "keep-ignored/", "keep.txt"]
    );
}

#[test]
fn browse_clears_only_query_and_back_restores_mode_selection_preview_and_settings() {
    use dirpick::session::ScopePreset;
    let root = tempdir().unwrap();
    for name in ["dest-a", "dest-b"] {
        fs::create_dir(root.path().join(name)).unwrap();
    }
    for index in 0..60 {
        fs::write(
            root.path().join(format!("dest-b/child-{index:02}")),
            "source",
        )
        .unwrap();
    }
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetMode(SearchMode::Directories));
    session.command(Command::SetQuery("dest".into()));
    session.command(Command::SetInclude("^dest".into()));
    session.command(Command::SetExclude("blocked".into()));
    session.command(Command::ToggleHidden);
    session.command(Command::ToggleIgnored);
    settled(&mut session);
    session.command(Command::Select(1));
    settled(&mut session);
    session.command(Command::ScrollPreview(30));
    settled(&mut session);
    let before = session.state().clone();
    assert_eq!(before.preview.top, 30);
    session.command(Command::OpenSelected);
    settled(&mut session);
    assert_eq!(session.state().scope, root.path().join("dest-b"));
    assert_eq!(session.state().preset, ScopePreset::Custom);
    assert_eq!(session.state().query, "");
    assert_eq!(session.state().include, before.include);
    assert_eq!(session.state().exclude, before.exclude);
    assert!(session.state().hidden && session.state().ignored);
    assert_eq!(session.state().launch_cwd, root.path());
    session.command(Command::SetMode(SearchMode::Files));
    session.command(Command::SetInclude(String::new()));
    session.command(Command::ToggleHidden);
    session.command(Command::Back);
    settled(&mut session);
    assert_eq!(session.state().mode, before.mode);
    assert_eq!(session.state().scope, before.scope);
    assert_eq!(session.state().selected(), before.selected());
    assert_eq!(session.state().preview, before.preview);
    assert_eq!(session.state().query, before.query);
    assert_eq!(session.state().include, before.include);
    assert_eq!(session.state().exclude, before.exclude);
    assert_eq!(session.state().hidden, before.hidden);
    assert_eq!(session.state().ignored, before.ignored);
    session.command(Command::OpenSelected);
    settled(&mut session);
    session.command(Command::ToggleScope);
    settled(&mut session);
    assert_eq!(session.state().scope, root.path());
    session.command(Command::ToggleScope);
    settled(&mut session);
    assert_eq!(session.state().preset, ScopePreset::Cwd);
    assert_eq!(session.state().scope, root.path());
}

#[test]
fn directory_actions_require_integration_and_preserve_exact_json_paths() {
    use dirpick::{action::Action, session::Options};
    let root = tempdir().unwrap();
    let name = "-space 'quote' \"한글\"\t \n";
    fs::create_dir(root.path().join(name)).unwrap();
    let mut direct = SearchSession::open(root.path()).unwrap();
    direct.command(Command::SetMode(SearchMode::Directories));
    settled(&mut direct);
    direct.command(Command::ChangeDirectory);
    assert!(direct.take_action().is_none());
    assert!(direct.state().message.contains("--print-nu-wrapper"));
    assert_eq!(
        direct.state().selected().unwrap().path,
        root.path().join(name)
    );
    let mut session = SearchSession::with_options(
        root.path(),
        Options {
            shell_integration: true,
            ..Options::default()
        },
    )
    .unwrap();
    session.command(Command::SetMode(SearchMode::Directories));
    settled(&mut session);
    assert!(session.take_action().is_none());
    session.command(Command::ChangeDirectory);
    let action = session.take_action().unwrap();
    assert_eq!(
        action,
        Action::ChangeDirectory {
            path: root.path().join(name)
        }
    );
    let mut json = Vec::new();
    action.write_json(&mut json).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"action": "cd", "path": root.path().join(name).to_str().unwrap()})
    );
    session.command(Command::CopyPath);
    let copy = session.take_action().unwrap();
    let mut json = Vec::new();
    copy.write_json(&mut json).unwrap();
    assert!(json.is_empty());
    session.command(Command::ChangeDirectory);
    session.command(Command::SetQuery("no match".into()));
    assert!(
        session.take_action().is_none(),
        "a replacement search cannot return a stale action"
    );
    settled(&mut session);
    session.command(Command::ChangeDirectory);
    assert!(session.take_action().is_none());
}

#[test]
fn unsupported_or_vanished_directory_actions_retain_selection_without_partial_json() {
    use dirpick::{action::Action, session::Options};
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let root = tempdir().unwrap();
    let invalid = root
        .path()
        .join(OsString::from_vec(b"invalid-\xff".to_vec()));
    fs::create_dir(&invalid).unwrap();
    let mut session = SearchSession::with_options(
        root.path(),
        Options {
            shell_integration: true,
            ..Options::default()
        },
    )
    .unwrap();
    session.command(Command::SetMode(SearchMode::Directories));
    settled(&mut session);
    session.command(Command::ChangeDirectory);
    assert!(session.take_action().is_none());
    assert!(session.state().message.contains("UTF-8"));
    assert_eq!(session.state().selected().unwrap().path, invalid);
    let mut bytes = Vec::new();
    assert!(
        Action::ChangeDirectory { path: invalid }
            .write_json(&mut bytes)
            .is_err()
    );
    assert!(bytes.is_empty());
    let gone = root.path().join("gone");
    fs::create_dir(&gone).unwrap();
    session.command(Command::SetQuery("gone".into()));
    settled(&mut session);
    session.command(Command::ChangeDirectory);
    let action = session.take_action().unwrap();
    fs::remove_dir(&gone).unwrap();
    let result = action.write_json(&mut bytes);
    assert!(result.is_err());
    assert!(bytes.is_empty());
    session.complete_action(action, result);
    assert!(session.state().message.contains("search retained"));
    assert_eq!(session.state().selected().unwrap().path, gone);
}

#[test]
fn back_explains_a_vanished_directory_and_selects_a_remaining_result() {
    let root = tempdir().unwrap();
    for name in ["dest-a", "dest-b"] {
        fs::create_dir(root.path().join(name)).unwrap();
    }
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetMode(SearchMode::Directories));
    settled(&mut session);
    session.command(Command::OpenSelected);
    settled(&mut session);
    fs::remove_dir(root.path().join("dest-a")).unwrap();
    session.command(Command::Back);
    settled(&mut session);
    assert_eq!(session.state().selected().unwrap().relative, "dest-b");
    assert!(
        session
            .state()
            .message
            .contains("Previous selection unavailable")
    );
}
