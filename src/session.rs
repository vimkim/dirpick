//! Application boundary: commands in, observable state and generation events out.
use crate::{
    action::Action,
    preview::{self, PreviewRequest},
    search::{self, SearchRequest},
    traversal::VisibilityPolicy,
    worker::Worker,
};
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub entries: usize,
    pub results: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            entries: 50_000,
            results: 2_000,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchMode {
    Files,
    Directories,
    Contents,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopePreset {
    Project,
    Cwd,
    Custom,
}
#[derive(Clone, Debug)]
pub struct Options {
    pub git_program: Option<OsString>,
    pub limits: Limits,
    pub shell_integration: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            git_program: Some("git".into()),
            limits: Limits::default(),
            shell_integration: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult {
    pub path: PathBuf,
    pub relative: String,
    pub line: usize,
    /// Original matching source line; empty on file/directory path rows.
    pub text: String,
    /// Every half-open raw UTF-8 match range in `text`.
    pub matches: Vec<std::ops::Range<usize>>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreviewSpan {
    /// Half-open UTF-8 byte offsets in `PreviewLine::text`, before display escaping.
    pub range: std::ops::Range<usize>,
    pub foreground: [u8; 3],
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreviewLine {
    pub number: usize,
    /// Original source text; control characters are escaped only when rendered.
    pub text: String,
    /// Syntax styles over the raw source. Empty means plain-text fallback.
    pub spans: Vec<PreviewSpan>,
    /// Half-open UTF-8 byte ranges in `text`, overlaid after syntax styles.
    pub matches: Vec<std::ops::Range<usize>>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preview {
    pub lines: Vec<PreviewLine>,
    pub top: usize,
    pub total_lines: usize,
    pub loading: bool,
    pub status: String,
}
/// Search inputs move together through observable state, history, and workers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchSettings {
    pub mode: SearchMode,
    /// Retained across modes; effective only in Contents.
    pub regex: bool,
    pub scope: PathBuf,
    pub query: String,
    pub include: String,
    pub exclude: String,
    pub hidden: bool,
    pub ignored: bool,
}
#[derive(Clone, Debug)]
pub struct State {
    pub settings: SearchSettings,
    pub query_error: Option<String>,
    pub skipped: usize,
    pub launch_cwd: PathBuf,
    pub project_root: PathBuf,
    pub project_fallback: bool,
    pub preset: ScopePreset,
    pub include_error: Option<String>,
    pub exclude_error: Option<String>,
    pub results: Vec<SearchResult>,
    pub selected_index: usize,
    pub generation: u64,
    pub scanning: bool,
    pub scope_loading: bool,
    pub visited: usize,
    pub matched: usize,
    pub errors: usize,
    pub scan_error: Option<String>,
    pub entry_limit: bool,
    pub result_limit: bool,
    pub preview: Preview,
    pub message: String,
}
impl State {
    pub fn selected(&self) -> Option<&SearchResult> {
        self.results.get(self.selected_index)
    }
}
#[derive(Debug)]
pub enum Command {
    SetMode(SearchMode),
    ToggleRegex,
    OpenSelected,
    ChangeDirectory,
    SetQuery(String),
    SetInclude(String),
    SetExclude(String),
    ToggleHidden,
    ToggleIgnored,
    Select(usize),
    MoveSelection(isize),
    /// Number of source/child rows visible in the preview, excluding its chrome.
    SetPreviewRows(usize),
    ScrollPreview(isize),
    /// Scroll by visible pages, overlapping one source/child row when possible.
    PagePreview(isize),
    ToggleScope,
    ApplyScope(PathBuf),
    Back,
    CopyPath,
}
#[derive(Clone, Debug)]
pub enum SearchEvent {
    InvalidQuery {
        generation: u64,
        message: String,
    },
    ContentStats {
        generation: u64,
        skipped: usize,
        result_limit: bool,
    },
    ScanError {
        generation: u64,
        message: String,
    },
    InvalidFilters {
        generation: u64,
        include: Option<String>,
        exclude: Option<String>,
    },
    Results {
        generation: u64,
        rows: Vec<SearchResult>,
        visited: usize,
        matched: usize,
        errors: usize,
        entry_limit: bool,
        done: bool,
    },
    Scope {
        id: u64,
        result: Result<PathBuf, String>,
    },
    Preview {
        id: u64,
        preview: Preview,
    },
}
struct HistoryEntry {
    settings: SearchSettings,
    preset: ScopePreset,
    target: Option<SelectionTarget>,
}

struct SelectionTarget {
    path: PathBuf,
    line: usize,
    top: usize,
    following_selection: bool,
}

pub struct SearchSession {
    state: State,
    limits: Limits,
    events: Receiver<SearchEvent>,
    search: Worker<SearchRequest>,
    preview: Worker<PreviewRequest>,
    preview_id: u64,
    scope_worker: Worker<PathBuf>,
    scope_id: u64,
    history: Vec<HistoryEntry>,
    restore: Option<SelectionTarget>,
    preview_rows: usize,
    preview_follows_selection: bool,
    action: Option<Action>,
    shell_integration: bool,
}
impl SearchSession {
    pub fn open(cwd: impl AsRef<Path>) -> io::Result<Self> {
        Self::with_options(cwd, Options::default())
    }
    pub fn with_options(cwd: impl AsRef<Path>, options: Options) -> io::Result<Self> {
        let launch_cwd = cwd.as_ref().canonicalize()?;
        if !launch_cwd.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "Launch CWD must be a directory",
            ));
        }
        let discovered = options
            .git_program
            .as_ref()
            .and_then(|program| project_root(program, &launch_cwd));
        let project_fallback = discovered.is_none();
        let scope = discovered.unwrap_or_else(|| launch_cwd.clone());
        let (tx, events) = mpsc::sync_channel(8);
        let preview_tx = tx.clone();
        let scope_tx = tx.clone();
        let mut session = Self {
            state: State {
                settings: SearchSettings {
                    mode: SearchMode::Files,
                    regex: false,
                    scope: scope.clone(),
                    query: String::new(),
                    include: String::new(),
                    exclude: String::new(),
                    hidden: false,
                    ignored: false,
                },
                query_error: None,
                skipped: 0,
                project_root: scope.clone(),
                launch_cwd,
                project_fallback,
                preset: ScopePreset::Project,
                include_error: None,
                exclude_error: None,
                results: Vec::new(),
                selected_index: 0,
                generation: 0,
                scanning: false,
                scope_loading: false,
                visited: 0,
                matched: 0,
                errors: 0,
                scan_error: None,
                entry_limit: false,
                result_limit: false,
                preview: Preview::default(),
                message: String::new(),
            },
            limits: options.limits,
            events,
            search: Worker::new("dirpick-search", move |id, request, cancel| {
                search::run(id, request, &tx, cancel)
            }),
            preview: Worker::new("dirpick-preview", move |id, request, cancel| {
                preview::run(id, request, &preview_tx, cancel)
            }),
            preview_id: 0,
            scope_id: 0,
            scope_worker: Worker::new("dirpick-scope", move |id, path: PathBuf, cancel| {
                let result = path
                    .canonicalize()
                    .and_then(|path| {
                        if path.is_dir() {
                            Ok(path)
                        } else {
                            Err(io::Error::new(
                                io::ErrorKind::NotADirectory,
                                "not a directory",
                            ))
                        }
                    })
                    .map_err(|error| error.to_string());
                search::deliver(&scope_tx, SearchEvent::Scope { id, result }, &cancel, true);
            }),
            history: Vec::new(),
            restore: None,
            preview_rows: 20,
            preview_follows_selection: true,
            action: None,
            shell_integration: options.shell_integration,
        };
        session.refresh();
        Ok(session)
    }
    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn command(&mut self, command: Command) {
        match command {
            Command::ChangeDirectory => {
                self.action = None;
                if self.state.settings.mode != SearchMode::Directories
                    || self.state.selected().is_none()
                {
                    self.state.message =
                        "Select a directory in F2 Directories for cd & exit".into();
                } else if !self.shell_integration {
                    self.state.message = "Nushell setup: dirpick --print-nu-wrapper; use the dp wrapper for cd & exit".into();
                } else if let Some(row) = self.state.selected() {
                    if row.path.is_absolute() && row.path.to_str().is_some() {
                        self.action = Some(Action::ChangeDirectory {
                            path: row.path.clone(),
                        });
                    } else {
                        self.state.message =
                            "Cannot cd: JSON requires an absolute UTF-8 path".into();
                    }
                }
            }
            Command::OpenSelected => {
                self.action = None;
                if self.state.settings.mode == SearchMode::Directories {
                    if let Some(row) = self.state.selected() {
                        self.command(Command::ApplyScope(row.path.clone()));
                    } else {
                        self.state.message = "No directory selected to browse".into();
                    }
                    return;
                }
                if let Some(row) = self.state.selected() {
                    match crate::action::EditorCommand::from_environment(
                        &row.path,
                        row.line,
                        self.state.settings.mode == SearchMode::Contents,
                    ) {
                        Ok(_) => {
                            self.action = Some(Action::OpenEditor {
                                path: row.path.clone(),
                                line: row.line,
                            })
                        }
                        Err(error) => {
                            self.state.message =
                                format!("Cannot open editor: {error} · search retained")
                        }
                    }
                } else {
                    self.state.message = "Nothing selected to open".into();
                }
            }
            Command::SetMode(mode) => {
                if mode != self.state.settings.mode {
                    self.state.settings.mode = mode;
                    self.refresh();
                }
            }
            Command::ToggleRegex => {
                self.state.settings.regex = !self.state.settings.regex;
                if self.state.settings.mode == SearchMode::Contents {
                    self.refresh();
                }
            }
            Command::CopyPath => {
                self.action = None;
                if let Some(row) = self.state.selected() {
                    if row.path.to_str().is_some() && row.path.is_absolute() {
                        self.action = Some(Action::CopyPath {
                            path: row.path.clone(),
                        });
                        self.state.message = "Sending OSC 52 clipboard payload…".into();
                    } else {
                        self.state.message =
                            "Cannot copy: OSC 52 requires an absolute UTF-8 path".into();
                    }
                } else {
                    self.state.message = "Nothing selected to copy".into();
                }
            }
            Command::ToggleScope => {
                let (scope, preset) = if self.state.preset == ScopePreset::Project {
                    (self.state.launch_cwd.clone(), ScopePreset::Cwd)
                } else {
                    (self.state.project_root.clone(), ScopePreset::Project)
                };
                self.change_scope(scope, preset, false);
            }
            Command::ApplyScope(path) => {
                let path = if path.is_absolute() {
                    path
                } else {
                    self.state.settings.scope.join(path)
                };
                self.scope_id += 1;
                self.state.scope_loading = true;
                self.scope_worker.submit(self.scope_id, path);
            }
            Command::Back => {
                self.cancel_scope();
                if let Some(previous) = self.history.pop() {
                    let target = previous.target;
                    self.state.settings = previous.settings;
                    self.state.preset = previous.preset;
                    self.refresh();
                    self.restore = target;
                    self.state.message = "Previous scope and search restored".into();
                } else {
                    self.state.message = "No earlier scope".into();
                }
            }
            Command::SetQuery(query) => {
                if self.state.settings.query != query {
                    self.state.settings.query = query;
                    self.refresh();
                }
            }
            Command::SetInclude(include) => {
                if self.state.settings.include != include {
                    self.state.settings.include = include;
                    self.refresh();
                }
            }
            Command::SetExclude(exclude) => {
                if self.state.settings.exclude != exclude {
                    self.state.settings.exclude = exclude;
                    self.refresh();
                }
            }
            Command::ToggleHidden => {
                self.state.settings.hidden = !self.state.settings.hidden;
                self.refresh();
            }
            Command::ToggleIgnored => {
                self.state.settings.ignored = !self.state.settings.ignored;
                self.refresh();
            }
            Command::Select(index) => self.select(index),
            Command::MoveSelection(delta) => {
                self.select(self.state.selected_index.saturating_add_signed(delta))
            }
            Command::ScrollPreview(delta) => {
                self.preview_follows_selection = false;
                let top = self.state.preview.top.saturating_add_signed(delta);
                // Before the first preview arrives, its length is unknown.
                // The worker will clamp the requested position after reading.
                self.state.preview.top = if self.state.preview.total_lines == 0 {
                    top
                } else {
                    top.min(self.state.preview.total_lines - 1)
                };
                self.request_preview();
            }
            Command::PagePreview(pages) => {
                // A byte-limited window can be shorter than the viewport.
                // Advance past only the rows actually shown. Backward paging
                // uses the viewport so a short final page does not shrink it.
                let visible = if pages > 0 {
                    self.preview_rows.min(self.state.preview.lines.len()).max(1)
                } else {
                    self.preview_rows
                };
                let step = visible.saturating_sub(1).max(1) as isize;
                self.command(Command::ScrollPreview(pages.saturating_mul(step)));
            }
            Command::SetPreviewRows(rows) => {
                let rows = rows.clamp(1, 200);
                if rows != self.preview_rows {
                    self.preview_rows = rows;
                    if self.preview_follows_selection {
                        let top = self.centered_preview_top();
                        if top != self.state.preview.top {
                            self.state.preview.top = top;
                            self.request_preview();
                        }
                    }
                }
            }
        }
    }
    pub fn poll(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            self.apply_event(event);
        }
    }
    /// Consume a requested effect; the runtime supplies the terminal transport.
    pub fn take_action(&mut self) -> Option<Action> {
        self.action.take()
    }
    pub fn complete_action(&mut self, action: Action, result: io::Result<()>) {
        match action {
            Action::ChangeDirectory { .. } => {
                if let Err(error) = result {
                    self.state.message = format!("Cannot cd & exit: {error} · search retained");
                }
            }
            Action::OpenEditor { .. } => {
                self.state.message = match result {
                    Ok(()) => "Editor closed · search retained".into(),
                    Err(error) => format!("Editor failed: {error} · search retained"),
                };
            }
            Action::CopyPath { .. } => {
                self.state.message = match result {
                    Ok(()) => "OSC 52 payload sent · client acceptance unknown".into(),
                    Err(error) => format!("Copy failed: {error} · search retained"),
                };
            }
        }
    }
    pub fn wait(&mut self, timeout: Duration) {
        if let Ok(event) = self.events.recv_timeout(timeout) {
            self.apply_event(event);
        }
        self.poll();
    }
    /// Accept a worker event only while its identity still belongs to this session.
    pub fn apply_event(&mut self, event: SearchEvent) {
        match event {
            SearchEvent::InvalidQuery {
                generation,
                message,
            } if generation == self.state.generation => {
                self.state.query_error = Some(message);
                self.state.scanning = false;
            }
            SearchEvent::ContentStats {
                generation,
                skipped,
                result_limit,
            } if generation == self.state.generation => {
                self.state.skipped = skipped;
                self.state.result_limit = result_limit;
            }
            SearchEvent::ScanError {
                generation,
                message,
            } if generation == self.state.generation => {
                self.state.scan_error = Some(message);
            }
            SearchEvent::InvalidFilters {
                generation,
                include,
                exclude,
            } if generation == self.state.generation => {
                self.state.scanning = false;
                self.state.include_error = include;
                self.state.exclude_error = exclude;
            }
            SearchEvent::Results {
                generation,
                rows,
                visited,
                matched,
                errors,
                entry_limit,
                done,
            } if generation == self.state.generation => {
                let selected = self
                    .state
                    .selected()
                    .map(|row| (row.path.clone(), row.line));
                self.state.results = rows;
                let target = self
                    .restore
                    .as_ref()
                    .map(|target| (target.path.clone(), target.line))
                    .or(selected.clone());
                self.state.selected_index = target
                    .and_then(|(path, line)| {
                        self.state
                            .results
                            .iter()
                            .position(|row| row.path == path && row.line == line)
                    })
                    .unwrap_or(0);
                self.state.visited = visited;
                self.state.matched = matched;
                self.state.errors = errors;
                self.state.entry_limit = entry_limit;
                if self.state.settings.mode != SearchMode::Contents {
                    self.state.result_limit = matched > self.limits.results;
                }
                self.state.scanning = !done;
                if selected
                    != self
                        .state
                        .selected()
                        .map(|row| (row.path.clone(), row.line))
                {
                    let restored = self
                        .restore
                        .as_ref()
                        .filter(|target| {
                            self.state.selected().is_some_and(|row| {
                                row.path == target.path && row.line == target.line
                            })
                        })
                        .map(|target| (target.top, target.following_selection));
                    let (top, following) =
                        restored.unwrap_or_else(|| (self.centered_preview_top(), true));
                    self.state.preview.top = top;
                    self.preview_follows_selection = following;
                    self.request_preview();
                }
                if done
                    && let Some(target) = self.restore.take()
                    && !self
                        .state
                        .results
                        .iter()
                        .any(|row| row.path == target.path && row.line == target.line)
                {
                    self.state.message =
                        "Previous selection unavailable; selected a remaining result".into();
                }
            }
            SearchEvent::Scope { id, result } if id == self.scope_id => {
                self.state.scope_loading = false;
                match result {
                    Ok(scope) => {
                        let preset = if scope == self.state.project_root {
                            ScopePreset::Project
                        } else if scope == self.state.launch_cwd {
                            ScopePreset::Cwd
                        } else {
                            ScopePreset::Custom
                        };
                        self.change_scope(scope, preset, true);
                    }
                    Err(error) => self.state.message = format!("Cannot apply scope: {error}"),
                }
            }
            SearchEvent::Preview { id, preview } if id == self.preview_id => {
                self.state.preview = preview;
                // A byte-limited window may contain fewer source lines than
                // the viewport. Keep a new match visible even in that case.
                if self.preview_follows_selection
                    && self.state.settings.mode == SearchMode::Contents
                    && let Some(row) = self.state.selected()
                    && row.line <= self.state.preview.total_lines
                    && !self.state.preview.lines.is_empty()
                    && !self
                        .state
                        .preview
                        .lines
                        .iter()
                        .take(self.preview_rows)
                        .any(|line| line.number == row.line)
                {
                    self.state.preview.top = row.line.saturating_sub(1);
                    self.request_preview();
                }
            }
            _ => {}
        }
    }
    fn cancel_scope(&mut self) {
        self.scope_id += 1;
        self.scope_worker.cancel(self.scope_id);
        self.state.scope_loading = false;
    }
    fn change_scope(&mut self, scope: PathBuf, preset: ScopePreset, clear_query: bool) {
        self.cancel_scope();
        // History stores user state, not retained result batches.
        self.history.push(HistoryEntry {
            settings: self.state.settings.clone(),
            preset: self.state.preset,
            target: self.state.selected().map(|row| SelectionTarget {
                path: row.path.clone(),
                line: row.line,
                top: self.state.preview.top,
                following_selection: self.preview_follows_selection,
            }),
        });
        self.state.settings.scope = scope;
        self.state.preset = preset;
        if clear_query {
            self.state.settings.query.clear();
        }
        self.state.message = if clear_query {
            "Scope changed · query cleared · Alt-Left goes Back"
        } else {
            "Scope changed · query kept · Alt-Left goes Back"
        }
        .into();
        self.refresh();
    }
    fn select(&mut self, index: usize) {
        self.restore = None;
        let index = index.min(self.state.results.len().saturating_sub(1));
        if index != self.state.selected_index {
            self.state.selected_index = index;
            self.preview_follows_selection = true;
            self.state.preview.top = self.centered_preview_top();
            self.request_preview();
        }
    }
    fn refresh(&mut self) {
        self.action = None;
        self.restore = None;
        self.preview_follows_selection = true;
        self.state.generation += 1;
        self.state.scanning = true;
        self.state.results.clear();
        self.state.selected_index = 0;
        self.state.visited = 0;
        self.state.matched = 0;
        self.state.errors = 0;
        self.state.skipped = 0;
        self.state.query_error = None;
        self.state.scan_error = None;
        self.state.entry_limit = false;
        self.state.result_limit = false;
        self.state.include_error = None;
        self.state.exclude_error = None;
        self.preview_id += 1;
        self.preview.cancel(self.preview_id);
        self.state.preview = Preview::default();
        self.search.submit(
            self.state.generation,
            SearchRequest {
                settings: self.state.settings.clone(),
                limits: self.limits,
            },
        );
    }
    fn centered_preview_top(&self) -> usize {
        self.state.selected().map_or(0, |row| {
            row.line
                .saturating_sub(1)
                .saturating_sub(self.preview_rows / 2)
        })
    }
    fn request_preview(&mut self) {
        self.preview_id += 1;
        if let Some(row) = self.state.selected() {
            self.preview.submit(
                self.preview_id,
                PreviewRequest {
                    path: row.path.clone(),
                    top: self.state.preview.top,
                    directory_policy: (self.state.settings.mode == SearchMode::Directories)
                        .then(|| {
                            VisibilityPolicy::compile(
                                self.state.settings.scope.clone(),
                                &self.state.settings.include,
                                &self.state.settings.exclude,
                                self.state.settings.hidden,
                                self.state.settings.ignored,
                            )
                            .ok()
                        })
                        .flatten(),
                    content_pattern: (self.state.settings.mode == SearchMode::Contents)
                        .then(|| {
                            search::content_pattern(
                                &self.state.settings.query,
                                self.state.settings.regex,
                            )
                            .ok()
                        })
                        .flatten(),
                },
            );
            self.state.preview.loading = true;
        } else {
            self.preview.cancel(self.preview_id);
            self.state.preview = Preview::default();
        }
    }
}

fn project_root(program: &OsString, cwd: &Path) -> Option<PathBuf> {
    let output = std::process::Command::new(program)
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut bytes = output.stdout;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    #[cfg(unix)]
    let path = {
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(OsString::from_vec(bytes))
    };
    #[cfg(not(unix))]
    let path = PathBuf::from(String::from_utf8(bytes).ok()?);
    path.canonicalize().ok().filter(|path| path.is_dir())
}
