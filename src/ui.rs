//! Workbench rendering and keyboard editing. Filesystem work belongs to the session.
use crate::{
    preview::display_text,
    session::{Command, PreviewSpan, ScopePreset, SearchMode, SearchSession},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Query,
    Include,
    Exclude,
    Scope,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,
    Insert,
}
#[derive(Default)]
struct Field {
    text: String,
    cursor: usize,
}
impl Field {
    fn set(&mut self, text: String) {
        self.cursor = text.len();
        self.text = text;
    }
    fn edit(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.text.clear();
                self.cursor = 0;
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.text.insert(self.cursor, character);
                self.cursor += character.len_utf8();
            }
            KeyCode::Backspace => {
                if let Some((previous, _)) = self.text[..self.cursor].char_indices().next_back() {
                    self.text.drain(previous..self.cursor);
                    self.cursor = previous;
                }
            }
            KeyCode::Delete => {
                if let Some(character) = self.text[self.cursor..].chars().next() {
                    self.text
                        .drain(self.cursor..self.cursor + character.len_utf8());
                }
            }
            KeyCode::Left => {
                self.cursor = self.text[..self.cursor]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(index, _)| index);
                return false;
            }
            KeyCode::Right => {
                self.cursor += self.text[self.cursor..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8);
                return false;
            }
            KeyCode::Home => {
                self.cursor = 0;
                return false;
            }
            KeyCode::End => {
                self.cursor = self.text.len();
                return false;
            }
            _ => return false,
        }
        true
    }
    fn visible(&self, width: usize) -> (String, usize) {
        let before = display_text(&self.text[..self.cursor]);
        let mut skip = before.width().saturating_sub(width.saturating_sub(1));
        let mut visible = String::new();
        let mut removed = 0;
        let displayed = Span::raw(display_text(&self.text));
        for grapheme in displayed.styled_graphemes(Style::default()) {
            let cells = grapheme.symbol.width();
            if skip > 0 {
                skip = skip.saturating_sub(cells);
                removed += cells;
            } else {
                visible.push_str(grapheme.symbol);
            }
        }
        (
            visible,
            before
                .width()
                .saturating_sub(removed)
                .min(width.saturating_sub(1)),
        )
    }
}
struct Theme {
    no_color: bool,
    base: Style,
    dim: Style,
    accent: Style,
    selected: Style,
    error: Style,
    files: Style,
    directories: Style,
    contents: Style,
    surface: Style,
}
impl Theme {
    fn new() -> Self {
        if std::env::var_os("NO_COLOR").is_some() {
            Self {
                no_color: true,
                base: Style::default(),
                dim: Style::default(),
                accent: Style::default().add_modifier(Modifier::BOLD),
                selected: Style::default().add_modifier(Modifier::REVERSED),
                error: Style::default().add_modifier(Modifier::BOLD),
                files: Style::default().add_modifier(Modifier::BOLD),
                directories: Style::default().add_modifier(Modifier::BOLD),
                contents: Style::default().add_modifier(Modifier::BOLD),
                surface: Style::default(),
            }
        } else {
            let base = Style::default()
                .bg(Color::Rgb(30, 30, 46))
                .fg(Color::Rgb(205, 214, 244));
            Self {
                no_color: false,
                base,
                dim: base.fg(Color::Rgb(166, 173, 200)),
                accent: base
                    .fg(Color::Rgb(180, 190, 254))
                    .add_modifier(Modifier::BOLD),
                selected: base
                    .bg(Color::Rgb(49, 50, 68))
                    .fg(Color::Rgb(180, 190, 254)),
                error: base.fg(Color::Rgb(243, 139, 168)),
                files: base.fg(Color::Rgb(166, 227, 161)),
                directories: base.fg(Color::Rgb(137, 180, 250)),
                contents: base.fg(Color::Rgb(250, 179, 135)),
                surface: base.bg(Color::Rgb(36, 38, 58)),
            }
        }
    }
    fn mode(&self, mode: SearchMode) -> Style {
        match mode {
            SearchMode::Files => self.files,
            SearchMode::Directories => self.directories,
            SearchMode::Contents => self.contents,
        }
    }
    fn badge(&self, style: Style) -> Style {
        style.add_modifier(Modifier::BOLD | Modifier::REVERSED)
    }
}

