# Dirpick project brief

Dirpick is a standalone Rust terminal application for filesystem navigation,
search, filtering, and previews. Its command is `dirpick`.

This brief preserves the agreed direction from the project handoff and the
subsequent design interview. It records accepted product choices. The approved
[GitHub spec](https://github.com/vimkim/dirpick/issues/1) defines engineering
defaults and the test boundary.

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
Use Workbench (B) as the preferred layout: group content matches by file,
with the search controls on the left and scope and path filters on the right.
On narrow terminals, switch between Results and Preview panes. Keep the active
search settings visible; final focus and selection keys are in the spec.
The user accepted the native terminal design; the earlier browser mockups did
not look sufficiently like a TUI.

### Appearance

Use Catppuccin Mocha as the default theme. The native prototype applies the
official palette to backgrounds, labels, borders, focus, selections, source
syntax, match highlights, and errors. Terminal monochrome preferences remain
available through the existing `NO_COLOR` behavior.

### Startup

Start at the project root with Files mode selected and the query focused.
Search recursively beneath that directory. Project root means the top of the
Git working tree containing the launch CWD; recognize linked worktrees as well
as repositories with a `.git` directory. Outside a Git working tree, use the
launch CWD as the default scope.

Keep a visible Project / CWD toggle, with Project selected by default. CWD
always means the directory from which Dirpick was launched, even after browsing.
Switching presets preserves the mode, query, include/exclude filters, and Regex
setting, then refreshes results and resets selection. Back restores the previous
scope and search. Browsing or manually applying another scope remains available;
show Custom when it matches neither preset. The prototype uses F7 for the toggle.

An empty Files or Directories query enumerates recursive paths. An empty
Contents query shows guidance. Ranking and operational limits are defined
in the approved spec.

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

### Traversal

Respect `.gitignore`, `.ignore`, and `.rgignore` rules by default. Hidden entries
are off by default. Provide separate visible toggles for hidden and ignored
entries, while keeping `.git` metadata excluded. Recursive search does not
follow symlinks.

### Shared search state

Mode changes keep the query, scope, include/exclude filters, and Regex setting.
The Regex setting applies to Contents queries only. Scope preset changes also
keep these settings; directory browsing and manual scope application clear the
query as specified above. Recompute results after each change.

### Shell and clipboard

Support Nushell integration first. The shell adapter changes the invoking
shell's directory after an explicit cd & exit action. Copy sends the selected
absolute path through OSC 52, matching the existing SSH clipboard setup.
Copy keeps Dirpick open. Bash and Zsh adapters can follow in a later release.

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
commit `e49d91b`, with entry point `prototypes/terminal/Cargo.toml`.
Its checkout is `/home/vimkim/gh/dirpick-prototype`. Run instructions and keys
are in `prototypes/README.md` on that branch. Ratatui and Crossterm are prototype
tools and are also the approved production terminal framework.

It compares a compact flat picker (A), a grouped content workbench (B), and a
directory tree (C), with the accepted controls above results and preview.
F5 switches layouts while preserving the search; F6 loads sample workflows.
F7 switches Project / launch CWD while keeping the search settings.
All data are synthetic. Editor, clipboard, and shell actions remain simulated.
The prototype uses fixed sample roots; real Git-root discovery remains to build.
Workbench (B) is the preferred design and the prototype's startup layout.
A and C remain available for comparison.

Actual PTY checks verified keyboard fields, regex error recovery, matching,
selection, editor-overlay return, directory scope history, Project/CWD switching
with settings preserved and a fixed launch CWD, Unicode paths,
layout switching, resize from 120x32 to 80x24, and terminal flag/alternate-screen
restoration on exit. Captures are under `prototypes/terminal/screenshots/`.
These checks do not validate a real editor process or clipboard transport.

Shared mode-switch state has been accepted. Keep prototype code off the main
branch and carry the selected design into the implementation spec.
Add its branch and path to the relevant implementation ticket when tickets exist.

## Approved spec and implementation graph

The user approved the shell, traversal, and interaction defaults, followed by
the engineering defaults, application-level testing approach, and six-ticket
breakdown. [Spec #1](https://github.com/vimkim/dirpick/issues/1) and implementation
tickets #2–#7 are published with verified native sub-issue and blocking links.
Build complete workflows from the ready frontier on one integration branch.

The [publication index](reviews/first-release-publication.md) links the approved
issues and their dependencies. GitHub Issues are authoritative; the original
review copy is preserved in Git history.

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
The detected login shell is Nu, and `EDITOR` is `nvim`. The existing Nushell
clipboard helper and Neovim clipboard configuration both use OSC 52 over SSH.
Nushell wrappers already use JSON picker output or a cwd file followed by an
ordinary shell `cd`. These observed conventions support the accepted Nushell
and OSC 52 integration choices.

Evidence: `~/.config/nushell/clip.nu`, `~/.config/nvim/lua/config/options.lua`,
and `~/.config/nushell/alias.nu`. The current session reports `TERM_PROGRAM=herdr`;
this does not establish the SSH client's clipboard capabilities. Successful
OSC 52 delivery to that client has not been verified.

A child process cannot change its parent shell's working directory. The directory
selection action needs a shell hook and a reliable path-return protocol. Paths
and editor arguments must remain data, including spaces, quotes, Unicode, leading
dashes, and embedded newlines.
