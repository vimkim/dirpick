use crate::{
    search::deliver,
    session::{Preview, PreviewLine, SearchEvent},
    worker::Cancellation,
};
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
    let preview = read(&request);
    deliver(tx, SearchEvent::Preview { id, preview }, &cancel, true);
}
fn read(request: &PreviewRequest) -> Preview {
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
            return preview;
        }
    };
    if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
        preview.status = "No longer a regular file".into();
        return preview;
    }
    let mut bytes = Vec::new();
    if let Err(error) = file.take(FILE_BYTES as u64 + 1).read_to_end(&mut bytes) {
        preview.status = format!("Cannot read file: {error}");
        return preview;
    }
    if bytes.len() > FILE_BYTES {
        preview.status = "File exceeds 2 MiB preview limit".into();
        return preview;
    }
    if bytes.contains(&0) {
        preview.status = "Binary file · path remains searchable".into();
        return preview;
    }
    let Ok(text) = std::str::from_utf8(&bytes) else {
        preview.status = "Invalid UTF-8 · path remains searchable".into();
        return preview;
    };
    preview.total_lines = text.lines().count();
    preview.top = request.top.min(preview.total_lines.saturating_sub(1));
    let mut used = 0;
    for (index, line) in text.lines().enumerate().skip(preview.top).take(200) {
        if cancel_line_too_large(line.len(), used) {
            preview.status = "Preview window limited to 64 KiB".into();
            break;
        }
        used += line.len();
        preview.lines.push(PreviewLine {
            number: index + 1,
            text: line.to_owned(),
        });
    }
    if preview.total_lines == 0 {
        preview.status = "Empty file".into();
    }
    preview
}
fn cancel_line_too_large(bytes: usize, used: usize) -> bool {
    bytes.saturating_add(used) > 64 * 1024
}

/// Render control characters visibly without changing the underlying native path.
pub fn display_text(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            if character == '\t' {
                "    ".chars().collect::<Vec<_>>()
            } else if character.is_control() {
                character.escape_default().collect()
            } else {
                vec![character]
            }
        })
        .collect()
}
