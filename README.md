# Dirpick

Dirpick is a native terminal workbench for finding files and text, previewing source, and opening an editor.
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

F1 selects Files and F2 selects Directories. Both use the same fuzzy path queries,
filters, Hidden, and Ignored settings. Directory previews list immediate children
using the active search scope's relative-path filters and visibility rules. Enter
on a directory browses into it and clears only Query; Alt-Left restores the prior
mode, scope, settings, selection, and preview position when available.

F3 selects Contents mode: queries match literal text on individual lines by
default. F4 toggles Regex. Queries use smart case in both settings; filters keep
their independent regex and case rules. An empty content query shows guidance
without scanning. Matching lines are grouped by file with 1-based line numbers;
source previews show every matching span and start near the selected line.
Mode changes keep Query, Include, Exclude, scope, Regex, Hidden, and Ignored.

Enter opens the selected file at line one, or a content result at its exact
line. Dirpick resolves VISUAL, then EDITOR, then nvim, parsing quoted executable
arguments without shell evaluation. Vim and Neovim receive a separate +line
argument before -- and the literal absolute path. Other editors can open file
results; content locations require vim/nvim and show setup guidance for unsupported
commands. The editor owns /dev/tty with normal terminal modes; success, nonzero
exit, Ctrl-C interruption, and spawn failure restore the search, selection,
preview position, and TUI.

Directory browsing and Nushell integration follow the
[approved release spec](https://github.com/vimkim/dirpick/issues/1).

## Keys

| Key | Effect |
| --- | --- |
| F1 / F2 / F3 | Select Files / Directories / Contents |
| F4 | Toggle content Regex; retain it when changing modes |
| Enter | Open the selected file or content line in the configured editor |
| Enter on a directory | Browse inside Dirpick; clear only Query |
| Ctrl-G | cd & exit for a directory through the Nushell wrapper |
| Up / Down | Select a result |
| PageUp / PageDown | Scroll the preview independently |
| Ctrl-Y | Send the exact selected absolute path through OSC 52; keep searching |
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
the best 2,000 file or directory paths. It continues scanning after the retention limit.
Contents retains at most the first 5,000 matching lines and skips files over
2 MiB, binary files, and invalid UTF-8, with skipped and failed counts. Content
results also have a 16 MiB aggregate memory budget for source/path bytes and
allocated match ranges. Reaching it stops before retaining an incomplete line
and reports partial results with refinement guidance. These files remain
available in Files mode. The status line identifies scanning, errors, and limits.

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
and 64 KiB of source text. Control characters appear as visible escapes. Embedded
syntax definitions provide Mocha colors without runtime downloads, with plain
text for unknown syntax. Multiline parsing retains preceding source context.
Lines over 4 KiB disable syntax parsing from that line onward; oversized window
lines show a clipped prefix with a limit notice. PageDown can inspect later lines.

Ctrl-Y writes the selected absolute UTF-8 path to the controlling terminal as an
OSC 52 clipboard payload. The feedback says **payload sent**, because terminal
and SSH client clipboard acceptance cannot be confirmed. Write/flush failures
leave the search and selection intact. Native path values retain spaces, quotes,
Unicode, trailing whitespace, and newlines; displayed escapes are never copied.
Paths outside the UTF-8 transport contract show a limitation instead of a lossy
substitution. Your terminal must support and permit OSC 52 clipboard writes.

Syntax-data and added transport dependency notices ship in
`THIRD_PARTY_NOTICES.txt` and in the installed command via
`dirpick --third-party-notices`. These third-party notices do not select a
license for Dirpick itself.

Dirpick uses `/dev/tty` for its interface and leaves stdout free of terminal output.
Normal exit, input errors, failed terminal setup, and unwinding restore raw mode
and the alternate screen where cleanup remains possible.

## Nushell setup

Install the command with `cargo install --path . --locked`. Generate the supplied
wrapper and load it in Nushell:

```nu
dirpick --print-nu-wrapper | save -f ~/.config/nushell/dirpick.nu
use ~/.config/nushell/dirpick.nu dp
dp
```

To keep `dp` available in future shells, add that `use` line to your Nu config
yourself. Dirpick does not edit shell configuration. The wrapper is embedded in
the binary, so it remains available after a temporary Cargo installation without
the source checkout or a separate data installation.

Within `dp`, F2 searches directories, Enter browses without changing the shell,
and Ctrl-G changes the invoking Nu directory and exits. The wrapper uses ordinary
`cd`, preserving Nu's normal directory behavior. It passes decoded paths as data;
the appended separator preserves names ending in spaces, tabs, or newlines.
Cancellation, plain quitting, copying, or failed picker execution never changes
PWD. Invalid JSON, unexpected keys/actions/types, relative paths, and unavailable
directories are rejected before `cd`.

The wrapper invokes `dirpick --json`. That explicit integration mode emits one
`{"action":"cd","path":"/exact/absolute/path"}` record only for Ctrl-G on a
directory; other exits emit nothing. The interface and clipboard traffic stay on
`/dev/tty`. Direct `dirpick` invocation explains setup on Ctrl-G and keeps the
session open. Redirecting `--json` alone cannot change a parent shell directory.

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
toggles, syntax preview, exact OSC 52 bytes in isolated PTYs, stdout separation,
help, narrow panes, `NO_COLOR`, and restored terminal flags. Content tests cover
literal/regex matching, all source spans, line identity, stale events/actions,
file/entry/row/memory bounds, and missing files. Isolated fake editor processes
in real PTYs prove exact argv, all child streams on the terminal, terminal
release/reacquisition, and unchanged stdout on success and failure.

Test installation without replacing an active command:

```sh
install_root=$(mktemp -d)
cargo install --path . --locked --root "$install_root"
"$install_root/bin/dirpick" --help
```

The accepted visual reference is the native Workbench prototype on local branch
`prototype/search-layouts`, commit `e49d91b`, in `prototypes/terminal`.
Production discovery and previews use the real filesystem, not prototype data.
