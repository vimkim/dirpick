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
    pub scope: PathBuf,
    pub mode: SearchMode,
    pub regex: bool,
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
    if request.mode == SearchMode::Contents {
        run_contents(generation, &request, &policy, tx, cancel);
        return;
    }
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
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
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
                text: String::new(),
                matches: Vec::new(),
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

pub(crate) fn content_pattern(query: &str, regex: bool) -> Result<regex::Regex, String> {
    regex::RegexBuilder::new(&if regex {
        query.to_owned()
    } else {
        regex::escape(query)
    })
    .case_insensitive(!query.chars().any(char::is_uppercase))
    .build()
    .map_err(|error| {
        error
            .to_string()
            .lines()
            .last()
            .unwrap_or("Invalid regex")
            .to_owned()
    })
}

fn run_contents(
    generation: u64,
    request: &SearchRequest,
    policy: &VisibilityPolicy,
    tx: &SyncSender<SearchEvent>,
    cancel: Cancellation,
) {
    use std::{fs::File, io::Read};
    const FILE_BYTES: usize = 2 * 1024 * 1024;
    const CONTENT_RESULTS: usize = 5_000;
    const RESULT_BYTES: usize = 16 * 1024 * 1024;
    let mut retained_bytes = 0;
    let pattern = match content_pattern(&request.query, request.regex) {
        Ok(pattern) => pattern,
        Err(message) => {
            deliver(
                tx,
                SearchEvent::InvalidQuery {
                    generation,
                    message,
                },
                &cancel,
                true,
            );
            return;
        }
    };
    let mut rows = Vec::new();
    let (mut visited, mut matched, mut errors, mut skipped) = (0, 0, 0, 0);
    let (mut entry_limit, mut result_limit) = (false, false);
    let mut reported_error = false;
    let mut report = |message: String| {
        if !reported_error {
            deliver(
                tx,
                SearchEvent::ScanError {
                    generation,
                    message,
                },
                &cancel,
                true,
            );
            reported_error = true;
        }
    };
    if !request.query.is_empty() {
        'walk: for entry in policy.walker(&request.scope, cancel.clone()).build() {
            if cancel.cancelled() {
                return;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    errors += 1;
                    report(error.to_string());
                    continue;
                }
            };
            if let Some(error) = entry.error() {
                errors += 1;
                report(error.to_string());
            }
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
            let relative = entry
                .path()
                .strip_prefix(&request.scope)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if !policy.accepts(&relative) {
                continue;
            }
            let file = match File::open(entry.path()) {
                Ok(file) => file,
                Err(error) => {
                    errors += 1;
                    report(format!("{}: {error}", entry.path().display()));
                    continue;
                }
            };
            let mut bytes = Vec::new();
            if let Err(error) = file.take(FILE_BYTES as u64 + 1).read_to_end(&mut bytes) {
                errors += 1;
                report(format!("{}: {error}", entry.path().display()));
                continue;
            }
            if bytes.len() > FILE_BYTES || bytes.contains(&0) {
                skipped += 1;
                continue;
            }
            let Ok(text) = std::str::from_utf8(&bytes) else {
                skipped += 1;
                continue;
            };
            for (index, line) in text.lines().enumerate() {
                if cancel.cancelled() {
                    return;
                }
                let mut matches = Vec::new();
                let line_bytes = line.len()
                    + relative.len()
                    + entry.path().as_os_str().as_encoded_bytes().len()
                    + std::mem::size_of::<SearchResult>();
                for found in pattern.find_iter(line) {
                    if cancel.cancelled() {
                        return;
                    }
                    if matches.is_empty() {
                        matched += 1;
                        if rows.len() == CONTENT_RESULTS {
                            result_limit = true;
                            break 'walk;
                        }
                    }
                    if matches.len() == matches.capacity() {
                        let capacity = matches.capacity().saturating_mul(2).max(4);
                        if retained_bytes
                            + line_bytes
                            + capacity * std::mem::size_of::<std::ops::Range<usize>>()
                            > RESULT_BYTES
                        {
                            result_limit = true;
                            deliver(tx, SearchEvent::ScanError { generation, message: "Content results reached the 16 MiB memory limit · PARTIAL; refine the query or path filters".into() }, &cancel, true);
                            break 'walk;
                        }
                        matches.reserve_exact(capacity - matches.len());
                    }
                    matches.push(found.range());
                }
                if matches.is_empty() {
                    continue;
                }
                retained_bytes +=
                    line_bytes + matches.capacity() * std::mem::size_of::<std::ops::Range<usize>>();
                rows.push(SearchResult {
                    path: entry.path().to_owned(),
                    relative: relative.clone(),
                    line: index + 1,
                    text: line.to_owned(),
                    matches,
                });
                if rows.len() % 128 == 0 {
                    rows.sort_by(|a, b| (&a.relative, a.line).cmp(&(&b.relative, b.line)));
                    deliver(
                        tx,
                        SearchEvent::Results {
                            generation,
                            rows: rows.clone(),
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
        }
    }
    rows.sort_by(|a, b| (&a.relative, a.line).cmp(&(&b.relative, b.line)));
    deliver(
        tx,
        SearchEvent::ContentStats {
            generation,
            skipped,
            result_limit,
        },
        &cancel,
        true,
    );
    deliver(
        tx,
        SearchEvent::Results {
            generation,
            rows,
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
