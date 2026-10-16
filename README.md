# Dirpick

Dirpick is a native terminal workbench for finding files and text, previewing source, and opening an editor.
It runs on Linux terminals and over SSH, without Node, a browser, a service, or
an external ripgrep command. Git is optional and only resolves the Project root.

## Run

```sh
cargo run --locked
```

Launch inside a Git working tree to search its Project root, including linked
worktrees and submodules. Outside Git, Project uses the launch directory.
The visible Project/CWD preset remains distinct when both paths are equal.

Press `/` and type whitespace-separated fuzzy path fragments in Query. Uppercase
characters make the query case-sensitive. Exact basenames rank before basename prefixes,
then other fuzzy matches. An empty query lists files in relative-path order.

Include and Exclude are regex filters over slash-separated paths relative to the
active scope. Each filter uses smart case independently of Query. Empty filters
leave paths unconstrained. Exclude wins and prunes matching directories. Include
never prevents traversal into a directory that could have matching descendants.
An invalid pattern marks its field, explains the error, and clears stale results
until you fix the pattern. Validation also applies to empty queries and directories.

`Space f` selects Files and `Space d` selects Directories. Both use the same fuzzy
path queries, filters, Hidden, and Ignored settings. Directory previews list
immediate children using the active search scope's relative-path filters and visibility rules. Enter
on a directory browses into it and clears only Query. Alt-Left restores the prior
mode, scope, settings, selection, and preview position when available.

`Space c` selects Contents mode: queries match literal text on individual lines by
default. `Space r` toggles Regex. Queries use smart case in both settings. Filters
keep their independent regex and case rules. An empty content query shows guidance
without scanning. Dirpick groups matching lines by file with 1-based line numbers.
Source previews show every matching span and begin near the selected line.
Mode changes keep Query, Include, Exclude, scope, Regex, Hidden, and Ignored.

Enter opens the selected file at line one, or a content result at its exact
line. Dirpick resolves VISUAL, then EDITOR, then nvim, parsing quoted executable
arguments without shell evaluation. Vim and Neovim receive a separate +line
argument before -- and the literal absolute path. Other editors can open file
results. Content locations require vim/nvim and show setup guidance for unsupported
commands. The editor owns /dev/tty with normal terminal modes. After success,
nonzero exit, Ctrl-C interruption, or spawn failure, Dirpick restores the search,
selection, preview position, and TUI.

