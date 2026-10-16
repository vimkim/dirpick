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

Include and Exclude are regex filters over slash-separated paths relative to the
active scope. Each filter uses smart case independently of Query. Empty filters
leave paths unconstrained. Exclude wins and prunes matching directories; Include
never prevents traversal into a directory that could have matching descendants.
An invalid pattern marks its field, explains the error, and clears stale results
until the pattern is fixed, including when Query or the directory is empty.

The current build implements Files mode, path filters, and plain source previews.
The [approved release spec](https://github.com/vimkim/dirpick/issues/1) also
covers directory/content modes, syntax colors, and external actions.
Those features arrive in subsequent implementation tickets.

## Keys

| Key | Effect |
| --- | --- |
| Up / Down | Select a file |
| PageUp / PageDown | Scroll the preview independently |
| F7 | Switch Project / launch CWD, retaining the query |
| F8 | Reveal / hide dotfiles and dot directories |
| F9 | Bypass / respect ignore rules |
| Tab / Shift-Tab | Cycle Query, Include, Exclude, and Scope |
| Ctrl-F / Ctrl-L | Focus Query / Scope |
| Ctrl-U | Clear the focused field |
| Left / Right / Home / End | Move the field cursor |
| Enter in Scope | Apply a directory and clear only Query |
| Alt-Left | Restore the previous scope, fields, toggles, selection, and preview position |
| F10 | Switch Results / Preview below 100 columns |
| F12 | Show keyboard help |
| Esc | Close help, or quit |
| Ctrl-C | Quit |

Relative scope paths resolve against the active scope. Invalid paths retain the
active search and show an error. CWD always means the original launch directory.
Scope changes retain Include, Exclude, Hidden, and Ignored settings.
The interface supports terminals of at least 70 columns and 18 rows.
Set `NO_COLOR` to use terminal default colors with selection markers and attributes.

## Limits and terminal behavior

Search and preview work run outside terminal input handling. Superseded searches
cannot replace current results. A search visits at most 50,000 entries and retains
the best 2,000 file paths. It continues scanning after the retention limit.
The status line identifies scanning, errors, and partial results.

Traversal respects `.gitignore`, `.ignore`, `.rgignore`, and Git exclude rules,
including parent rules for nested CWD and Custom scopes. Hidden and Ignored are
independent: ignore-file whitelists cannot reveal dot entries while Hidden is off.
Explicit path filters apply with either toggle enabled. Traversal always excludes
`.git` files and directories and skips symlink entries, including cycles. A scope
explicitly selected through a symlink resolves to its target. Permission failures
and vanished paths count as scan errors and show the first affected path.
Binary and invalid UTF-8 files remain searchable, with a preview status instead
of source text.
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
It verifies real input, path-filter editing and errors, independent traversal
toggles, preview, help, narrow panes, `NO_COLOR`, and terminal flags.

Test installation without replacing an active command:

```sh
install_root=$(mktemp -d)
cargo install --path . --locked --root "$install_root"
"$install_root/bin/dirpick" --help
```

The accepted visual reference is the native Workbench prototype on local branch
`prototype/search-layouts`, commit `e49d91b`, in `prototypes/terminal`.
Production discovery and previews use the real filesystem, not prototype data.
