//! Application boundary: commands in, observable state and generation events out.
use crate::{
    preview::{self, PreviewRequest},
    search::{self, SearchRequest},
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
pub enum ScopePreset {
    Project,
    Cwd,
    Custom,
}
#[derive(Clone, Debug)]
pub struct Options {
    pub git_program: Option<OsString>,
    pub limits: Limits,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            git_program: Some("git".into()),
            limits: Limits::default(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult {
    pub path: PathBuf,
    pub relative: String,
    pub line: usize,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreviewLine {
    pub number: usize,
    pub text: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preview {
    pub lines: Vec<PreviewLine>,
    pub top: usize,
    pub total_lines: usize,
    pub loading: bool,
    pub status: String,
}
#[derive(Clone, Debug)]
pub struct State {
    pub scope: PathBuf,
    pub launch_cwd: PathBuf,
    pub project_root: PathBuf,
    pub project_fallback: bool,
    pub preset: ScopePreset,
    pub query: String,
    pub include: String,
    pub exclude: String,
    pub include_error: Option<String>,
    pub exclude_error: Option<String>,
    pub hidden: bool,
    pub ignored: bool,
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
    SetQuery(String),
    SetInclude(String),
    SetExclude(String),
    ToggleHidden,
    ToggleIgnored,
    Select(usize),
    MoveSelection(isize),
    ScrollPreview(isize),
    ToggleScope,
    ApplyScope(PathBuf),
    Back,
}
#[derive(Clone, Debug)]
pub enum SearchEvent {
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
    scope: PathBuf,
    preset: ScopePreset,
    query: String,
    include: String,
    exclude: String,
    hidden: bool,
    ignored: bool,
    target: Option<(PathBuf, usize)>,
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
    restore: Option<(PathBuf, usize)>,
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
                project_root: scope.clone(),
                scope,
                launch_cwd,
                project_fallback,
                preset: ScopePreset::Project,
                query: String::new(),
                include: String::new(),
                exclude: String::new(),
                include_error: None,
                exclude_error: None,
                hidden: false,
                ignored: false,
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
        };
        session.refresh();
        Ok(session)
    }
    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn command(&mut self, command: Command) {
        match command {
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
                    self.state.scope.join(path)
                };
                self.scope_id += 1;
                self.state.scope_loading = true;
                self.scope_worker.submit(self.scope_id, path);
            }
            Command::Back => {
                self.cancel_scope();
                if let Some(previous) = self.history.pop() {
                    let target = previous.target;
                    self.state.scope = previous.scope;
                    self.state.preset = previous.preset;
                    self.state.query = previous.query;
                    self.state.include = previous.include;
                    self.state.exclude = previous.exclude;
                    self.state.hidden = previous.hidden;
                    self.state.ignored = previous.ignored;
                    self.refresh();
                    self.restore = target;
                    self.state.message = "Previous scope and search restored".into();
                } else {
                    self.state.message = "No earlier scope".into();
                }
            }
            Command::SetQuery(query) => {
                if self.state.query != query {
                    self.state.query = query;
                    self.refresh();
                }
            }
            Command::SetInclude(include) => {
                if self.state.include != include {
                    self.state.include = include;
                    self.refresh();
                }
            }
            Command::SetExclude(exclude) => {
                if self.state.exclude != exclude {
                    self.state.exclude = exclude;
                    self.refresh();
                }
            }
            Command::ToggleHidden => {
                self.state.hidden = !self.state.hidden;
                self.refresh();
            }
            Command::ToggleIgnored => {
                self.state.ignored = !self.state.ignored;
                self.refresh();
            }
            Command::Select(index) => self.select(index),
            Command::MoveSelection(delta) => {
                self.select(self.state.selected_index.saturating_add_signed(delta))
            }
            Command::ScrollPreview(delta) => {
                self.state.preview.top = self
                    .state
                    .preview
                    .top
                    .saturating_add_signed(delta)
                    .min(self.state.preview.total_lines.saturating_sub(1));
                self.request_preview();
            }
        }
    }
    pub fn poll(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            self.apply_event(event);
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
                let selected = self.state.selected().map(|row| row.path.clone());
                self.state.results = rows;
                let target = self
                    .restore
                    .as_ref()
                    .map(|(path, _)| path)
                    .or(selected.as_ref());
                self.state.selected_index = target
                    .and_then(|path| self.state.results.iter().position(|row| &row.path == path))
                    .unwrap_or(0);
                self.state.visited = visited;
                self.state.matched = matched;
                self.state.errors = errors;
                self.state.entry_limit = entry_limit;
                self.state.result_limit = matched > self.limits.results;
                self.state.scanning = !done;
                if selected != self.state.selected().map(|row| row.path.clone()) {
                    self.state.preview.top = self
                        .restore
                        .as_ref()
                        .filter(|(path, _)| {
                            self.state.selected().is_some_and(|row| &row.path == path)
                        })
                        .map_or(0, |(_, top)| *top);
                    self.request_preview();
                }
                if done
                    && let Some((path, _)) = self.restore.take()
                    && !self.state.results.iter().any(|row| row.path == path)
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
                self.state.preview = preview
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
            scope: self.state.scope.clone(),
            preset: self.state.preset,
            query: self.state.query.clone(),
            include: self.state.include.clone(),
            exclude: self.state.exclude.clone(),
            hidden: self.state.hidden,
            ignored: self.state.ignored,
            target: self
                .state
                .selected()
                .map(|row| (row.path.clone(), self.state.preview.top)),
        });
        self.state.scope = scope;
        self.state.preset = preset;
        if clear_query {
            self.state.query.clear();
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
            self.state.preview.top = 0;
            self.request_preview();
        }
    }
    fn refresh(&mut self) {
        self.restore = None;
        self.state.generation += 1;
        self.state.scanning = true;
        self.state.results.clear();
        self.state.selected_index = 0;
        self.state.visited = 0;
        self.state.matched = 0;
        self.state.errors = 0;
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
                scope: self.state.scope.clone(),
                query: self.state.query.clone(),
                include: self.state.include.clone(),
                exclude: self.state.exclude.clone(),
                hidden: self.state.hidden,
                ignored: self.state.ignored,
                limits: self.limits,
            },
        );
    }
    fn request_preview(&mut self) {
        self.preview_id += 1;
        if let Some(row) = self.state.selected() {
            self.preview.submit(
                self.preview_id,
                PreviewRequest {
                    path: row.path.clone(),
                    top: self.state.preview.top,
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
