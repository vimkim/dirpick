use dirpick::{
    action::{Action, EditorCommand},
    session::{Command, SearchMode, SearchSession},
};
use std::{
    ffi::OsStr,
    fs,
    time::{Duration, Instant},
};
use tempfile::tempdir;

fn settled(session: &mut SearchSession) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while session.state().scanning || session.state().preview.loading {
        assert!(Instant::now() < deadline);
        session.wait(Duration::from_millis(20));
    }
}

#[test]
fn editor_arguments_preserve_native_paths_and_resolve_precedence_without_shell_evaluation() {
    let path = std::path::Path::new("/tmp/-quoted '한글' \t\n.rs");
    let editor = EditorCommand::resolve(
        Some(OsStr::new("'/fake path/nvim' -u 'no config'")),
        Some(OsStr::new("ignored")),
        path,
        42,
        true,
    )
    .unwrap();
    assert_eq!(editor.program, OsStr::new("/fake path/nvim"));
    assert_eq!(
        editor.args,
        [
            OsStr::new("-u"),
            OsStr::new("no config"),
            OsStr::new("+42"),
            OsStr::new("--"),
            path.as_os_str()
        ]
    );
    let editor = EditorCommand::resolve(
        None,
        Some(OsStr::new("vim '$HOME' '$(touch bad)'")),
        path,
        1,
        false,
    )
    .unwrap();
    assert_eq!(editor.program, "vim");
    assert_eq!(editor.args[0], "$HOME");
    assert_eq!(editor.args[1], "$(touch bad)");
    assert_eq!(
        EditorCommand::resolve(None, None, path, 1, false)
            .unwrap()
            .program,
        "nvim"
    );
    assert!(
        EditorCommand::resolve(Some(OsStr::new("vim '")), None, path, 1, false)
            .unwrap_err()
            .to_string()
            .contains("quot")
    );
    let unsupported =
        EditorCommand::resolve(Some(OsStr::new("code --wait")), None, path, 2, true).unwrap_err();
    assert!(unsupported.to_string().contains("Vim/Neovim"));
    assert!(EditorCommand::resolve(Some(OsStr::new("code --wait")), None, path, 1, false).is_ok());
}

#[test]
fn opening_files_and_matches_requests_the_exact_line_and_preserves_state_on_return() {
    let root = tempdir().unwrap();
    let path = root.path().join("source.rs");
    fs::write(&path, format!("{}needle\n", "context\n".repeat(250))).unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    session.command(Command::OpenSelected);
    assert_eq!(
        session.take_action(),
        Some(Action::OpenEditor {
            path: path.clone(),
            line: 1
        })
    );
    session.command(Command::SetMode(SearchMode::Contents));
    session.command(Command::SetQuery("needle".into()));
    settled(&mut session);
    session.command(Command::ScrollPreview(1));
    settled(&mut session);
    let before = session.state().clone();
    for result in [
        Ok(()),
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "fake editor missing",
        )),
    ] {
        session.command(Command::OpenSelected);
        let action = session.take_action().unwrap();
        assert_eq!(
            action,
            Action::OpenEditor {
                path: path.clone(),
                line: 251
            }
        );
        session.complete_action(action, result);
        assert_eq!(session.state().generation, before.generation);
        assert_eq!(session.state().query, before.query);
        assert_eq!(session.state().selected(), before.selected());
        assert_eq!(session.state().preview, before.preview);
    }
    assert!(session.state().message.contains("fake editor missing"));
    assert!(session.state().message.contains("retained"));
}
