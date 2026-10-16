# Dirpick project brief

Dirpick is a standalone Rust terminal application for filesystem navigation,
search, filtering, and previews. Its command is `dirpick`.

This brief preserves the agreed direction from the project handoff and the
subsequent design interview. It records accepted choices; remaining details
need agreement before the implementation spec is written.

## Settled project choices

- Language: Rust.
- Primary workspace: `/home/vimkim/gh/dirpick`.
- Repository: <https://github.com/vimkim/dirpick>, public.
- Specs and implementation tickets: GitHub Issues in `vimkim/dirpick`.
- Overall work item: 403 in work-tracker.
- One domain context, with a root glossary and `docs/adr/` created as needed.

## Desired experience

The user wants a coherent interaction for choosing a scope and search mode,
entering a query, refining filename filters, inspecting results and source
previews, and acting on a selection. Controls must be easy to discover and
change throughout this workflow.

Desired outcomes include:

- Find file and directory paths, and copy a selected path.
- Search file contents with literal or regex queries and filename
  include/exclude filters.
- Browse directories and change the invoking shell to the selected directory
  through shell integration.
- Preview source with syntax highlighting and matching-line context.
- Open a content result in an editor at the matching line.

The user has tried the proposed existing tools and found their interactions
unsuitable. Further tool comparisons should address a specific design question.

## Accepted design decisions

The user accepted the following choices on 2026-10-16.

### First release

The first release includes Files, Directories, and Contents modes, path copying,
shell `cd` integration, syntax previews, and opening an editor at the matching
line. Build complete workflows one at a time within that release.

### Layout and focus

Mode, scope, query, include, and exclude controls stay visible above side-by-side
results and preview. Tab moves between fields. Current settings remain visible.
Narrow-terminal behavior and the remaining keybindings are still open.
The user found the browser mockups did not look like a TUI. Evaluate the layout
in the running terminal prototype when choosing the design.

### Appearance

Use Catppuccin Mocha as the default theme. The native prototype applies the
official palette to backgrounds, labels, borders, focus, selections, source
syntax, match highlights, and errors. Terminal monochrome preferences remain
available through the existing `NO_COLOR` behavior.

### Startup

Start in the launching directory with Files mode selected and the query focused.
Search recursively beneath that directory. Enumeration, limits, ranking, and
empty-query behavior remain open.

### Query matching

Files and Directories use fuzzy path matching. Contents uses literal matching
by default, with a visible Regex toggle. Matching is case-insensitive unless
the query contains uppercase.

### Include and exclude filters

Include and exclude fields contain regexes matched against paths relative to
the search scope, across all three modes. Exclusion wins when both match.
For example, `\.rs$` includes Rust files and `(^|/)tests(/|$)` excludes a
tests directory and its descendants. These filters operate independently of
the Contents query's Regex toggle.

### Actions and restoration

- Enter on a directory changes the search scope to it and clears the query.
  Back restores the previous search.
- Enter on a file or content result opens the editor. Closing the editor
  restores the search and selection. Content results open at the matching line.
- A separate `cd & exit` action changes the invoking shell's directory through
  shell integration.
- Copy keeps Dirpick open.

## Layout prototype

The user requested design prototypes after accepting the matching, filter,
and action contracts. The browser study remains at commit `ed6eb3a` as
historical evidence; the user rejected its appearance as insufficiently TUI-like.

The current study is a native Rust TUI on local branch `prototype/search-layouts`,
commit `ad47a4f`, with entry point `prototypes/terminal/Cargo.toml`.
Its checkout is `/home/vimkim/gh/dirpick-prototype`. Run instructions and keys
are in `prototypes/README.md` on that branch. Ratatui and Crossterm are prototype
tools; the production framework choice remains open.

It compares a compact flat picker (A), a grouped content workbench (B), and a
directory tree (C), with the accepted controls above results and preview.
F5 switches layouts while preserving the search; F6 loads sample workflows.
All data are synthetic. Editor, clipboard, and shell actions remain simulated.
No winning layout has been selected.

Actual PTY checks verified keyboard fields, regex error recovery, matching,
selection, editor-overlay return, directory scope history, Unicode paths,
layout switching, resize from 120x32 to 80x24, and terminal flag/alternate-screen
restoration on exit. Captures are under `prototypes/terminal/screenshots/`.
These checks do not validate a real editor process or clipboard transport.

Shared mode-switch state remains a prototype assumption. Keep prototype code
off the main branch and carry the selected design into the implementation spec.
Add its branch and path to the relevant implementation ticket when tickets exist.

## Decisions still open

- The order of complete workflows within the agreed release.
- Narrow-terminal layout and remaining focus/selection keybindings.
- Ranking and empty-query behavior in each mode.
- State preserved across mode changes and manual scope edits.
- Hidden entries, ignored entries, symlinks, and traversal boundaries.
- Clipboard destination and formats; supported shells and the directory-return protocol.
- Editor command selection and terminal handoff details.
- Search cancellation, large-directory behavior, and result limits.
- Terminal framework, search backend, highlighting engine, and packaging.

## Behavioral reference

Docserve's Finder is a behavioral reference, not an implementation dependency.
Reference checkout: `/home/vimkim/gh/docserve`.

- `docs/guide.md`: Finding and revisiting paths.
- `GLOSSARY.md`: Finder and Preview terminology.
- `src/html_serve/finder.py`: search, traversal, filtering, and preview extraction.
- `src/html_serve/assets/navigation.js` and `navigation.css`: controls and selection.
- `tests/test_finder.py`: search edge cases.
- `docs/search-navigation-history-design.md`: prior design reasoning.

Its web API, serving-root restrictions, and execution limits are not automatically
Dirpick requirements. Its Finder preview shows plain source; syntax highlighting
is a desired Dirpick capability.

## Environment

Development runs on remote Linux over SSH. Use terminal tools and headless checks.
The detected login shell is Nu; supported everyday shells and the local terminal
client still need confirmation.

A child process cannot change its parent shell's working directory. The directory
selection action needs a shell hook and a reliable path-return protocol. Paths
and editor arguments must remain data, including spaces, quotes, Unicode, leading
dashes, and embedded newlines.