#[derive(Clone, Copy)]
enum Shortcut {
    Files,
    Directories,
    Contents,
    Regex,
    Scope,
    Hidden,
    Ignored,
    Pane,
    Help,
    Quit,
    Query,
}
impl Shortcut {
    const ALL: [(char, Self); 11] = [
        ('f', Self::Files),
        ('d', Self::Directories),
        ('c', Self::Contents),
        ('r', Self::Regex),
        ('h', Self::Hidden),
        ('i', Self::Ignored),
        ('s', Self::Scope),
        ('p', Self::Pane),
        ('?', Self::Help),
        ('/', Self::Query),
        ('q', Self::Quit),
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Directories => "Directories",
            Self::Contents => "Contents",
            Self::Regex => "Content regex",
            Self::Scope => "Project / CWD",
            Self::Hidden => "Hidden paths",
            Self::Ignored => "Ignored paths",
            Self::Pane => "Results / Preview",
            Self::Help => "Help & full feedback",
            Self::Quit => "Quit",
            Self::Query => "Edit Query",
        }
    }
}

pub struct Workbench {
    focus: Focus,
    input_mode: InputMode,
    query: Field,
    include: Field,
    exclude: Field,
    scope: Field,
    help: bool,
    leader: bool,
    help_scroll: usize,
    help_max_scroll: usize,
    preview_pane: bool,
    theme: Theme,
    observed_scope: std::path::PathBuf,
    applying_scope: Option<u64>,
}
impl Workbench {
    pub fn new(session: &SearchSession) -> Self {
        let mut ui = Self {
            focus: Focus::Query,
            input_mode: InputMode::Insert,
            query: Field::default(),
            include: Field::default(),
            exclude: Field::default(),
            scope: Field::default(),
            help: false,
            leader: false,
            help_scroll: 0,
            help_max_scroll: 0,
            preview_pane: false,
            theme: Theme::new(),
            observed_scope: session.state().settings.scope.clone(),
            applying_scope: None,
        };
        ui.sync(session);
        ui
    }
    fn sync(&mut self, session: &SearchSession) {
        self.query.set(session.state().settings.query.clone());
        self.include.set(session.state().settings.include.clone());
        self.exclude.set(session.state().settings.exclude.clone());
        self.scope.set(
            session
                .state()
                .settings
                .scope
                .to_string_lossy()
                .into_owned(),
        );
    }
    pub fn observe(&mut self, session: &SearchSession) {
        if let Some(generation) = self.applying_scope
            && !session.state().scope_loading
        {
            if session.state().generation > generation {
                self.sync(session);
                self.focus = Focus::Query;
            }
            self.applying_scope = None;
        }
        if self.observed_scope != session.state().settings.scope {
            self.sync(session);
            self.observed_scope = session.state().settings.scope.clone();
        } else {
            if self.query.text != session.state().settings.query {
                self.query.set(session.state().settings.query.clone());
            }
            if self.include.text != session.state().settings.include {
                self.include.set(session.state().settings.include.clone());
            }
            if self.exclude.text != session.state().settings.exclude {
                self.exclude.set(session.state().settings.exclude.clone());
            }
        }
    }
    pub fn resize(&self, session: &mut SearchSession, height: u16) {
        // Header/settings: 7 rows; footer: 3; preview border/status: 3.
        session.command(Command::SetPreviewRows(usize::from(
            height.saturating_sub(13),
        )));
    }
    /// Return true only when the user requests exit.
    pub fn key(&mut self, session: &mut SearchSession, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return true;
        }
        if self.help {
            match key.code {
                KeyCode::Esc => self.help = false,
                KeyCode::PageDown | KeyCode::Down => {
                    self.help_scroll = (self.help_scroll + 10).min(self.help_max_scroll)
                }
                KeyCode::PageUp | KeyCode::Up => {
                    self.help_scroll = self.help_scroll.saturating_sub(10)
                }
                KeyCode::Home => self.help_scroll = 0,
                KeyCode::End => self.help_scroll = self.help_max_scroll,
                _ => {}
            }
            return false;
        }
        if self.leader {
            if key.code == KeyCode::Esc {
                self.leader = false;
            } else if let KeyCode::Char(character) = key.code
                && !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                && let Some((_, shortcut)) = Shortcut::ALL.iter().find(|(key, _)| *key == character)
            {
                self.leader = false;
                match shortcut {
                    Shortcut::Files => session.command(Command::SetMode(SearchMode::Files)),
                    Shortcut::Directories => {
                        session.command(Command::SetMode(SearchMode::Directories))
                    }
                    Shortcut::Contents => session.command(Command::SetMode(SearchMode::Contents)),
                    Shortcut::Regex => session.command(Command::ToggleRegex),
                    Shortcut::Scope => {
                        session.command(Command::ToggleScope);
                        self.sync(session);
                    }
                    Shortcut::Hidden => session.command(Command::ToggleHidden),
                    Shortcut::Ignored => session.command(Command::ToggleIgnored),
                    Shortcut::Pane => self.preview_pane = !self.preview_pane,
                    Shortcut::Help => {
                        self.help = true;
                        self.help_scroll = 0;
                    }
                    Shortcut::Quit => return true,
                    Shortcut::Query => {
                        self.focus = Focus::Query;
                        self.input_mode = InputMode::Insert;
                    }
                }
            }
            return false;
        }
        match key.code {
            KeyCode::Esc => self.input_mode = InputMode::Normal,
            KeyCode::Char(' ')
                if self.input_mode == InputMode::Normal
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.leader = true;
            }
            KeyCode::Char(character)
                if self.input_mode == InputMode::Normal
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                match character {
                    'i' => self.input_mode = InputMode::Insert,
                    '/' => {
                        self.focus = Focus::Query;
                        self.input_mode = InputMode::Insert;
                    }
                    'j' => session.command(Command::MoveSelection(1)),
                    'k' => session.command(Command::MoveSelection(-1)),
                    'q' => return true,
                    _ => {}
                }
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
                session.command(Command::Back);
                self.sync(session);
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let reverse =
                    key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT);
                self.focus = if reverse {
                    match self.focus {
                        Focus::Query => Focus::Scope,
                        Focus::Include => Focus::Query,
                        Focus::Exclude => Focus::Include,
                        Focus::Scope => Focus::Exclude,
                    }
                } else {
                    match self.focus {
                        Focus::Query => Focus::Include,
                        Focus::Include => Focus::Exclude,
                        Focus::Exclude => Focus::Scope,
                        Focus::Scope => Focus::Query,
                    }
                }
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.focus = Focus::Query;
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.focus = Focus::Scope;
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                session.command(Command::CopyPath)
            }
            KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                session.command(Command::ChangeDirectory)
            }
            KeyCode::Up => session.command(Command::MoveSelection(-1)),
            KeyCode::Down => session.command(Command::MoveSelection(1)),
            KeyCode::PageUp => session.command(Command::PagePreview(-1)),
            KeyCode::PageDown => session.command(Command::PagePreview(1)),
            KeyCode::Enter if self.focus == Focus::Scope => {
                self.applying_scope = Some(session.state().generation);
                session.command(Command::ApplyScope(self.scope.text.clone().into()));
            }
            KeyCode::Enter => session.command(Command::OpenSelected),
            _ if self.input_mode == InputMode::Insert => self.edit_field(session, key),
            _ => {}
        }
        false
    }
    fn edit_field(&mut self, session: &mut SearchSession, key: KeyEvent) {
        let field = match self.focus {
            Focus::Query => &mut self.query,
            Focus::Include => &mut self.include,
            Focus::Exclude => &mut self.exclude,
            Focus::Scope => &mut self.scope,
        };
        if field.edit(key) {
            match self.focus {
                Focus::Query => session.command(Command::SetQuery(self.query.text.clone())),
                Focus::Include => session.command(Command::SetInclude(self.include.text.clone())),
                Focus::Exclude => session.command(Command::SetExclude(self.exclude.text.clone())),
                Focus::Scope => {}
            }
        }
    }
    pub fn draw(&mut self, frame: &mut Frame, session: &SearchSession) {
        let area = frame.area();
        let state = session.state();
        frame.render_widget(Block::default().style(self.theme.base), area);
        if area.width < 70 || area.height < 18 {
            frame.render_widget(
                Paragraph::new(if self.help {
                    "Keyboard help · Esc close · Ctrl-C quit\nNORMAL: i Insert · / edit Query · Space shortcuts\nINSERT: type normally, including spaces · Esc Normal\nNormal j/k or Up/Down select · q / Ctrl-C quit\nSpace f Files · Space d Directories · Space c Contents\nSpace r Regex · Space s Project/CWD · Space p Pane\nSpace h Hidden · Space i Ignored · Space ? Help\nTab / Shift-Tab fields · Ctrl-F Query · Ctrl-L Scope\nCtrl-U clear · arrows/Home/End edit in Insert\nEnter browse/editor · Alt-Left Back · Ctrl-G cd & exit\nCtrl-Y copy · PageUp/PageDown preview"
                } else if self.input_mode == InputMode::Insert {
                    "dirpick · Resize terminal to at least 70×18\nINSERT · Type normally · Esc Normal · Ctrl-C quit"
                } else {
                    "dirpick · Resize terminal to at least 70×18\nNORMAL · i Edit · / Query · Space Shortcuts\nq / Ctrl-C quit · Space ? Help"
                })
                .wrap(Wrap { trim: false })
                .style(self.theme.accent),
                area,
            );
            if self.leader {
                self.guide(frame, area, session);
            }
            return;
        }
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(6),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);
        let header =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(17)]).split(vertical[0]);
        let mut modes = vec![Span::styled(" dirpick │ ", self.theme.accent)];
        for (mode, label) in [
            (SearchMode::Files, "f Files"),
            (SearchMode::Directories, "d Directories"),
            (SearchMode::Contents, "c Contents"),
        ] {
            let style = self.theme.mode(mode);
            modes.push(if state.settings.mode == mode {
                Span::styled(format!("[{label}]"), self.theme.badge(style))
            } else {
                Span::styled(format!(" {label} "), style)
            });
            modes.push(Span::raw(" "));
        }
        frame.render_widget(Paragraph::new(Line::from(modes)), header[0]);
        frame.render_widget(
            Paragraph::new(if self.input_mode == InputMode::Normal {
                " Space Shortcuts "
            } else {
                " Esc Normal mode "
            })
            .style(self.theme.badge(self.theme.accent)),
            header[1],
        );
        let controls = Layout::default()
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .direction(Direction::Horizontal)
            .split(vertical[1]);
        let search_inner = controls[0].inner(ratatui::layout::Margin::new(1, 1));
        frame.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Search · regex filters ")
                .border_style(if self.focus == Focus::Scope {
                    self.theme.dim
                } else {
                    self.theme.mode(state.settings.mode)
                }),
            controls[0],
        );
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    match state.settings.mode {
                        SearchMode::Contents if state.settings.regex => "Regex:on · smart case",
                        SearchMode::Contents => "Regex:off · literal · smart case",
                        _ if state.settings.regex => "fuzzy · Regex:on (Contents)",
                        _ => "fuzzy · Regex:off (Contents)",
                    },
                    self.theme.mode(state.settings.mode),
                )),
                Line::raw(""),
            ])
            .style(self.theme.dim),
            search_inner,
        );
        let query_area = Rect::new(search_inner.x, search_inner.y + 1, search_inner.width, 1);
        self.field(
            frame,
            query_area,
            "Query",
            &self.query,
            self.focus == Focus::Query,
            state.query_error.is_some(),
        );
        self.field(
            frame,
            Rect::new(search_inner.x, search_inner.y + 2, search_inner.width, 1),
            "Include",
            &self.include,
            self.focus == Focus::Include,
            state.include_error.is_some(),
        );
        self.field(
            frame,
            Rect::new(search_inner.x, search_inner.y + 3, search_inner.width, 1),
            "Exclude",
            &self.exclude,
            self.focus == Focus::Exclude,
            state.exclude_error.is_some(),
        );
        frame.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Search scope ")
                .border_style(if self.focus == Focus::Scope {
                    self.theme.directories
                } else {
                    self.theme.dim
                }),
            controls[1],
        );
        let scope_inner = controls[1].inner(ratatui::layout::Margin::new(1, 1));
        self.field(
            frame,
            Rect::new(scope_inner.x, scope_inner.y, scope_inner.width, 1),
            "Scope",
            &self.scope,
            self.focus == Focus::Scope,
            false,
        );
        let preset = match state.preset {
            ScopePreset::Project => "[Project]  CWD",
            ScopePreset::Cwd => "Project  [CWD]",
            ScopePreset::Custom => "Project  CWD  [Custom]",
        };
        let fallback = if state.project_fallback {
            " · CWD fallback"
        } else {
            ""
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    format!("s {preset}{fallback}"),
                    self.theme.directories,
                )),
                Line::from(vec![
                    Span::styled(
                        format!(
                            "h Hidden:{}",
                            if state.settings.hidden { "on" } else { "off" }
                        ),
                        if state.settings.hidden {
                            self.theme.files
                        } else {
                            self.theme.dim
                        },
                    ),
                    Span::styled(
                        format!(
                            "  i Ignored:{}",
                            if state.settings.ignored { "on" } else { "off" }
                        ),
                        if state.settings.ignored {
                            self.theme.contents
                        } else {
                            self.theme.dim
                        },
                    ),
                ]),
                Line::raw(format!(
                    "Launch CWD: {}",
                    display_text(&state.launch_cwd.to_string_lossy())
                )),
            ])
            .style(self.theme.dim),
            Rect::new(scope_inner.x, scope_inner.y + 1, scope_inner.width, 3),
        );
        if area.width >= 100 {
            let panes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
                .split(vertical[2]);
            self.results(frame, panes[0], session);
            self.preview(frame, panes[1], session);
        } else if self.preview_pane {
            self.preview(frame, vertical[2], session);
        } else {
            self.results(frame, vertical[2], session);
        }
        let invalid_filters = state.query_error.is_some()
            || state.include_error.is_some()
            || state.exclude_error.is_some();
        let details_hint = if self.input_mode == InputMode::Insert {
            "Esc Space ?"
        } else {
            "Space ?"
        };
        let status = if state.query_error.is_some() {
            format!("Invalid content regex · {details_hint} details")
        } else if state.include_error.is_some() {
            format!("Invalid include regex · {details_hint} details")
        } else if state.exclude_error.is_some() {
            format!("Invalid exclude regex · {details_hint} details")
        } else if state.scope_loading {
            "Validating scope… current search retained".into()
        } else if state.scanning {
            format!(
                "Scanning… {} visited · {} retained",
                state.visited,
                state.results.len()
            )
        } else {
            format!(
                "{}{}{} {} · {} visited · {} errors · {} skipped",
                if state.entry_limit {
                    "PARTIAL: entry limit · "
                } else {
                    ""
                },
                if state.result_limit {
                    "PARTIAL: result limit · "
                } else {
                    ""
                },
                state.results.len(),
                match state.settings.mode {
                    SearchMode::Files => "files",
                    SearchMode::Directories => "directories",
                    SearchMode::Contents => "matching lines",
                },
                state.visited,
                state.errors,
                state.skipped,
            )
        };
        let detail = if !state.message.is_empty() {
            state.message.as_str()
        } else if let Some(error) = state
            .query_error
            .as_ref()
            .or(state.include_error.as_ref())
            .or(state.exclude_error.as_ref())
            .or(state.scan_error.as_ref())
        {
            error
        } else {
            "Ctrl-Y Copy · Ctrl-G cd & exit · Alt-Left Back · PgUp/Dn Preview"
        };
        let mut detail = display_text(detail);
        if detail.width() > usize::from(vertical[3].width) {
            let suffix = format!("… {details_hint} details");
            let available = usize::from(vertical[3].width).saturating_sub(suffix.width());
            let mut cells = 0;
            detail = detail
                .chars()
                .take_while(|character| {
                    cells += character.width().unwrap_or(0);
                    cells <= available
                })
                .collect::<String>();
            detail.push_str(&suffix);
        }
        // Reserve one row each for completion/limits, action feedback, and help.
        // Long errors remain inspectable in help without displacing these rows.
        let enter_label = if self.focus == Focus::Scope {
            "Apply Scope"
        } else if state.settings.mode == SearchMode::Directories {
            "Browse"
        } else {
            "Editor"
        };
        let lines = vec![
            Line::from(Span::styled(
                status.clone(),
                if invalid_filters || state.errors > 0 || state.entry_limit || state.result_limit {
                    self.theme.error
                } else {
                    self.theme.files
                },
            )),
            Line::raw(detail),
            if self.input_mode == InputMode::Normal {
                Line::from(vec![
                    Span::styled(" NORMAL ", self.theme.badge(self.theme.directories)),
                    Span::styled(" i ", self.theme.files),
                    Span::raw("Edit "),
                    Span::styled("/ ", self.theme.files),
                    Span::raw("Query "),
                    Span::styled("Space ", self.theme.accent),
                    Span::raw("Keys "),
                    Span::styled("j/k ", self.theme.directories),
                    Span::raw("Select "),
                    Span::styled("Enter ", self.theme.files),
                    Span::raw(format!("{enter_label} ")),
                    Span::styled("q ", self.theme.contents),
                    Span::raw("Quit"),
                ])
            } else {
                Line::from(vec![
                    Span::styled(" INSERT ", self.theme.badge(self.theme.files)),
                    Span::raw(" Type to edit  "),
                    Span::styled("Esc ", self.theme.directories),
                    Span::raw("Normal  "),
                    Span::styled("Tab ", self.theme.contents),
                    Span::raw("Fields  "),
                    Span::styled("Enter ", self.theme.files),
                    Span::raw(enter_label),
                ])
            },
        ];
        frame.render_widget(Paragraph::new(lines).style(self.theme.dim), vertical[3]);
        if self.help {
            let popup = Rect::new(
                area.x + 4,
                area.y + 3,
                area.width - 8,
                (area.height - 3).min(15),
            );
            frame.render_widget(Clear, popup);
            let mut feedback = vec![status];
            for message in [
                Some(&state.message),
                state.query_error.as_ref(),
                state.include_error.as_ref(),
                state.exclude_error.as_ref(),
                state.scan_error.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                if !message.is_empty() {
                    feedback.push(display_text(message));
                }
            }
            let help = format!(
                "Esc close help · Ctrl-C quit · PgUp/Dn scroll\nNORMAL: i edit focused field · / edit Query\nINSERT: type normally, including spaces · Esc Normal\nNormal j/k or Up/Down: select · q: quit\nSpace: shortcut guide in Normal\nSpace f / d / c: Files / Directories / Contents\nSpace r: toggle content Regex\nSpace s: Project / immutable launch CWD\nSpace h / i: reveal Hidden / Ignored independently\nSpace p: Results/Preview below 100 columns\nSpace /: edit Query · Space ?: help · Space q: quit\nTab / Shift-Tab: Query, Include, Exclude, Scope\nCtrl-F / Ctrl-L: edit Query / Scope in Insert\nInsert Ctrl-U: clear; Left/Right/Home/End: edit\nPageUp/PageDown preview · Ctrl-Y: exact path via OSC 52\nInclude/Exclude: relative path regex, smart case\nEnter in Scope: apply path · Alt-Left Back\nEnter browse/editor in either mode · Ctrl-G cd & exit\n\nCurrent feedback (Home/End jump):\n{}",
                feedback.join("\n")
            );
            let mut lines = Vec::new();
            let width = usize::from(popup.width.saturating_sub(2));
            for line in help.lines() {
                let (mut text, mut cells) = (String::new(), 0);
                for word in line.split_inclusive(' ') {
                    if cells > 0 && word.width() <= width && cells + word.width() > width {
                        lines.push(Line::raw(std::mem::take(&mut text)));
                        cells = 0;
                    }
                    let word = Span::raw(word);
                    for grapheme in word.styled_graphemes(Style::default()) {
                        let next = grapheme.symbol.width();
                        if cells + next > width {
                            lines.push(Line::raw(std::mem::take(&mut text)));
                            cells = 0;
                        }
                        text.push_str(grapheme.symbol);
                        cells += next;
                    }
                }
                lines.push(Line::raw(text));
            }
            self.help_max_scroll = lines
                .len()
                .saturating_sub(usize::from(popup.height.saturating_sub(2)));
            self.help_scroll = self.help_scroll.min(self.help_max_scroll);
            let help = Paragraph::new(lines);
            frame.render_widget(
                help.scroll((self.help_scroll as u16, 0))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(self.theme.accent)
                            .title(" Keyboard help · PgUp/Dn details "),
                    )
                    .style(self.theme.base),
                popup,
            );
        }
        if self.leader {
            self.guide(frame, area, session);
        }
    }
    fn guide(&self, frame: &mut Frame, area: Rect, session: &SearchSession) {
        let width = area.width.saturating_sub(2).min(78);
        let columns = if width >= 56 { 2 } else { 1 };
        let rows = Shortcut::ALL.len().div_ceil(columns);
        let height = (rows as u16 + 4).min(area.height);
        let popup = Rect::new(
            area.right().saturating_sub(width + 1).max(area.x),
            area.bottom().saturating_sub(height + 1).max(area.y),
            width,
            height,
        );
        frame.render_widget(Clear, popup);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.theme.accent)
            .style(self.theme.surface)
            .title(" Space → Shortcuts ")
            .title_bottom(" Esc cancel · / edit Query ");
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        let cells =
            Layout::horizontal(vec![Constraint::Ratio(1, columns as u32); columns]).split(inner);
        let settings = &session.state().settings;
        for (column, cell) in cells.iter().enumerate() {
            frame.render_widget(
                Paragraph::new(if columns == 1 {
                    " Press a key to choose"
                } else if column == 0 {
                    " Search & filters"
                } else {
                    " Scope & session"
                })
                .style(self.theme.accent),
                Rect {
                    height: cell.height.min(1),
                    ..*cell
                },
            );
            let lines: Vec<_> = Shortcut::ALL
                .iter()
                .skip(column * rows)
                .take(rows)
                .map(|(key, shortcut)| {
                    let (style, enabled) = match shortcut {
                        Shortcut::Files => (self.theme.files, settings.mode == SearchMode::Files),
                        Shortcut::Directories => (
                            self.theme.directories,
                            settings.mode == SearchMode::Directories,
                        ),
                        Shortcut::Contents => {
                            (self.theme.contents, settings.mode == SearchMode::Contents)
                        }
                        Shortcut::Regex => (self.theme.contents, settings.regex),
                        Shortcut::Hidden => (self.theme.files, settings.hidden),
                        Shortcut::Ignored => (self.theme.contents, settings.ignored),
                        _ => (self.theme.accent, false),
                    };
                    let key = format!("  {key}  ");
                    let state = match shortcut {
                        Shortcut::Files | Shortcut::Directories | Shortcut::Contents if enabled => {
                            " [active]"
                        }
                        Shortcut::Regex | Shortcut::Hidden | Shortcut::Ignored => {
                            if enabled {
                                " [on]"
                            } else {
                                " [off]"
                            }
                        }
                        _ => "",
                    };
                    Line::from(vec![
                        Span::styled(key, self.theme.badge(style)),
                        Span::styled(format!(" {}{state}", shortcut.label()), style),
                    ])
                })
                .collect();
            frame.render_widget(
                Paragraph::new(lines).style(self.theme.surface),
                Rect {
                    y: cell.y.saturating_add(2),
                    height: cell.height.saturating_sub(2),
                    ..*cell
                },
            );
        }
    }
    fn field(
        &self,
        frame: &mut Frame,
        area: Rect,
        label: &str,
        field: &Field,
        focused: bool,
        invalid: bool,
    ) {
        let prefix = format!(
            "{} {label} ",
            if invalid {
                "!"
            } else if focused {
                ">"
            } else {
                " "
            }
        );
        let available = usize::from(area.width).saturating_sub(prefix.width());
        let (text, cursor) = field.visible(available);
        frame.render_widget(
            Paragraph::new(format!("{prefix}{text}")).style(if invalid {
                self.theme.error
            } else if focused {
                self.theme.selected
            } else {
                self.theme.base
            }),
            area,
        );
        if focused
            && self.input_mode == InputMode::Insert
            && !self.help
            && !self.leader
            && available > 0
        {
            frame.set_cursor_position((area.x + prefix.width() as u16 + cursor as u16, area.y));
        }
    }
    fn results(&self, frame: &mut Frame, area: Rect, session: &SearchSession) {
        let state = session.state();
        let window = usize::from(area.height.saturating_sub(2)).max(1);
        let first = state
            .selected_index
            .saturating_sub(window / 2)
            .min(state.results.len().saturating_sub(window));
        let items: Vec<_> = state
            .results
            .iter()
            .enumerate()
            .skip(first)
            .take(window)
            .map(|(index, row)| {
                if state.settings.mode == SearchMode::Contents {
                    let mut lines = Vec::new();
                    if index == first || state.results[index - 1].path != row.path {
                        lines.push(Line::styled(display_text(&row.relative), self.theme.accent));
                    }
                    let mut spans =
                        vec![Span::styled(format!("  {:>5}  ", row.line), self.theme.dim)];
                    spans.extend(self.source_spans(
                        &row.text,
                        &[],
                        &row.matches,
                        usize::from(area.width).saturating_sub(11),
                    ));
                    lines.push(Line::from(spans));
                    ListItem::new(lines)
                } else {
                    ListItem::new(display_text(&row.relative))
                }
            })
            .collect();
        let mut selection = ListState::default()
            .with_selected(state.selected().map(|_| state.selected_index - first));
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(format!(
                        " Results · {} {} ",
                        state.results.len(),
                        if state.scanning { "· scanning" } else { "" }
                    ))
                    .border_style(self.theme.mode(state.settings.mode)),
            )
            .style(self.theme.base)
            .highlight_style(self.theme.selected)
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut selection);
        if state.results.is_empty() {
            frame.render_widget(
                Paragraph::new(if state.scanning {
                    "Scanning…"
                } else if state.query_error.is_some()
                    || state.include_error.is_some()
                    || state.exclude_error.is_some()
                {
                    "Invalid patterns · edit marked fields"
                } else if state.settings.mode == SearchMode::Contents
                    && state.settings.query.is_empty()
                {
                    "Type a content query to search lines"
                } else if state.settings.mode == SearchMode::Contents {
                    "No matching lines"
                } else {
                    match state.settings.mode {
                        SearchMode::Directories => "No matching directories",
                        _ => "No matching files",
                    }
                })
                .style(self.theme.dim),
                area.inner(ratatui::layout::Margin::new(2, 2)),
            );
        }
    }
    fn preview(&self, frame: &mut Frame, area: Rect, session: &SearchSession) {
        let state = session.state();
        let title = state.selected().map_or_else(
            || " Preview ".into(),
            |row| format!(" {} ", display_text(&row.path.to_string_lossy())),
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title)
            .border_style(self.theme.directories);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let lines = if state.preview.loading {
            vec![Line::raw("Loading preview…")]
        } else if !state.preview.status.is_empty() && state.preview.lines.is_empty() {
            vec![Line::raw(&state.preview.status)]
        } else {
            state
                .preview
                .lines
                .iter()
                .take(usize::from(inner.height.saturating_sub(1)))
                .map(|line| {
                    let mut spans = vec![Span::styled(
                        format!("{:>5}  ", line.number),
                        self.theme.dim,
                    )];
                    spans.extend(self.source_spans(
                        &line.text,
                        &line.spans,
                        &line.matches,
                        usize::from(inner.width).saturating_sub(7),
                    ));
                    Line::from(spans)
                })
                .collect()
        };
        let source_area = Rect {
            height: inner.height.saturating_sub(1),
            ..inner
        };
        frame.render_widget(Paragraph::new(lines).style(self.theme.base), source_area);
        if inner.height > 0 {
            let label = if state.preview.loading || state.preview.lines.is_empty() {
                String::new()
            } else {
                format!(
                    "{} {}–{}/{} · PgUp/Dn{}{}",
                    if state.settings.mode == SearchMode::Directories {
                        "Children"
                    } else {
                        "Lines"
                    },
                    state.preview.top + 1,
                    state.preview.top
                        + state
                            .preview
                            .lines
                            .len()
                            .min(usize::from(source_area.height)),
                    state.preview.total_lines,
                    if state.preview.status.is_empty() {
                        ""
                    } else {
                        " · "
                    },
                    state.preview.status
                )
            };
            frame.render_widget(
                Paragraph::new(label).style(self.theme.dim),
                Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
            );
        }
    }
    fn source_spans(
        &self,
        text: &str,
        syntax: &[PreviewSpan],
        matches: &[std::ops::Range<usize>],
        width: usize,
    ) -> Vec<Span<'static>> {
        // Both range lists are ordered raw UTF-8 offsets. Walk only the visible
        // prefix, advancing each range cursor once instead of testing every
        // match for every grapheme. Keep each displayed cluster together even
        // when a syntax or match boundary falls inside it. The source offsets
        // remain raw UTF-8 bytes; escaping happens only after segmentation.
        // Bound runs of zero-width clusters too.
        let (mut syntax_index, mut match_index, mut cells) = (0, 0, 0);
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (offset, grapheme) in text
            .grapheme_indices(true)
            .take(width.saturating_mul(4).saturating_add(32))
        {
            if cells >= width {
                break;
            }
            let displayed = display_text(grapheme);
            let grapheme_width = displayed.width();
            if cells + grapheme_width > width {
                break;
            }
            cells += grapheme_width;
            while syntax_index < syntax.len() && syntax[syntax_index].range.end <= offset {
                syntax_index += 1;
            }
            while match_index < matches.len() && matches[match_index].end <= offset {
                match_index += 1;
            }
            let mut style = self.theme.base;
            if !self.theme.no_color
                && let Some(syntax) = syntax
                    .get(syntax_index)
                    .filter(|span| span.range.contains(&offset))
            {
                style = style.fg(Color::Rgb(
                    syntax.foreground[0],
                    syntax.foreground[1],
                    syntax.foreground[2],
                ));
                if syntax.bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if syntax.italic {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                if syntax.underline {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }
            }
            if matches
                .get(match_index)
                .is_some_and(|range| range.start < offset + grapheme.len() && range.end > offset)
            {
                style = if self.theme.no_color {
                    style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    style
                        .bg(Color::Rgb(249, 226, 175))
                        .fg(Color::Rgb(30, 30, 46))
                };
            }
            if let Some(last) = spans.last_mut().filter(|span| span.style == style) {
                last.content.to_mut().push_str(&displayed);
            } else {
                spans.push(Span::styled(displayed, style));
            }
        }
        spans
    }
}
