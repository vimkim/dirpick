# Dirpick

Dirpick is a native terminal workbench for finding and previewing real files.
It runs on Linux terminals and over SSH, without Node, a browser, or a service.

## Run

```sh
cargo run --locked
```

Launch inside a Git working tree to search its Project root, including linked
worktrees and submodules. Outside Git, Project uses the launch directory.
The visible Project/CWD preset remains distinct when both paths are equal.

Type whitespace-separated fuzzy path fragments in Query. Uppercase characters
make the query case-sensitive. Exact basenames rank before basename prefixes,
then other fuzzy matches. An empty query lists files in relative-path order.

The current build implements Files mode and plain source previews.
The [approved release spec](https://github.com/vimkim/dirpick/issues/1) also
covers directory/content modes, filters, syntax colors, and external actions.
Those features arrive in subsequent implementation tickets.

## Keys

| Key | Effect |
| --- | --- |
| Up / Down | Select a file |
| PageUp / PageDown | Scroll the preview independently |
| F7 | Switch Project / launch CWD, retaining the query |
| Tab / Shift-Tab | Move between Query and Scope |
| Ctrl-F / Ctrl-L | Focus Query / Scope |
| Ctrl-U | Clear the focused field |
| Left / Right / Home / End | Move the field cursor |
| Enter in Scope | Apply a directory and clear Query |
| Alt-Left | Restore the previous scope, query, selection, and preview position |
| F10 | Switch Results / Preview below 100 columns |
| F12 | Show keyboard help |
| Esc | Close help, or quit |
| Ctrl-C | Quit |

Relative scope paths resolve against the active scope. Invalid paths retain the
active search and show an error. CWD always means the original launch directory.
The interface supports terminals of at least 70 columns and 18 rows.
Set `NO_COLOR` to use terminal default colors with selection markers and attributes.

## Limits and terminal behavior

Search and preview work run outside terminal input handling. Superseded searches
cannot replace current results. A search visits at most 50,000 entries and retains
the best 2,000 file paths. It continues scanning after the retention limit.
The status line identifies scanning, errors, and partial results.

Traversal respects ignore files and hides dot entries. It excludes `.git` files
and directories and skips symlink entries. Binary and invalid UTF-8 files remain
searchable, with a preview status instead of source text.
Previews support files up to 2 MiB. Each preview window contains at most 200 lines
and 64 KiB. Control characters appear as visible escapes.

Dirpick uses `/dev/tty` for its interface and leaves stdout free of terminal output.
Normal exit, input errors, failed terminal setup, and unwinding restore raw mode
and the alternate screen where cleanup remains possible.

## Check and build

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
```

Tests use real filesystem and Git fixtures at the application session boundary.
The focused Linux PTY check also requires Python 3 from the system.
It verifies real input, preview, help, narrow panes, `NO_COLOR`, and terminal flags.

Test installation without replacing an active command:

```sh
install_root=$(mktemp -d)
cargo install --path . --locked --root "$install_root"
"$install_root/bin/dirpick" --help
```

The accepted visual reference is the native Workbench prototype on local branch
`prototype/search-layouts`, commit `e49d91b`, in `prototypes/terminal`.
Production discovery and previews use the real filesystem, not prototype data.
