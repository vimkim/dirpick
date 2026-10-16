//! Workbench rendering and keyboard editing. Filesystem work belongs to the session.
use crate::{
    preview::display_text,
    session::{Command, ScopePreset, SearchSession},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Query,
    Include,
    Exclude,
    Scope,
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
        for character in display_text(&self.text).chars() {
            let cells = character.width().unwrap_or(0);
            if skip > 0 {
                skip = skip.saturating_sub(cells);
                removed += cells;
            } else {
                visible.push(character);
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
    base: Style,
    dim: Style,
    accent: Style,
    selected: Style,
    error: Style,
}
impl Theme {
    fn new() -> Self {
        if std::env::var_os("NO_COLOR").is_some() {
            Self {
                base: Style::default(),
                dim: Style::default(),
                accent: Style::default().add_modifier(Modifier::BOLD),
                selected: Style::default().add_modifier(Modifier::REVERSED),
                error: Style::default().add_modifier(Modifier::BOLD),
            }
        } else {
            let base = Style::default()
                .bg(Color::Rgb(30, 30, 46))
                .fg(Color::Rgb(205, 214, 244));
            Self {
                base,
                dim: base.fg(Color::Rgb(166, 173, 200)),
                accent: base
                    .fg(Color::Rgb(180, 190, 254))
                    .add_modifier(Modifier::BOLD),
                selected: base
                    .bg(Color::Rgb(49, 50, 68))
                    .fg(Color::Rgb(180, 190, 254)),
                error: base.fg(Color::Rgb(243, 139, 168)),
            }
        }
    }
}

pub struct Workbench {
    focus: Focus,
    query: Field,
    include: Field,
    exclude: Field,
    scope: Field,
    help: bool,
    preview_pane: bool,
    theme: Theme,
    observed_scope: std::path::PathBuf,
    applying_scope: Option<u64>,
}
impl Workbench {
    pub fn new(session: &SearchSession) -> Self {
        let mut ui = Self {
            focus: Focus::Query,
            query: Field::default(),
            include: Field::default(),
            exclude: Field::default(),
            scope: Field::default(),
            help: false,
            preview_pane: false,
            theme: Theme::new(),
            observed_scope: session.state().scope.clone(),
            applying_scope: None,
        };
        ui.sync(session);
        ui
    }
    fn sync(&mut self, session: &SearchSession) {
        self.query.set(session.state().query.clone());
        self.include.set(session.state().include.clone());
        self.exclude.set(session.state().exclude.clone());
        self.scope
            .set(session.state().scope.to_string_lossy().into_owned());
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
        if self.observed_scope != session.state().scope {
            self.sync(session);
            self.observed_scope = session.state().scope.clone();
        } else {
            if self.query.text != session.state().query {
                self.query.set(session.state().query.clone());
            }
            if self.include.text != session.state().include {
                self.include.set(session.state().include.clone());
            }
            if self.exclude.text != session.state().exclude {
                self.exclude.set(session.state().exclude.clone());
            }
        }
    }
    /// Return true only when the user requests exit.
    pub fn key(&mut self, session: &mut SearchSession, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return true;
        }
        if self.help {
            if matches!(key.code, KeyCode::Esc | KeyCode::F(12)) {
                self.help = false;
            }
            return false;
        }
        match key.code {
            KeyCode::Esc => return true,
            KeyCode::F(12) => self.help = true,
            KeyCode::F(10) => self.preview_pane = !self.preview_pane,
            KeyCode::F(7) => {
                session.command(Command::ToggleScope);
                self.sync(session);
            }
            KeyCode::F(8) => session.command(Command::ToggleHidden),
            KeyCode::F(9) => session.command(Command::ToggleIgnored),
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
                self.focus = Focus::Query
            }
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.focus = Focus::Scope
            }
            KeyCode::Up => session.command(Command::MoveSelection(-1)),
            KeyCode::Down => session.command(Command::MoveSelection(1)),
            KeyCode::PageUp => session.command(Command::ScrollPreview(-10)),
            KeyCode::PageDown => session.command(Command::ScrollPreview(10)),
            KeyCode::Enter if self.focus == Focus::Scope => {
                self.applying_scope = Some(session.state().generation);
                session.command(Command::ApplyScope(self.scope.text.clone().into()));
            }
            _ => {
                let field = match self.focus {
                    Focus::Query => &mut self.query,
                    Focus::Include => &mut self.include,
                    Focus::Exclude => &mut self.exclude,
                    Focus::Scope => &mut self.scope,
                };
                if field.edit(key) {
                    match self.focus {
                        Focus::Query => session.command(Command::SetQuery(self.query.text.clone())),
                        Focus::Include => {
                            session.command(Command::SetInclude(self.include.text.clone()))
                        }
                        Focus::Exclude => {
                            session.command(Command::SetExclude(self.exclude.text.clone()))
                        }
                        Focus::Scope => {}
                    }
                }
            }
        }
        false
    }
    pub fn draw(&self, frame: &mut Frame, session: &SearchSession) {
        let area = frame.area();
        let state = session.state();
        frame.render_widget(Block::default().style(self.theme.base), area);
        if area.width < 70 || area.height < 18 {
            frame.render_widget(
                Paragraph::new("dirpick · Resize terminal to at least 70×18\nEsc / Ctrl-C quit")
                    .style(self.theme.accent),
                area,
            );
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
        frame.render_widget(
            Paragraph::new(" dirpick │ Workbench │ Files").style(self.theme.accent),
            vertical[0],
        );
        let controls = Layout::default()
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .direction(Direction::Horizontal)
            .split(vertical[1]);
        let search_inner = controls[0].inner(ratatui::layout::Margin::new(1, 1));
        frame.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .title(" Search · regex filters ")
                .border_style(self.theme.dim),
            controls[0],
        );
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "Files · fuzzy · smart case",
                    self.theme.accent,
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
            false,
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
                .title(" Search scope ")
                .border_style(self.theme.dim),
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
                    format!("F7 {preset}{fallback}"),
                    self.theme.accent,
                )),
                Line::from(Span::styled(
                    format!(
                        "F8 Hidden:{} F9 Ignored:{}",
                        if state.hidden { "on" } else { "off" },
                        if state.ignored { "on" } else { "off" }
                    ),
                    self.theme.accent,
                )),
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
        let invalid_filters = state.include_error.is_some() || state.exclude_error.is_some();
        let mut status = if let Some(error) = &state.include_error {
            format!("Invalid include regex: {error}")
        } else if let Some(error) = &state.exclude_error {
            format!("Invalid exclude regex: {error}")
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
                "{} files · {} visited · {} errors{}{}",
                state.results.len(),
                state.visited,
                state.errors,
                if state.entry_limit {
                    " · PARTIAL: entry limit"
                } else {
                    ""
                },
                if state.result_limit {
                    " · PARTIAL: result limit"
                } else {
                    ""
                }
            )
        };
        if let Some(error) = &state.scan_error {
            status.push_str(" · ");
            status.push_str(&display_text(error));
        }
        if !state.message.is_empty() {
            status.push_str(" · ");
            status.push_str(&state.message);
        }
        let lines = vec![
            Line::from(Span::styled(
                status,
                if invalid_filters || state.errors > 0 || state.entry_limit || state.result_limit {
                    self.theme.error
                } else {
                    self.theme.accent
                },
            )),
            Line::raw("↑↓ Select  PgUp/Dn Preview  F7 Scope  F8 Hidden  F9 Ignored  F12 Help"),
            Line::raw("Tab Fields  Ctrl-F Query  Ctrl-L Scope  Alt-Left Back  Esc Quit"),
        ];
        frame.render_widget(Paragraph::new(lines).style(self.theme.dim), vertical[3]);
        if self.help {
            let popup = Rect::new(area.x + 4, area.y + 3, area.width - 8, 12);
            frame.render_widget(Clear, popup);
            frame.render_widget(Paragraph::new("Tab / Shift-Tab: Query, Include, Exclude, Scope\nCtrl-F / Ctrl-L: focus Query / Scope\nCtrl-U: clear field; arrows/Home/End: edit\nUp / Down: select; PageUp / PageDown: preview\nF7: Project / immutable launch CWD\nF8 / F9: reveal Hidden / Ignored independently\nInclude / Exclude: relative path regex, smart case\nEnter in Scope: apply path; Alt-Left: Back\nF10: Results / Preview on narrow terminals\nF12 / Esc: close help; Ctrl-C: quit").block(Block::default().borders(Borders::ALL).title(" Keyboard help ")).style(self.theme.base), popup);
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
        if focused && !self.help && available > 0 {
            frame.set_cursor_position((area.x + prefix.width() as u16 + cursor as u16, area.y));
        }
    }
    fn results(&self, frame: &mut Frame, area: Rect, session: &SearchSession) {
        let state = session.state();
        let items: Vec<_> = state
            .results
            .iter()
            .map(|row| ListItem::new(display_text(&row.relative)))
            .collect();
        let mut selection =
            ListState::default().with_selected(state.selected().map(|_| state.selected_index));
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        " Results · {} {} ",
                        state.results.len(),
                        if state.scanning { "· scanning" } else { "" }
                    ))
                    .border_style(self.theme.dim),
            )
            .style(self.theme.base)
            .highlight_style(self.theme.selected)
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut selection);
        if state.results.is_empty() {
            frame.render_widget(
                Paragraph::new(if state.scanning {
                    "Scanning…"
                } else if state.include_error.is_some() || state.exclude_error.is_some() {
                    "Invalid filters · edit marked fields"
                } else {
                    "No matching files"
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
            .title(title)
            .border_style(self.theme.dim);
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
                .map(|line| {
                    Line::from(vec![
                        Span::styled(format!("{:>5}  ", line.number), self.theme.dim),
                        Span::styled(display_text(&line.text), self.theme.base),
                    ])
                })
                .collect()
        };
        frame.render_widget(Paragraph::new(lines).style(self.theme.base), inner);
    }
}
