use crate::{
    session::{Limits, SearchEvent, SearchMode, SearchResult, SearchSettings},
    traversal::VisibilityPolicy,
    worker::Cancellation,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::mpsc::{SyncSender, TrySendError},
};

pub(crate) struct SearchRequest {
    pub settings: SearchSettings,
    pub limits: Limits,
}

struct Candidate {
    path: PathBuf,
    relative_path: PathBuf,
    relative: String,
}

/// Own the scan's common traversal, eligibility, and accounting. Matching,
/// retention, and streaming cadence remain the responsibility of each mode.
struct SearchTraversal<'a> {
    walk: ignore::Walk,
    request: &'a SearchRequest,
    policy: &'a VisibilityPolicy,
    generation: u64,
    tx: &'a SyncSender<SearchEvent>,
    cancel: &'a Cancellation,
    visited: usize,
    errors: usize,
    entry_limit: bool,
    reported_error: bool,
}

impl<'a> SearchTraversal<'a> {
    fn new(
        generation: u64,
        request: &'a SearchRequest,
        policy: &'a VisibilityPolicy,
        tx: &'a SyncSender<SearchEvent>,
        cancel: &'a Cancellation,
    ) -> Self {
        Self {
            walk: policy
                .walker(&request.settings.scope, cancel.clone())
                .build(),
            request,
            policy,
            generation,
            tx,
            cancel,
            visited: 0,
            errors: 0,
            entry_limit: false,
            reported_error: false,
        }
    }

    fn error(&mut self, message: impl ToString) {
        self.errors += 1;
        if !self.reported_error {
            deliver(
                self.tx,
                SearchEvent::ScanError {
                    generation: self.generation,
                    message: message.to_string(),
                },
                self.cancel,
                true,
            );
            self.reported_error = true;
        }
    }

    fn next(&mut self) -> Option<Candidate> {
        loop {
            if self.cancel.cancelled() {
                return None;
            }
            let entry = match self.walk.next()? {
                Ok(entry) => entry,
                Err(error) => {
                    self.error(error);
                    continue;
                }
            };
            if let Some(error) = entry.error() {
                self.error(error);
            }
            if entry.depth() == 0 {
                continue;
            }
            if self.visited == self.request.limits.entries {
                self.entry_limit = true;
                return None;
            }
            self.visited += 1;
            if !entry.file_type().is_some_and(|kind| {
                if self.request.settings.mode == SearchMode::Directories {
                    kind.is_dir()
                } else {
                    kind.is_file()
                }
            }) {
                continue;
            }
            let relative_path = entry
                .path()
                .strip_prefix(&self.request.settings.scope)
                .unwrap()
                .to_owned();
            let relative = relative_path.to_string_lossy().into_owned();
            if self.policy.accepts(&relative) {
                return Some(Candidate {
                    path: entry.into_path(),
                    relative_path,
                    relative,
                });
            }
        }
    }
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
        request.settings.scope.clone(),
        &request.settings.include,
        &request.settings.exclude,
        request.settings.hidden,
        request.settings.ignored,
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
    if request.settings.mode == SearchMode::Contents {
        run_contents(generation, &request, &policy, tx, cancel);
        return;
    }
    let mut retained = BTreeMap::new();
    let mut matched = 0;
    let mut traversal = SearchTraversal::new(generation, &request, &policy, tx, &cancel);
    while let Some(candidate) = traversal.next() {
        let Candidate {
            path,
            relative_path,
            relative,
        } = candidate;
        let Some(score) = rank(&request.settings.query, &relative) else {
            continue;
        };
        matched += 1;
        retained.insert(
            (score, relative_path),
            SearchResult {
                path,
                relative,
                line: 1,
                text: String::new(),
                matches: Vec::new(),
            },
        );
        if retained.len() > request.limits.results {
            retained.pop_last();
        }
        if traversal.visited.is_multiple_of(128) {
            deliver(
                tx,
                SearchEvent::Results {
                    generation,
                    rows: retained.values().cloned().collect(),
                    visited: traversal.visited,
                    matched,
                    errors: traversal.errors,
                    entry_limit: traversal.entry_limit,
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
            visited: traversal.visited,
            matched,
            errors: traversal.errors,
            entry_limit: traversal.entry_limit,
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
    let pattern = match content_pattern(&request.settings.query, request.settings.regex) {
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
    let (mut matched, mut skipped) = (0, 0);
    let mut result_limit = false;
    let mut traversal = SearchTraversal::new(generation, request, policy, tx, &cancel);
    if !request.settings.query.is_empty() {
        'walk: while let Some(candidate) = traversal.next() {
            let Candidate { path, relative, .. } = candidate;
            let file = match File::open(&path) {
                Ok(file) => file,
                Err(error) => {
                    traversal.error(format!("{}: {error}", path.display()));
                    continue;
                }
            };
            let mut bytes = Vec::new();
            if let Err(error) = file.take(FILE_BYTES as u64 + 1).read_to_end(&mut bytes) {
                traversal.error(format!("{}: {error}", path.display()));
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
                    + path.as_os_str().as_encoded_bytes().len()
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
                    path: path.clone(),
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
                            visited: traversal.visited,
                            matched,
                            errors: traversal.errors,
                            entry_limit: traversal.entry_limit,
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
            visited: traversal.visited,
            matched,
            errors: traversal.errors,
            entry_limit: traversal.entry_limit,
            done: true,
        },
        &cancel,
        true,
    );
}
