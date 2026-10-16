//! Path eligibility is shared by recursive searches and later directory previews.
use crate::worker::Cancellation;
use regex::{Regex, RegexBuilder};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub(crate) struct VisibilityPolicy {
    scope: PathBuf,
    filters: PathFilters,
    hidden: bool,
    ignored: bool,
}

impl VisibilityPolicy {
    pub fn compile(
        scope: PathBuf,
        include: &str,
        exclude: &str,
        hidden: bool,
        ignored: bool,
    ) -> Result<Self, FilterErrors> {
        Ok(Self {
            scope,
            filters: PathFilters::compile(include, exclude)?,
            hidden,
            ignored,
        })
    }

    /// `root` may be the scope or a selected descendant for an immediate-child preview.
    /// Include filters affect candidates, never traversal into their parents.
    pub fn walker(&self, root: &Path, cancel: Cancellation) -> ignore::WalkBuilder {
        let mut builder = ignore::WalkBuilder::new(root);
        builder.follow_links(false).hidden(false);
        // The ignore walker does not call filter_entry for its root. An
        // explicitly selected metadata scope must not expose its descendants.
        if self
            .scope
            .components()
            .any(|part| part.as_os_str() == ".git")
        {
            builder.max_depth(Some(0));
        }
        if self.ignored {
            builder.standard_filters(false);
        } else {
            builder
                .parents(true)
                .require_git(false)
                .add_custom_ignore_filename(".rgignore");
        }
        let policy = self.clone();
        builder.filter_entry(move |entry| {
            !cancel.cancelled()
                && entry.file_name() != ".git"
                && (entry.depth() == 0
                    || ((policy.hidden || !entry.file_name().to_string_lossy().starts_with('.'))
                        && !entry.file_type().is_some_and(|kind| kind.is_symlink())
                        && !policy.filters.excludes(
                            &entry
                                .path()
                                .strip_prefix(&policy.scope)
                                .unwrap()
                                .to_string_lossy(),
                        )))
        });
        builder
    }

    pub fn accepts(&self, relative: &str) -> bool {
        self.filters.accepts(relative)
    }
}

#[derive(Clone)]
struct PathFilters {
    include: Regex,
    exclude: Option<Regex>,
}

pub(crate) struct FilterErrors {
    pub include: Option<String>,
    pub exclude: Option<String>,
}

impl PathFilters {
    pub fn compile(include: &str, exclude: &str) -> Result<Self, FilterErrors> {
        let included = compile(include);
        let excluded = compile(exclude);
        match (included, excluded) {
            (Ok(include), Ok(excluded)) => Ok(Self {
                include,
                exclude: (!exclude.is_empty()).then_some(excluded),
            }),
            (include, exclude) => Err(FilterErrors {
                include: include.err(),
                exclude: exclude.err(),
            }),
        }
    }

    pub fn accepts(&self, relative: &str) -> bool {
        self.include.is_match(relative) && !self.excludes(relative)
    }

    pub fn excludes(&self, relative: &str) -> bool {
        self.exclude
            .as_ref()
            .is_some_and(|pattern| pattern.is_match(relative))
    }
}

fn compile(pattern: &str) -> Result<Regex, String> {
    RegexBuilder::new(pattern)
        .case_insensitive(!pattern.chars().any(char::is_uppercase))
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