Directory browsing and Nushell integration follow the
[approved release spec](https://github.com/vimkim/dirpick/issues/1).

## Keys

Dirpick starts in **Normal** mode. Press **i** to edit the focused field, or **/**
to focus and edit Query. In **Insert** mode, type normally, including spaces.
**Esc** returns to Normal. The footer shows a colored **NORMAL** or **INSERT**
badge and the keys available in that mode. Esc also closes the guide or help;
it never quits the application. Use **q** in Normal or **Ctrl-C** to quit.

In Normal, press **Space** to open the shortcut guide, then a listed key to act.
The header always shows Files, Directories, and Contents with their shortcut
letters; the active search mode has a colored badge and brackets. Green marks
Files, blue Directories, and peach Contents. Focused fields have a contrasting
background, search and result borders follow the search mode color, and enabled
traversal toggles light up. An unrecognized key leaves the guide open.

Input mode (Normal or Insert) and search mode (Files, Directories, or Contents)
are independent. Changing fields with Tab keeps the current input mode.

| Mode | Key | Effect |
| --- | --- | --- |
| Normal | i | Enter Insert to edit the focused field |
| Normal | / or Space / | Focus Query and enter Insert |
| Normal | Space | Open the shortcut guide |
| Normal | q or Space q | Quit |
| Normal | j / k | Select the next / previous result |
| Normal | Space f / Space d / Space c | Select Files / Directories / Contents |
| Normal | Space r | Toggle content Regex and retain it when changing search modes |
| Normal | Space s | Switch Project / launch CWD, retaining the query |
| Normal | Space h | Reveal / hide dotfiles and dot directories |
| Normal | Space i | Bypass / respect ignore rules |
| Normal | Space p | Switch Results / Preview below 100 columns |
| Normal | Space ? | Show keyboard help and full feedback |
| Insert | Text, including Space | Edit the focused field |
| Insert | Ctrl-U | Clear the focused field |
| Insert | Left / Right / Home / End | Move the field cursor |
| Either | Esc | Return to Normal, or close the shortcut guide or help |
| Either | Enter | Open the selected file or content line in the configured editor |
| Either | Enter on a directory | Browse inside Dirpick and clear only Query |
| Either | Enter in Scope | Apply a directory and clear only Query |
| Either | Ctrl-G | cd & exit for a directory through the Nushell wrapper |
| Either | Up / Down | Select a result |
| Either | PageUp / PageDown | Scroll the preview independently |
| Either | Ctrl-Y | Send the exact selected absolute path through OSC 52 and keep searching |
| Either | Tab / Shift-Tab | Cycle Query, Include, Exclude, and Scope |
| Either | Ctrl-F / Ctrl-L | Focus Query / Scope and enter Insert |
| Either | Alt-Left | Restore the previous scope, fields, toggles, selection, and preview position |
| Either | Ctrl-C | Quit |

Relative scope paths resolve against the active scope. Invalid paths retain the
active search and show an error. CWD always means the original launch directory.
Scope changes retain Include, Exclude, Hidden, and Ignored settings.
At 100 columns or more, Results and Preview appear beside each other. At 70–99
columns, `Space p` switches between them. Settings remain visible in both layouts.
Resizing keeps the active search, selected result, and manual preview position.
A new content selection centers its matching line within the available preview
rows. Its context adjusts on resize until PageUp/PageDown begins independent
scrolling. Back restores the previous preview position and scrolling behavior.
Preview pages follow the visible height with one overlapping row, so every
source line or directory child remains reachable at 70×18. A shorter byte-limited
window advances only past the rows it displayed. The preview footer shows the
displayed range.
The interface supports terminals of at least 70 columns and 18 rows. Below
that size, Normal-mode `Space ?` opens basic help. Ctrl-C still exits.
Set `NO_COLOR` to use terminal default colors with selection markers and attributes.
Field editing and cursor positioning use Unicode terminal display widths.
Source clipping keeps complete graphemes, including joined emoji, and measures
their composed display width. Syntax and match ranges retain raw UTF-8 offsets;
a match inside a composed grapheme highlights the complete glyph.

The footer reserves separate rows for completion or limits, action feedback,
and key guidance. `PARTIAL` appears first when a search reaches a limit.
Long feedback shows `Space ? details` in Normal or `Esc Space ? details` in Insert.
In help, PageUp/PageDown scroll and Home/End show the first or last page,
including complete error and action messages.
Esc closes help and returns to Normal. Ctrl-C exits even while help is open.

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
Lines over 4 KiB disable syntax parsing from that line onward. Oversized window
lines show a clipped prefix with a limit notice. PageDown can inspect later lines.
If the byte limit leaves insufficient context around a new content selection,
the preview begins at its matching line.

Ctrl-Y writes the selected absolute UTF-8 path to the controlling terminal as an
OSC 52 clipboard payload. The feedback says **payload sent**, because terminal
and SSH client clipboard acceptance remains unknown. Write/flush failures
leave the search and selection intact. Native path values retain spaces, quotes,
Unicode, trailing whitespace, and newlines. Clipboard payloads contain the original
path, including characters that require visible escapes in the interface.
Paths outside the UTF-8 transport contract show a limitation instead of a lossy
substitution. Your terminal must support and permit OSC 52 clipboard writes.

Dependency and syntax-data notices ship in
`THIRD_PARTY_NOTICES.txt` and in the installed command via
`dirpick --third-party-notices`. These third-party notices do not select a
license for Dirpick itself. The [dependency inventory](docs/dependency-notices.json)
records versions, upstream licenses, selected notice files, and hashes. It
includes development and platform crates beyond those built for Linux.

Dirpick uses `/dev/tty` for its interface and leaves stdout free of terminal output.
Normal exit, input errors, failed terminal setup, and unwinding restore terminal
modes and the alternate screen where cleanup remains possible. Dirpick retains
the original terminal modes even if a failed editor leaves the terminal raw.

## Nushell setup

Run `just sync` to build and install this checkout into `~/.local/bin`, then check
the installed command's version. The recipe uses the locked dependencies and
finds Cargo on PATH or in `~/.cargo/bin`. Add `~/.local/bin` to PATH if needed.
It installs the current sources without fetching Git changes or editing shell
configuration. Without Just, run
`cargo install --locked --path . --root "$HOME/.local" --bin dirpick`.

Generate the supplied wrapper and load it in Nushell:

```nu
dirpick --print-nu-wrapper | save -f ~/.config/nushell/dirpick.nu
use ~/.config/nushell/dirpick.nu dp
dp
```

To keep `dp` available in future shells, add that `use` line to your Nu config
yourself. Dirpick does not edit shell configuration. The binary embeds the wrapper,
so the wrapper remains available after temporary Cargo installation without the
source checkout or a separate data installation.

Within `dp`, Normal-mode `Space d` searches directories, Enter browses without changing the
shell, and Ctrl-G changes the invoking Nu directory and exits. The wrapper uses
ordinary `cd`, preserving Nu's normal directory behavior. It passes decoded paths as data.
The appended separator preserves names ending in spaces, tabs, or newlines.
Cancellation, plain quitting, copying, or failed picker execution never changes
PWD. The wrapper rejects invalid JSON, unexpected keys/actions/types, relative
paths, and unavailable directories before `cd`.

The wrapper invokes `dirpick --json`. That explicit integration mode emits one
`{"action":"cd","path":"/exact/absolute/path"}` record only for Ctrl-G on a
directory. Other exits emit nothing. The interface and clipboard traffic stay on
`/dev/tty`. Direct `dirpick` invocation explains setup on Ctrl-G and keeps the
session open. Redirecting `--json` alone cannot change a parent shell directory.

## Check and build

```sh
cargo fetch --locked
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
python3 -B scripts/dependency_notices.py --check
```

Tests use real filesystem and Git fixtures at the application session boundary.
The Linux PTY checks require Python 3. The Nu integration checks require Nushell.
PTY checks cover real input, path-filter errors, independent traversal toggles,
and syntax preview. They also cover exact OSC 52 bytes, stdout separation,
help, narrow panes, `NO_COLOR`, and restored terminal flags. Content tests cover
literal/regex matching, all source spans, line identity, stale events/actions,
file/entry/row/memory bounds, and missing files. Isolated fake editor processes
in real PTYs prove exact argv, all child streams on the terminal, terminal
release/reacquisition, and unchanged stdout on success and failure.
The release journeys cover all three modes at 120×32, 80×24, and 70×18, including
repeated resizing, field focus, Unicode editing, history, exact directory JSON,
and cancellation. The wrapper also changes an isolated Nu process's actual PWD
at each size. Failed editors include a child that leaves terminal modes raw.
Minimum-size PTYs page through all 50 source lines and all 50 directory children.
They also inspect emitted joined emoji, following text, combining marks, escaped
controls, and highlighting of a match inside a composed glyph.

After dependency changes, run `cargo fetch --locked` and
`python3 -B scripts/dependency_notices.py` to refresh the notice bundle.
The generator uses the locked registry source files and preserves their text.
Review new licenses and any package without upstream notice files before release.

Test installation without replacing an active command:

```sh
install_root=$(mktemp -d)
cargo install --path . --locked --root "$install_root"
"$install_root/bin/dirpick" --help
"$install_root/bin/dirpick" --third-party-notices
"$install_root/bin/dirpick" --print-nu-wrapper
python3 -B tests/pty_install.py "$install_root/bin/dirpick"
```

Automated checks prove OSC 52 emission and exact payload bytes. Clipboard
acceptance in the user's terminal or SSH client remains a manual check. Select
a path with Ctrl-Y, then paste into a local scratch buffer and compare the path.
The [release acceptance record](docs/reviews/first-release-acceptance.md) separates
automated evidence, pending independent review, and this manual check.

The accepted visual reference is the native Workbench prototype on local branch
`prototype/search-layouts`, commit `e49d91b`, in `prototypes/terminal`.
Production discovery and previews use the real filesystem, not prototype data.
