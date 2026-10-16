use crate::{
    search::deliver,
    session::{Preview, PreviewLine, PreviewSpan, SearchEvent},
    worker::Cancellation,
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Color, FontStyle, StyleModifier, Theme, ThemeItem},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

thread_local! {
    // Construct and retain parsing resources on the preview worker. Syntect's
    // parser state must not cross threads, and terminal input never loads it.
    static SYNTAXES: SyntaxSet = SyntaxSet::load_defaults_newlines();
    static THEME: Theme = mocha();
}
use std::{
    fs::File,
    io::{ErrorKind, Read},
    path::PathBuf,
    sync::mpsc::SyncSender,
};

pub(crate) struct PreviewRequest {
    pub path: PathBuf,
    pub top: usize,
}
pub(crate) fn run(
    id: u64,
    request: PreviewRequest,
    tx: &SyncSender<SearchEvent>,
    cancel: Cancellation,
) {
    if let Some(preview) = read(&request, &cancel) {
        deliver(tx, SearchEvent::Preview { id, preview }, &cancel, true);
    }
}
fn read(request: &PreviewRequest, cancel: &Cancellation) -> Option<Preview> {
    const FILE_BYTES: usize = 2 * 1024 * 1024;
    let mut preview = Preview {
        top: request.top,
        ..Preview::default()
    };
    let file = match File::open(&request.path) {
        Ok(file) => file,
        Err(error) => {
            preview.status = if error.kind() == ErrorKind::NotFound {
                "File vanished".into()
            } else {
                format!("Cannot read file: {error}")
            };
            return Some(preview);
        }
    };
    if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
        preview.status = "No longer a regular file".into();
        return Some(preview);
    }
    let mut bytes = Vec::new();
    if let Err(error) = file.take(FILE_BYTES as u64 + 1).read_to_end(&mut bytes) {
        preview.status = format!("Cannot read file: {error}");
        return Some(preview);
    }
    if bytes.len() > FILE_BYTES {
        preview.status = "File exceeds 2 MiB preview limit".into();
        return Some(preview);
    }
    if bytes.contains(&0) {
        preview.status = "Binary file · path remains searchable".into();
        return Some(preview);
    }
    let Ok(text) = std::str::from_utf8(&bytes) else {
        preview.status = "Invalid UTF-8 · path remains searchable".into();
        return Some(preview);
    };
    preview.total_lines = text.lines().count();
    preview.top = request.top.min(preview.total_lines.saturating_sub(1));
    if cancel.cancelled() {
        return None;
    }
    SYNTAXES.with(|syntaxes| {
        THEME.with(|theme| {
            let syntax = request
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| {
                    syntaxes.find_syntax_by_extension(name).or_else(|| {
                        request
                            .path
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .and_then(|ext| syntaxes.find_syntax_by_extension(ext))
                    })
                })
                .or_else(|| {
                    text.lines()
                        .next()
                        .filter(|line| line.len() <= 4096)
                        .and_then(|line| syntaxes.find_syntax_by_first_line(line))
                });
            let mut highlighter = syntax.map(|syntax| HighlightLines::new(syntax, theme));
            let mut used = 0;
            for (index, source) in LinesWithEndings::from(text).enumerate() {
                if cancel.cancelled() || index >= preview.top.saturating_add(200) {
                    break;
                }
                // Parse from line one to preserve multiline comments and strings.
                // Never run regex highlighting against an unbounded source line.
                if source.len() > 4096 && highlighter.is_some() {
                    highlighter = None;
                    preview.status = "Long source line · plain text from here".into();
                }
                let styles = match highlighter
                    .as_mut()
                    .map(|h| h.highlight_line(source, syntaxes))
                {
                    Some(Ok(styles)) => styles,
                    Some(Err(_)) => {
                        highlighter = None;
                        preview.status = "Syntax unavailable · showing plain text".into();
                        Vec::new()
                    }
                    None => Vec::new(),
                };
                if index < preview.top {
                    continue;
                }
                let line = source
                    .strip_suffix('\n')
                    .map_or(source, |line| line.strip_suffix('\r').unwrap_or(line));
                let available = (64 * 1024usize).saturating_sub(used);
                if available == 0 {
                    preview.status = "Preview window limited to 64 KiB".into();
                    break;
                }
                let mut end = line.len().min(available);
                while !line.is_char_boundary(end) {
                    end -= 1;
                }
                let mut offset = 0;
                let spans = styles
                    .into_iter()
                    .filter_map(|(style, fragment)| {
                        let start = offset;
                        offset += fragment.len();
                        let range = start..offset.min(end);
                        (!range.is_empty()).then_some(PreviewSpan {
                            range,
                            foreground: [
                                style.foreground.r,
                                style.foreground.g,
                                style.foreground.b,
                            ],
                            bold: style.font_style.contains(FontStyle::BOLD),
                            italic: style.font_style.contains(FontStyle::ITALIC),
                            underline: style.font_style.contains(FontStyle::UNDERLINE),
                        })
                    })
                    .collect();
                preview.lines.push(PreviewLine {
                    number: index + 1,
                    text: line[..end].to_owned(),
                    spans,
                    matches: Vec::new(),
                });
                used += end;
                if end < line.len() {
                    preview.status =
                        "Source line clipped · preview window limited to 64 KiB".into();
                    break;
                }
            }
        })
    });
    if preview.total_lines == 0 {
        preview.status = "Empty file".into();
    }
    (!cancel.cancelled()).then_some(preview)
}

fn mocha() -> Theme {
    fn color(rgb: [u8; 3]) -> Color {
        Color {
            r: rgb[0],
            g: rgb[1],
            b: rgb[2],
            a: 255,
        }
    }
    let mut theme = Theme::default();
    theme.settings.foreground = Some(color([205, 214, 244]));
    theme.settings.background = Some(color([30, 30, 46]));
    for (scope, rgb) in [
        ("comment", [108, 112, 134]),
        ("string", [166, 227, 161]),
        ("constant.numeric, constant.language", [250, 179, 135]),
        ("keyword, storage", [203, 166, 247]),
        ("entity.name.function, support.function", [137, 180, 250]),
        (
            "entity.name.type, entity.name.class, support.type",
            [249, 226, 175],
        ),
        ("variable.parameter", [235, 160, 172]),
        ("punctuation", [186, 194, 222]),
        ("invalid", [243, 139, 168]),
    ] {
        theme.scopes.push(ThemeItem {
            scope: scope.parse().expect("built-in syntax scope"),
            style: StyleModifier {
                foreground: Some(color(rgb)),
                ..StyleModifier::default()
            },
        });
    }
    theme
}

/// Render control characters visibly without changing the underlying native path.
pub fn display_text(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            if character.is_control() {
                character.escape_default().collect()
            } else {
                vec![character]
            }
        })
        .collect()
}
