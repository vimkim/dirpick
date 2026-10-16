use crate::{
    session::{Limits, SearchEvent, SearchResult},
    worker::Cancellation,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::mpsc::{SyncSender, TrySendError},
};

pub(crate) struct SearchRequest {
    pub scope: PathBuf,
    pub query: String,
    pub limits: Limits,
}

pub(crate) fn deliver(
    tx: &SyncSender<SearchEvent>,
    mut event: SearchEvent,
    cancel: &Cancellation,
    required: bool,
) {
    loop {
        if cancel.cancelled() {
            return;
        }
        match tx.try_send(event) {
            Ok(()) | Err(TrySendError::Disconnected(_)) => return,
            Err(TrySendError::Full(returned)) => {
                if !required {
                    return;
                }
                event = returned;
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }
}

pub(crate) fn run(
    generation: u64,
    request: SearchRequest,
    tx: &SyncSender<SearchEvent>,
    cancel: Cancellation,
) {
    let mut retained = BTreeMap::new();
    let (mut visited, mut matched, mut errors) = (0, 0, 0);
    let mut entry_limit = false;
    let mut walker = ignore::WalkBuilder::new(&request.scope);
    walker
        .follow_links(false)
        .hidden(false)
        .add_custom_ignore_filename(".rgignore")
        .filter_entry(|entry| {
            entry.file_name() != ".git"
                && (entry.depth() == 0
                    || (!entry.file_name().to_string_lossy().starts_with('.')
                        && !entry.file_type().is_some_and(|kind| kind.is_symlink())))
        });
    for entry in walker.build() {
        if cancel.cancelled() {
            return;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }
        if visited == request.limits.entries {
            entry_limit = true;
            break;
        }
        visited += 1;
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let relative_path = entry
            .path()
            .strip_prefix(&request.scope)
            .unwrap()
            .to_owned();
        let relative = relative_path.to_string_lossy().into_owned();
        let Some(score) = rank(&request.query, &relative) else {
            continue;
        };
        matched += 1;
        retained.insert(
            (score, relative_path),
            SearchResult {
                path: entry.into_path(),
                relative,
                line: 1,
            },
        );
        if retained.len() > request.limits.results {
            retained.pop_last();
        }
        if visited % 128 == 0 {
            deliver(
                tx,
                SearchEvent::Results {
                    generation,
                    rows: retained.values().cloned().collect(),
                    visited,
                    matched,
                    errors,
                    entry_limit,
                    done: false,
                },
                &cancel,
                false,
            );
        }
    }
    deliver(
        tx,
        SearchEvent::Results {
            generation,
            rows: retained.into_values().collect(),
            visited,
            matched,
            errors,
            entry_limit,
            done: true,
        },
        &cancel,
        true,
    );
}

fn rank(query: &str, relative: &str) -> Option<u8> {
    let sensitive = query.chars().any(char::is_uppercase);
    let (query, relative) = if sensitive {
        (query.to_owned(), relative.to_owned())
    } else {
        (query.to_lowercase(), relative.to_lowercase())
    };
    let tokens: Vec<_> = query.split_whitespace().collect();
    if tokens.is_empty() {
        return Some(0);
    }
    if !tokens.iter().all(|token| {
        let mut haystack = relative.chars();
        token
            .chars()
            .all(|needle| haystack.any(|character| character == needle))
    }) {
        return None;
    }
    let basename = relative.rsplit('/').next().unwrap_or(&relative);
    let basename_query = query.trim();
    Some(if basename == basename_query {
        0
    } else if basename.starts_with(basename_query) {
        1
    } else {
        2
    })
}
