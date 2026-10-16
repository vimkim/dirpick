use crate::{
    session::{Limits, SearchEvent, SearchMode, SearchResult},
    traversal::VisibilityPolicy,
    worker::Cancellation,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::mpsc::{SyncSender, TrySendError},
};

pub(crate) struct SearchRequest {
    pub mode: SearchMode,
    pub scope: PathBuf,
    pub query: String,
    pub include: String,
    pub exclude: String,
    pub hidden: bool,
    pub ignored: bool,
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
    let policy = match VisibilityPolicy::compile(
        request.scope.clone(),
        &request.include,
        &request.exclude,
        request.hidden,
        request.ignored,
    ) {
        Ok(filters) => filters,
        Err(errors) => {
            deliver(
                tx,
                SearchEvent::InvalidFilters {
                    generation,
                    include: errors.include,
                    exclude: errors.exclude,
                },
                &cancel,
                true,
            );
            return;
        }
    };
    let mut retained = BTreeMap::new();
    let (mut visited, mut matched, mut errors) = (0, 0, 0);
    let mut entry_limit = false;
    let mut reported_error = false;
    let mut report = |error: &ignore::Error| {
        if !reported_error {
            deliver(
                tx,
                SearchEvent::ScanError {
                    generation,
                    message: error.to_string(),
                },
                &cancel,
                true,
            );
            reported_error = true;
        }
    };
    for entry in policy.walker(&request.scope, cancel.clone()).build() {
        if cancel.cancelled() {
            return;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                errors += 1;
                report(&error);
                continue;
            }
        };
        if let Some(error) = entry.error() {
            errors += 1;
            report(error);
        }
        if entry.depth() == 0 {
            continue;
        }
        if visited == request.limits.entries {
            entry_limit = true;
            break;
        }
        visited += 1;
        if !entry.file_type().is_some_and(|kind| match request.mode {
            SearchMode::Files => kind.is_file(),
            SearchMode::Directories => kind.is_dir(),
            SearchMode::Contents => false,
        }) {
            continue;
        }
        let relative_path = entry
            .path()
            .strip_prefix(&request.scope)
            .unwrap()
            .to_owned();
        let relative = relative_path.to_string_lossy().into_owned();
        if !policy.accepts(&relative) {
            continue;
        }
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
