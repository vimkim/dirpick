# Dirpick v0.1 release acceptance

This record checks the implementation against [spec #1](https://github.com/vimkim/dirpick/issues/1)
and [release ticket #7](https://github.com/vimkim/dirpick/issues/7).
The implementation candidate is `57c8399ec188aa0edae6ad9a69fd913e5dc096ca` on
`ticket/7-release`, based on integration commit `338b6dd`.
The final evidence commit changes documentation and test formatting only.

Automated acceptance checks passed on Linux x86_64 on 2026-10-16.
The final independent code review is **pending**. The parent workflow will run
that review after integration. Actual clipboard acceptance in the user's terminal
or SSH client also remains **pending**. Automated OSC 52 emission does not prove
client clipboard acceptance.

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
cargo package --list --locked --allow-dirty
```

All 47 Rust integration tests passed, including five real PTY entry points.
The PTY suites use semantic text, exact action bytes, child argv, process exit,
and terminal flags. They do not compare screenshot pixels or private helpers.
The package listing contains the Nu integration, syntax notices, dependency
inventory, source, and verification scripts. No package upload occurred.

The temporary installation used a separate root:

```sh
install_root=$(mktemp -d)
cargo install --path . --locked --offline --root "$install_root" --target-dir target
python3 -B tests/pty_install.py "$install_root/bin/dirpick"
python3 -B tests/pty_workflow.py "$install_root/bin/dirpick"
python3 -B tests/pty_directories.py "$install_root/bin/dirpick"
```

The installed command passed the complete PTY journey and actual Nushell directory
checks at all three sizes. The installation smoke check used an unrelated working
directory and an empty PATH. It rendered embedded Rust syntax, used the CWD fallback
without Git, and restored terminal flags. Its wrapper and notices output matched
the packaged source bytes. No Node, browser, service, external ripgrep, or source
checkout was necessary for runtime data.

## Evidence against the spec

| Spec contract | Automated evidence |
| --- | --- |
| One native application session and standalone runtime | The integration suites drive `SearchSession` with real fixtures. Temporary Cargo installation runs the native command with an empty PATH. |
| Project, launch CWD, Custom scope, and Back | `session.rs` covers repositories, linked worktrees, submodules, unavailable Git, equal presets, invalid paths, and exact native paths. The combined PTY journey retains fields through F7 and Back. |
| Fuzzy ranking, smart case, relative filters, and ignore rules | `session.rs` and `filtering.rs` cover ranking, include/exclude precedence, invalid-pattern recovery, parent ignore rules, independent Hidden/Ignored, Git metadata, and skipped symlinks. |
| Files, Directories, and Contents share settings | `directories.rs`, `contents.rs`, and the combined PTY journey retain fields, scope, Regex, Hidden, and Ignored across mode changes. |
| Browsing and history restore selection and preview | Application tests restore path and line identity, preview position, and prior mode/settings. Vanished selections produce feedback. PTYs browse, alter state, and restore the directory selection and child-preview position. |
| Source and directory previews | `preview_actions.rs` checks syntax context, unknown syntax, unsupported files, control-character fidelity, and bounded scrolling. Directory tests verify immediate children under the active visibility rules. |
| Literal and regex content matching | `contents.rs` checks smart case, every match span, zero-width regexes, grouped line identity, nearby preview context, and unsupported source files. |
| Responsive, bounded, and honest search | Session tests deliver obsolete generations deliberately and verify rejection. Fixtures exercise entry, retained-path, matching-line, file-size, and memory limits. At 70×18, PTYs exercise 5,000 retained lines and the 16 MiB memory guard. PARTIAL and F12 remain visible after a copy action. |
| Exact clipboard transport | Application tests verify exact OSC 52 bytes, failed writes/flushes, unsupported UTF-8 transport, and retained state. PTYs decode file and directory payloads containing Unicode, quotes, whitespace, and newlines. |
| Editor arguments and terminal handoff | `editor.rs` verifies VISUAL/EDITOR precedence, literal arguments, and exact lines. PTYs verify all three child streams use the terminal, normal child modes, exact `+260 -- path`, return, failed spawn, unsupported editor guidance, nonzero exit, and Ctrl-C interruption. |
| Cleanup after a failed child | A child deliberately leaves its terminal raw and exits unsuccessfully. The PTY verifies resumed input, preserved search/preview, clean stdout, and exact restoration of launch terminal flags on final exit. |
| Explicit JSON and Nushell cd | Application and actual Nu tests verify exact absolute paths, including trailing whitespace/newlines, malformed-output rejection, and unchanged PWD after cancellation or failure. Actual Nu PTYs change PWD at 120×32, 80×24, and 70×18. |
| Wide, narrow, resize, keys, and Unicode | PTYs begin at 120×32, 80×24, and 70×18. Combined journeys exercise field focus, reverse Tab, editing, CJK/combining/emoji cursor width, pane switching, and preview scrolling. They also cover repeated resizing, help, history, editor return, copying, and explicit directory exit. |
| Undersize, cancellation, and theme | A 70×18 session resizes to 60×12 and retains usable help/exit. Other PTYs quit with Esc or Ctrl-C after superseding searches. Mocha output includes the expected colors. NO_COLOR emits no explicit RGB/index colors and retains selection markers. Every normal exit checks terminal flags and alternate-screen cleanup. |
| Installation and notices | The installed binary exposes the embedded wrapper, full dependency notice bundle, and syntax data. The locked inventory covers 124 dependency packages and records 131 upstream notice files from 121 packages. |

## Defects corrected during acceptance

Three behavioral regressions first failed at the approved real PTY boundary:

1. Below the supported size, F12 changed help state but the resize screen hid help.
   The undersize screen now shows basic help and working close/exit guidance.
2. At 70 columns, wrapped scan feedback and copy status displaced the help/exit row.
   The footer now reserves three rows and puts PARTIAL first. F12 exposes complete,
   scrollable feedback when the footer clips a long message.
3. A failed editor could leave raw flags that Crossterm captured as its next original modes.
   Dirpick now retains launch terminal modes across child handoffs and final cleanup.

The combined workflows and existing application tests pass with these fixes.
The prototype remains outside the implementation branch. Production searches and
previews use real filesystem data.

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
