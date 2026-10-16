# Dirpick v0.1 release acceptance

This record checks the implementation against [spec #1](https://github.com/vimkim/dirpick/issues/1)
and [release ticket #7](https://github.com/vimkim/dirpick/issues/7).
The review-correction candidate is `4727de1f2f8dd038fef3f1851c6bdc309a069dd3`
on `review/first-release-fixes`, based on reviewed integration commit `aa3c5da`.
The original release acceptance candidate was `1d24a085cdcfe5e2cf3e3cf5103f833ad54bdfed`.
The final evidence commit changes documentation only; its code and tests match
this tested candidate.

Automated acceptance checks passed on Linux x86_64 on 2026-10-16.
Both original independent reviewers verified the final code with **zero remaining
findings**. Their Standards and Spec conclusions are recorded below. Actual
clipboard acceptance in the user's terminal or SSH client remains **pending**.
Automated OSC 52 emission does not prove client clipboard acceptance.

## Environment and commands

The checks used Rust and Cargo 1.97.1, Python 3.14.6, and Nushell 0.115.1.
All editor and terminal tests used isolated PTYs and temporary filesystem fixtures.
The tests did not change the user's installed command or shell configuration.

```sh
cargo fetch --locked
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
python3 -B scripts/dependency_notices.py --check
cargo package --list --locked
```

All 50 Rust integration tests passed, including seven real PTY tests. The original
48 scenarios remain, with two added preview regressions.
The PTY suites use semantic text, exact action bytes, child argv, process exit,
and terminal flags. They do not compare screenshot pixels or private helpers.
The package listing contains the Nu integration, syntax notices, dependency
inventory, source, and verification scripts. No package upload occurred.

The temporary installation used a separate root:

```sh
install_root=$(mktemp -d)
cargo install --path . --locked --offline --root "$install_root" --target-dir target
python3 -B tests/pty_install.py "$install_root/bin/dirpick"
python3 -B tests/pty_preview.py "$install_root/bin/dirpick"
python3 -B tests/pty_preview_unicode.py "$install_root/bin/dirpick"
```

The installed command passed the installation smoke check and both minimum-size
preview regressions. The full Cargo test run also passed the complete PTY journeys
and actual Nushell directory checks at all three sizes. The installation smoke
check used an unrelated working directory and an empty PATH. It rendered embedded
Rust syntax, used the CWD fallback without Git, and restored terminal flags. Its
wrapper and notices output matched the packaged source bytes. No Node, browser,
service, external ripgrep, or source checkout was necessary for runtime data.

## Evidence against the spec

| Spec contract | Automated evidence |
| --- | --- |
| One native application session and standalone runtime | The integration suites drive `SearchSession` with real fixtures. Temporary Cargo installation runs the native command with an empty PATH. |
| Project, launch CWD, Custom scope, and Back | `session.rs` covers repositories, linked worktrees, submodules, unavailable Git, equal presets, invalid paths, and exact native paths. The combined PTY journey retains fields through F7 and Back. |
| Fuzzy ranking, smart case, relative filters, and ignore rules | `session.rs` and `filtering.rs` cover ranking, include/exclude precedence, invalid-pattern recovery, parent ignore rules, independent Hidden/Ignored, Git metadata, and skipped symlinks. |
| Files, Directories, and Contents share settings | `directories.rs`, `contents.rs`, and the combined PTY journey retain fields, scope, Regex, Hidden, and Ignored across mode changes. |
| Browsing and history restore selection and preview | Application tests restore path and line identity, preview position, and prior mode/settings. Vanished selections produce feedback. PTYs browse, alter state, and restore the directory selection and child-preview position. |
| Source and directory previews | `preview_actions.rs` checks syntax context, unknown syntax, unsupported files, control-character fidelity, and bounded scrolling. Session tests center a content match within the reported viewport and keep it visible when the byte budget reduces available context. Manual scrolling and its history survive resizing. Directory tests verify immediate children under the active visibility rules. Minimum-size PTYs reach all 50 source lines and all 50 immediate children by paging. |
| Literal and regex content matching | `contents.rs` checks smart case, every match span, zero-width regexes, grouped line identity, nearby preview context, and unsupported source files. |
| Responsive, bounded, and honest search | Session tests deliver obsolete generations deliberately and verify rejection. Fixtures exercise entry, retained-path, matching-line, file-size, and memory limits. At 70×18, PTYs exercise 5,000 retained lines and the 16 MiB memory guard. PARTIAL and F12 remain visible after a copy action. |
| Exact clipboard transport | Application tests verify exact OSC 52 bytes, failed writes/flushes, unsupported UTF-8 transport, and retained state. PTYs decode file and directory payloads containing Unicode, quotes, whitespace, and newlines. |
| Editor arguments and terminal handoff | `editor.rs` verifies VISUAL/EDITOR precedence, literal arguments, and exact lines. PTYs verify all three child streams use the terminal, normal child modes, exact `+260 -- path`, return, failed spawn, unsupported editor guidance, nonzero exit, and Ctrl-C interruption. |
| Cleanup after a failed child | A child deliberately leaves its terminal raw and exits unsuccessfully. The PTY verifies resumed input, preserved search/preview, clean stdout, and exact restoration of launch terminal flags on final exit. |
| Explicit JSON and Nushell cd | Application and actual Nu tests verify exact absolute paths, including trailing whitespace/newlines, malformed-output rejection, and unchanged PWD after cancellation or failure. Actual Nu PTYs change PWD at 120×32, 80×24, and 70×18. |
| Wide, narrow, resize, keys, and Unicode | PTYs begin at 120×32, 80×24, and 70×18. Combined journeys exercise field focus, reverse Tab, editing, CJK/combining/emoji cursor width, pane switching, and preview scrolling. Emitted-source checks prove joined emoji retain following text, control escapes, combining marks, and component-match highlighting. They also cover repeated resizing, help, history, editor return, copying, and explicit directory exit. |
| Undersize, cancellation, and theme | A 70×18 session resizes to 60×12 and retains usable help/exit. Other PTYs quit with Esc or Ctrl-C after superseding searches. Mocha output includes the expected colors. NO_COLOR emits no explicit RGB/index colors and retains selection markers. Every normal exit checks terminal flags and alternate-screen cleanup. |
| Installation and notices | The installed binary exposes the embedded wrapper, full dependency notice bundle, and syntax data. The locked inventory covers 124 dependency packages and records 131 upstream notice files from 121 packages. |

## Defects corrected during acceptance

Five behavioral regressions first failed at the approved real PTY boundary:

1. Below the supported size, F12 changed help state but the resize screen hid help.
   The undersize screen now shows basic help and working close/exit guidance.
2. At 70 columns, wrapped scan feedback and copy status displaced the help/exit row.
   The footer now reserves three rows and puts PARTIAL first. F12 exposes complete,
   scrollable feedback when the footer clips a long message.
3. A failed editor could leave raw flags that Crossterm captured as its next original modes.
   Dirpick now retains launch terminal modes across child handoffs and final cleanup.
4. A long field containing a joined emoji could hide the text next to its cursor.
   Horizontal scrolling now measures whole graphemes with their terminal display widths.
5. Fixed preview context could hide the selected content line in a short pane.
   The session now centers selections using the visible row count. It retains
   independent scrolling and restores that behavior through history. A bounded
   window with insufficient context begins at the selected line.

The combined workflows and existing application tests pass with these fixes.
The prototype remains outside the implementation branch. Production searches and
previews use real filesystem data.

## Final-review correction candidate

The review at `aa3c5da` identified two behavior bugs and three structural smells.
The two behavior fixes first failed at the approved real PTY seam, then passed:

1. At 70×18, PageDown jumped from source lines 1–5 to 11–15. Paging now uses
   the visible rows with one row of overlap, advancing 1–5 → 5–9 → 9–13.
   The same regression traverses all 50 immediate directory children. Short
   byte-limited windows advance only past displayed rows. The footer reports
   the visible range, while manual scrolling, selection following, history,
   resizing, and editor return retain their existing application coverage.
2. Ten joined family emoji followed by `TAIL` lost `TAIL` and emitted part of
   the eighth family despite the text fitting within the preview. Source clipping
   now measures and emits complete graphemes. A PTY inspects actual terminal
   bytes for the complete text, combining marks, escaped controls, and matching
   inside a composed glyph. Session evidence retains exact raw component-match
   ranges `7..11` and `32..36`; display clipping does not change stored ranges.

The three structural corrections reuse the green application and terminal coverage:

- Every PTY suite uses the same `Picker` lifecycle and `Screen`, preserving
  combining marks, stdout separation, semantic waits, exit status, alternate
  screen cleanup, and terminal flags. A filter fixture uses an equivalent glob
  so its preview cannot be mistaken for an absent result filename.
- One search traversal owns cancellation, first-error reporting, root skipping,
  visited-entry accounting, the entry cap, relative paths, and eligibility.
  Path ranking/retention and content matching/row-memory bounds retain their
  separate policies and streaming behavior.
- `SearchSettings` moves together through observable state, history, and worker
  requests. Preset identity and selection/preview history remain explicit.
  Regex is retained across modes and effective only in Contents. The pre-release
  application interface now exposes these inputs through `State.settings`.

The direct `unicode-segmentation` dependency is pinned to the already-locked
1.13.3 package. No package or upstream notice was added; the unchanged inventory
and notice bundle passed the generator's check. Strict Clippy required the
shared traversal counter's equivalent `is_multiple_of(128)` expression.

The final code candidate passed format, strict all-target Clippy, all 50 tests,
release build, locked fetch, notice consistency, package listing, isolated Cargo
installation, and the installed smoke/preview checks shown above. No full suite
was repeated for this documentation-only evidence update. Actual client clipboard
acceptance remains manual.

## Final independent review

The original Standards and Spec reviewers rechecked all five findings and
explicitly approved final code `4727de1f2f8dd038fef3f1851c6bdc309a069dd3`.
The final code's only amendment after their first recheck was the equivalent
Clippy expression described above; both reviewers explicitly carried their
verification to that exact commit. The reviewed revisions are:

| Review point | Commit |
| --- | --- |
| Original main baseline | `5d0abf7d5564279bef4fdb7ef8db76ae4637516f` |
| Initial reviewed integration candidate | `aa3c5da18243e856280b877e444e474e9f028815` |
| Final independently verified and fully tested code | `4727de1f2f8dd038fef3f1851c6bdc309a069dd3` |

The documentation-only commit containing this record is the evidence tip of
`review/first-release-fixes`; its code and tests are unchanged from the final
verified code above. No main merge, push, registry publication, or active-user
installation was performed as part of these corrections.

### Standards

The initial review found **zero hard breaches of documented standards** and
three judgment-call maintenance findings. All three are resolved:

- The divergent PTY `Screen` implementations and lifecycle code now share
  `pty_support.py`. Smoke, filtering, directories, editor handoffs, and release
  workflows retain semantic assertions and terminal/stdout cleanup checks.
- `SearchTraversal` centralizes eligibility and accounting while mode-specific
  matching, retention, caps, pruning, statistics, cancellation, first-error
  reporting, and streaming selection remain covered by application scenarios.
- One `SearchSettings` value crosses state, history, and worker requests. Logical
  preset identity, selection/preview history, and inactive retained Regex behavior
  remain explicit without a compatibility layer.

The original Standards reviewer independently verified these corrections against
the final code. **Zero remaining Standards findings.**

### Spec

The initial review found two **P2** behavior bugs. Both are resolved:

- At the supported 70×18 minimum, preview paging now exposes every intermediate
  source line and directory child. The real PTY regression visits all 50 rows
  of each fixture; existing history, resize, selection, and editor-return checks
  continue to pass.
- Source clipping measures complete graphemes. The emitted-text PTY regression
  retains all ten family emoji and `TAIL`, combining marks, control escaping,
  and highlighting inside a glyph. Application evidence retains original raw
  UTF-8 match ranges, including component matches within composed emoji.

The original Spec reviewer independently verified these corrections against
the final code, with no other omissions or scope expansion. **Zero remaining
Spec findings.** Actual client clipboard acceptance remains the manual check
described below.

## Notice scope and remaining limits

The [dependency inventory](../dependency-notices.json) includes development and
platform dependencies beyond the supported Linux build. The archives for `r-efi`
and the two `winapi-*-pc-windows-gnu` packages omit root license text. The bundle
identifies those packages and their declared licenses. Linux does not build them.
The generator requires review if another dependency lacks upstream notice files.

The selected upstream license texts remain verbatim, including original line
endings and whitespace. The Git attributes exception applies only to the generated
notice bundle. The existing Sublime syntax notices remain in
[the syntax notice source](../../notices/syntax.txt). Dirpick itself has no selected
project license. The work did not invent one or publish a registry package.

Linux and SSH are the first-release support target. Search, content, preview,
symlink, and UTF-8 transport limits remain those in the [README](../../README.md).
Generic editors cannot receive content locations without supported line syntax.
Abrupt termination that prevents Rust cleanup remains outside cleanup guarantees.

For the remaining manual clipboard check, run the command in the user's actual
terminal or SSH client. Select a known path and press Ctrl-Y. Paste into a local
scratch buffer and compare the original path, including whitespace and Unicode.
Record the terminal/client and result separately from the automated evidence.
