use dirpick::session::{Command, SearchSession};
use std::{
    fs,
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
            "session did not settle: {:?}",
            session.state()
        );
        session.wait(Duration::from_millis(50));
    }
}

#[test]
fn real_files_can_be_searched_and_previewed() {
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(
        root.path().join("src/main.rs"),
        "fn main() {\n    println!(\"hello\");\n}\n",
    )
    .unwrap();
    fs::write(root.path().join("notes.txt"), "notes\n").unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    assert_eq!(session.state().results.len(), 2);
    session.command(Command::SetQuery("sr mn".into()));
    settled(&mut session);
    assert_eq!(session.state().results[0].relative, "src/main.rs");
    assert_eq!(
        session.state().selected().unwrap().path,
        root.path().join("src/main.rs")
    );
    assert_eq!(session.state().preview.lines[0].text, "fn main() {");
}

fn git(root: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn project_cwd_custom_scope_and_back_preserve_the_search() {
    use dirpick::session::ScopePreset;
    let root = tempdir().unwrap();
    git(root.path(), &["init", "-q"]);
    fs::create_dir(root.path().join("nested")).unwrap();
    fs::write(root.path().join("top.txt"), "root\n").unwrap();
    fs::write(root.path().join("nested/other.txt"), "nested\n").unwrap();
    let mut session = SearchSession::open(root.path().join("nested")).unwrap();
    settled(&mut session);
    assert_eq!(session.state().settings.scope, root.path());
    assert_eq!(session.state().preset, ScopePreset::Project);
    session.command(Command::SetQuery("txt".into()));
    settled(&mut session);
    session.command(Command::Select(1));
    settled(&mut session);
    session.command(Command::ToggleScope);
    settled(&mut session);
    assert_eq!(session.state().settings.query, "txt");
    assert_eq!(session.state().settings.scope, root.path().join("nested"));
    assert_eq!(session.state().preset, ScopePreset::Cwd);
    session.command(Command::ApplyScope("..".into()));
    settled(&mut session);
    assert_eq!(session.state().settings.query, "");
    session.command(Command::Back);
    settled(&mut session);
    assert_eq!(session.state().settings.query, "txt");
    assert_eq!(session.state().preset, ScopePreset::Cwd);
    session.command(Command::Back);
    settled(&mut session);
    assert_eq!(session.state().selected().unwrap().relative, "top.txt");
    let generation = session.state().generation;
    session.command(Command::ApplyScope("does-not-exist".into()));
    settled(&mut session);
    assert_eq!(session.state().generation, generation);
    assert!(session.state().message.contains("Cannot apply scope"));
}

#[test]
fn git_worktrees_submodules_and_fallback_resolve_the_working_tree_not_metadata() {
    use dirpick::session::{Options, ScopePreset};
    let fixture = tempdir().unwrap();
    let repository = fixture.path().join("repository");
    fs::create_dir(&repository).unwrap();
    git(&repository, &["init", "-q"]);
    git(
        &repository,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "fixture",
        ],
    );
    let worktree = fixture.path().join("linked");
    git(
        &repository,
        &[
            "worktree",
            "add",
            "-qb",
            "fixture-worktree",
            worktree.to_str().unwrap(),
        ],
    );
    fs::create_dir(worktree.join("nested")).unwrap();
    let session = SearchSession::open(worktree.join("nested")).unwrap();
    assert_eq!(session.state().settings.scope, worktree);
    assert!(!session.state().project_fallback);
    git(
        &repository,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            worktree.to_str().unwrap(),
            "module",
        ],
    );
    fs::create_dir(repository.join("module/nested")).unwrap();
    let session = SearchSession::open(repository.join("module/nested")).unwrap();
    assert_eq!(session.state().settings.scope, repository.join("module"));
    let mut session = SearchSession::with_options(
        &repository,
        Options {
            git_program: None,
            ..Options::default()
        },
    )
    .unwrap();
    assert!(session.state().project_fallback);
    assert_eq!(session.state().settings.scope, repository);
    assert_eq!(session.state().preset, ScopePreset::Project);
    session.command(Command::ToggleScope);
    assert_eq!(session.state().preset, ScopePreset::Cwd);
    assert_eq!(session.state().settings.scope, repository);
    session.command(Command::ToggleScope);
    assert_eq!(session.state().preset, ScopePreset::Project);
    let non_git = fixture.path().join("ordinary");
    fs::create_dir(&non_git).unwrap();
    assert!(
        SearchSession::open(&non_git)
            .unwrap()
            .state()
            .project_fallback
    );
    assert!(
        SearchSession::with_options(
            &repository,
            Options {
                git_program: Some("/nonexistent/git".into()),
                ..Options::default()
            }
        )
        .unwrap()
        .state()
        .project_fallback
    );
}

#[test]
fn fuzzy_smart_case_and_ranking_are_deterministic_and_results_are_bounded() {
    use dirpick::session::{Limits, Options};
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    for path in [
        "main.rs",
        "main.rs.old",
        "src/m-a-i-n.rs",
        "MAIN.rs",
        "other.txt",
    ] {
        fs::write(root.path().join(path), "source\n").unwrap();
    }
    let mut session = SearchSession::with_options(
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
    session.command(Command::SetQuery("main.rs".into()));
    settled(&mut session);
    assert_eq!(
        session
            .state()
            .results
            .iter()
            .map(|row| row.relative.as_str())
            .collect::<Vec<_>>(),
        ["MAIN.rs", "main.rs"]
    );
    assert!(session.state().result_limit);
    assert_eq!(session.state().matched, 4);
    assert_eq!(session.state().visited, 6);
    session.command(Command::SetQuery("MAIN".into()));
    settled(&mut session);
    assert_eq!(
        session
            .state()
            .results
            .iter()
            .map(|row| row.relative.as_str())
            .collect::<Vec<_>>(),
        ["MAIN.rs"]
    );
    session.command(Command::SetQuery("sr mn".into()));
    settled(&mut session);
    assert_eq!(session.state().results[0].relative, "src/m-a-i-n.rs");
    let mut limited = SearchSession::with_options(
        root.path(),
        Options {
            limits: Limits {
                entries: 2,
                results: 2,
            },
            ..Options::default()
        },
    )
    .unwrap();
    settled(&mut limited);
    assert_eq!(limited.state().visited, 2);
    assert!(limited.state().entry_limit);
}

#[test]
fn stale_generations_cannot_replace_results_and_streaming_keeps_selection_identity() {
    use dirpick::session::{SearchEvent, SearchResult};
    let root = tempdir().unwrap();
    for path in ["a.txt", "b.txt"] {
        fs::write(root.path().join(path), "source\n").unwrap();
    }
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    let obsolete = session.state().generation;
    session.command(Command::SetQuery("b".into()));
    settled(&mut session);
    let event = |generation, names: &[&str]| SearchEvent::Results {
        generation,
        rows: names
            .iter()
            .map(|name| SearchResult {
                path: root.path().join(name),
                relative: name.to_string(),
                line: 1,
                text: String::new(),
                matches: Vec::new(),
            })
            .collect(),
        visited: 2,
        matched: names.len(),
        errors: 0,
        entry_limit: false,
        done: false,
    };
    session.apply_event(event(obsolete, &["a.txt"]));
    assert_eq!(session.state().selected().unwrap().relative, "b.txt");
    let current = session.state().generation;
    session.apply_event(event(current, &["a.txt", "b.txt"]));
    assert_eq!(session.state().selected().unwrap().relative, "b.txt");
    assert_eq!(session.state().selected_index, 1);
}

#[test]
fn native_paths_survive_scope_history_and_preview_scrolling() {
    use dirpick::session::ScopePreset;
    let root = tempdir().unwrap();
    let project = root.path().join("git project\n");
    let custom = root.path().join("custom '한글' \n");
    fs::create_dir(&project).unwrap();
    fs::create_dir(&custom).unwrap();
    git(&project, &["init", "-q"]);
    let filename = "-quote ' unicode 한글 \n.txt";
    let source = (1..=300)
        .map(|line| format!("source line {line}\n"))
        .collect::<String>();
    fs::write(project.join(filename), source).unwrap();
    fs::write(custom.join("custom.txt"), "custom preview\n").unwrap();
    let mut session = SearchSession::open(&project).unwrap();
    settled(&mut session);
    assert_eq!(session.state().settings.scope, project);
    assert!(!session.state().project_fallback);
    assert_eq!(
        session.state().selected().unwrap().path,
        project.join(filename)
    );
    session.command(Command::ScrollPreview(30));
    settled(&mut session);
    assert_eq!(session.state().preview.lines[0].number, 31);
    assert!(session.state().preview.lines.len() <= 200);
    session.command(Command::ApplyScope(custom.clone()));
    settled(&mut session);
    assert_eq!(session.state().preset, ScopePreset::Custom);
    session.command(Command::Back);
    settled(&mut session);
    assert_eq!(
        session.state().selected().unwrap().path,
        project.join(filename)
    );
    assert_eq!(session.state().preview.lines[0].number, 31);
    session.command(Command::ApplyScope(custom));
    settled(&mut session);
    session.command(Command::ToggleScope);
    settled(&mut session);
    assert_eq!(session.state().preset, ScopePreset::Project);
    assert_eq!(session.state().settings.scope, project);
}

#[test]
fn hidden_metadata_symlinks_and_binary_preview_remain_safe() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("visible.txt"), "text\n").unwrap();
    fs::write(root.path().join("binary.dat"), b"\x00\x01").unwrap();
    fs::write(root.path().join(".secret"), "hidden").unwrap();
    fs::write(root.path().join(".ignore"), "!.secret\n").unwrap();
    fs::write(root.path().join(".git"), "metadata").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.path(), root.path().join("cycle")).unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    assert_eq!(
        session
            .state()
            .results
            .iter()
            .map(|row| row.relative.as_str())
            .collect::<Vec<_>>(),
        ["binary.dat", "visible.txt"]
    );
    assert!(session.state().preview.status.contains("Binary"));
}

#[test]
fn applying_scope_is_pending_and_does_not_replace_the_search_before_validation() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("kept.txt"), "original\n").unwrap();
    let mut session = SearchSession::open(root.path()).unwrap();
    settled(&mut session);
    let generation = session.state().generation;
    session.command(Command::ApplyScope("missing".into()));
    assert!(session.state().scope_loading);
    assert_eq!(session.state().generation, generation);
    assert_eq!(session.state().selected().unwrap().relative, "kept.txt");
    settled(&mut session);
    assert_eq!(session.state().generation, generation);
    assert!(session.state().message.contains("Cannot apply scope"));
}

#[test]
fn exact_basename_ranking_preserves_spaces_in_a_multi_token_query() {
    let root = tempdir().unwrap();
    for name in ["readme", "read me", "read me notes"] {
        fs::write(root.path().join(name), "source\n").unwrap();
    }
    let mut session = SearchSession::open(root.path()).unwrap();
    session.command(Command::SetQuery("read me".into()));
    settled(&mut session);
    assert_eq!(
        session
            .state()
            .results
            .iter()
            .map(|row| row.relative.as_str())
            .collect::<Vec<_>>(),
        ["read me", "read me notes", "readme"]
    );
}
