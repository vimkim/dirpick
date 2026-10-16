# Dirpick v0.1: spec and ticket publication review

This is a review draft, not the issue tracker. GitHub Issues in `vimkim/dirpick`
will hold the approved spec and implementation tickets. Overall progress stays
in work-tracker item 403. No production code has been built yet.

The product choices below are accepted. Concrete engineering defaults in this
draft are proposals for implementing those choices. Approval covers the test
boundary, these defaults, and the six-ticket breakdown. Local integration and
pushing remain separate from publishing these issues.

## Review at a glance

- Preferred UI: native Workbench (B), Catppuccin Mocha, visible search controls.
- Scope: Project by default; F7 switches to the immutable launch CWD.
- Modes: Files, Directories, and Contents, with shared query and filters.
- Traversal: respect ignore files, hidden off, no recursive symlink following;
  separate Hidden and Ignored toggles; Git metadata stays excluded.
- Actions: syntax preview, absolute-path OSC 52 copy, editor at the match line,
  browse/back, and Nushell cd & exit.
- Main test boundary: drive a search session through its application interface
  against real temporary directories and Git working trees. Observe results,
  preview data, state restoration, and requested actions.
- OS checks: a small PTY suite plus isolated editor and Nushell adapter checks.
  Avoid tests tied to private functions, widget structure, or screenshot pixels.

| Ticket | Delivers | Blocked by |
| --- | --- | --- |
| T1 | Find and preview real files from Project or launch CWD | None |
| T2 | Refine file searches with path filters and traversal toggles | T1 |
| T3 | Inspect syntax previews and copy exact absolute paths | T1 |
| T4 | Browse directories and change the Nushell directory | T2 |
| T5 | Search file contents and return from an editor at the match line | T2, T3 |
| T6 | Use and install the complete workflow in wide and narrow terminals | T3, T4, T5 |

T2 and T3 can proceed independently after T1. T4 can proceed alongside T5
when their respective blockers are complete. T6 verifies the complete release;
responsiveness, terminal cleanup, and relevant tests belong in every slice.

## Draft spec issue: Dirpick v0.1 search, preview, and navigation

### Problem Statement

The user needs one terminal workflow for finding files, directories, and text,
refining path filters, inspecting source, and acting on the selection. Existing
tools tried by the user do not make this combination convenient. Controls must
remain visible and easy to change while results and previews update.

### Solution

Build the standalone Rust command Dirpick as a native terminal workbench. Keep
mode, scope, query, include, exclude, and traversal settings visible above
results and preview. Use Catppuccin Mocha and group content matches by file.
Start with recursive Files search at the enclosing Git working-tree root and
allow a quick switch to the launch directory. Support browsing, copying an
absolute path through OSC 52, opening an editor at the selected source line,
and changing the invoking Nushell directory through an explicit shell action.

### User Stories

1. As a terminal user, I want a native keyboard interface, so that navigation
   feels natural over SSH.
2. As a user, I want the Workbench layout and Mocha colors, so that modes,
   focus, selection, matches, and source syntax are easy to distinguish.
3. As a user, I want search settings to remain visible, so that I can understand
   the current results without reopening a menu.
4. As a developer, I want Project scope selected at startup, so that launching
   from a nested directory searches my whole Git working tree.
5. As a Git worktree user, I want the root of the current working tree, so that
   linked worktrees and submodules search the appropriate checkout.
6. As a user outside Git, I want startup to use my launch directory, so that
   Dirpick remains useful in ordinary directories.
7. As a user, I want a Project/CWD toggle, so that I can change search breadth
   without typing a path.
8. As a user, I want CWD to keep meaning the launch directory, so that browsing
   does not silently move that preset.
9. As a user, I want scope switching to preserve the query and filters, so that
   I can compare the same search at both roots.
10. As a user, I want Files mode to match relative paths fuzzily, so that I can
    locate a file from a few path fragments.
11. As a user, I want Directories mode to find directory paths, so that I can
    browse or select a destination without file results getting in the way.
12. As a user, I want empty path queries to enumerate recursive paths, so that
    I can explore before deciding what to type.
13. As a developer, I want literal content search by default, so that punctuation
    is treated as ordinary text.
14. As a developer, I want a visible Regex toggle for content queries, so that
    patterns are available when I choose them.
15. As a user, I want smart-case matching, so that uppercase text can narrow a
    search without a separate case switch.
16. As a user, I want include and exclude regexes over relative paths in every
    mode, so that the same constraints work across files, directories, and text.
17. As a user, I want exclusion to win, so that an excluded subtree remains
    excluded even when it also matches the include pattern.
18. As a user, I want invalid patterns shown beside the search, so that I can
    fix them without leaving Dirpick or confusing stale results with new ones.
19. As a developer, I want ignore files respected, so that generated and ignored
    paths do not dominate ordinary searches.
20. As a user, I want separate Hidden and Ignored controls, so that I can reveal
    one category without implicitly revealing the other.
21. As a user, I want Git metadata excluded and symlinks left untraversed, so
    that ordinary searches stay focused and avoid recursive link cycles.
22. As a user, I want the query and filters preserved across mode changes, so
    that changing the candidate type does not discard my work.
23. As a developer, I want grouped content results with file names and line
    numbers, so that I can understand matches before opening a file.
24. As a developer, I want a syntax preview with line numbers and match context,
    so that I can judge the selected result within the terminal.
25. As a user, I want preview scrolling independent of result selection, so
    that I can inspect nearby source without losing my match.
26. As a user, I want a directory preview to show immediate children, so that
    I can inspect a destination before browsing or changing the shell directory.
27. As a user, I want Enter on a directory to browse within Dirpick, so that I
    can explore without changing my shell yet.
28. As a user, I want Back to restore the previous scope and search state, so
    that exploration is reversible.
29. As a user, I want to copy the exact selected absolute path to my local
    clipboard through OSC 52, so that remote selections are useful locally.
30. As a developer, I want Enter on a file or content match to open my editor,
    so that I can act on the result immediately.
31. As a developer, I want content results opened at the matching line and the
    search restored after the editor exits, so that I can inspect several hits.
32. As a Nushell user, I want a separate cd & exit action, so that choosing a
    directory changes my invoking shell through its normal directory behavior.
33. As a user, I want spaces, quotes, Unicode, leading dashes, trailing
    whitespace, and newlines in paths preserved, so that actions reach the exact
    selection without shell interpretation.
34. As a user, I want searches to remain responsive and obsolete work discarded,
    so that typing or switching scope never revives an old result set.
35. As a user, I want scanning, errors, and truncation shown clearly, so that I
    can distinguish complete results from a bounded or interrupted search.
36. As a user in a narrow terminal, I want to switch Results and Preview panes
    while keeping settings visible, so that the workflow remains usable.
37. As a user, I want help and terminal restoration on exit or failed actions,
    so that I can discover keys and safely return to my shell.
38. As a developer, I want a documented Cargo installation and Nushell setup,
    so that I can run the real application without the prototype checkout.

### Implementation Decisions

The following are the proposed engineering defaults for this spec. They do not
claim that the prototype implements production filesystem or process behavior.

1. **Application boundary.** Use one search-session interface for user commands,
   observable search/preview state, and requested external actions. Keep terminal
   rendering and OS effects thin. The session owns scope history, active search
   generation, selection identity, and preview position. Keep filesystem search
   and matching behind this interface rather than exposing helper-level APIs
   solely for tests.
2. **Runtime.** Use Rust with Ratatui and Crossterm, carrying the selected native
   layout into production code. Use Rust libraries for ignore-aware traversal,
   matching, and syntax previews. Avoid requiring Node, a browser, a running
   service, or a separate ripgrep executable at runtime. Lock dependencies and
   package required syntax/theme data and notices with the application.
3. **Scope roots.** Capture the absolute launch CWD once. Discover the nearest
   enclosing Git working-tree top, including linked worktrees and submodules;
   never use the Git metadata directory as the search root. Git discovery may
   use the Git command with literal arguments. If Git is unavailable or no
   working tree is found, Project uses launch CWD. Retain a logical active preset
   even when both roots happen to be equal. Show the resolved path and fallback.
4. **Scope changes.** F7 switches Project/CWD without clearing fields. From a
   Custom scope, F7 returns to Project. Enter on a directory or a valid manually
   edited scope applies that directory and clears only the query. Manual relative
   scope paths resolve against the active scope. Invalid paths leave the active
   search unchanged and show an error. Save the previous session state for Back.
   A browsed or edited path equal to a preset may show that preset; other paths
   show Custom. Explicit browsing may move outside the initial roots.
5. **Queries.** Files and Directories match each whitespace-separated query
   token as an ordered subsequence of the relative path. All tokens must match.
   Prefer exact basename matches, then basename prefixes, then other fuzzy
   matches; break ties deterministically by relative path. Empty path queries
   enumerate eligible descendants in relative-path order. Contents uses literal
   line matching unless Regex is enabled. Report one result per matching line,
   with every match span on that line. An empty Contents query starts no scan.
6. **Case and filters.** Queries and each include/exclude pattern use smart
   case independently: uppercase makes that expression case-sensitive. Filters
   always use regex syntax against slash-separated relative paths, independent
   of the Contents Regex setting. Validate filters even for an empty query or
   empty tree. Exclude wins. A nonmatching directory include does not prune
   potentially matching descendants. A directory excluded by a path filter
   excludes its descendants.
7. **Traversal.** Apply Git and ordinary ignore rules, including parent rules
   relevant to nested CWD scopes, and recognize ripgrep ignore files. Hidden and
   Ignored are independent session settings. Enabling Ignored bypasses ignore
   rules; enabling Hidden bypasses dotfile/directory hiding. Both remain subject
   to explicit include/exclude filters. Always exclude entries named `.git`,
   whether metadata is a directory or file. Skip symlink entries during recursive
   candidate enumeration in v0.1. A scope explicitly selected by the user may
   resolve through a symlink, but recursion still skips symlink descendants.
8. **Shared state and history.** Mode changes retain the query, filters, scope,
   Regex, Hidden, and Ignored settings. Regex is inactive outside Contents.
   Recompute results and select the first result after an effective search
   setting changes. Back restores scope, mode, fields, toggles, selected result,
   and preview position when the result still exists. Otherwise select a valid
   remaining result and explain any unavailable target. While one search streams
   more results, keep the selected result's identity rather than its old row index.
9. **Preview.** Render plain source with line numbers, syntax colors, and query
   highlighting; fall back to plain text for unknown syntax. Center content
   previews near the selected line. Preview immediate child names for a selected
   directory using current traversal visibility. Escape control characters for
   display while preserving the underlying path for actions. Unreadable,
   vanished, binary, or oversized files show a useful preview status.
10. **Responsive bounded work.** Run filesystem work away from terminal input.
    Give each effective search a generation, cancel superseded generations, and
    reject late events from them. Bound queues and retained results. Initial
    defaults: at most 50,000 visited entries per search, the best 2,000 path
    results from the scanned candidates or the first 5,000 matching lines, and
    a 2 MiB content-file size limit. Oversized files are skipped for content
    search. Reaching the path-result retention cap does not stop scanning before
    the entry cap, so later better-ranked paths can replace retained results.
    Preview windows contain at most 200 lines and 64 KiB and can scroll through an
    otherwise supported file. Show active scanning and any limit reached; never
    present partial results as an exhaustive search. Binary or invalid UTF-8
    content is skipped; count skipped and failed entries without aborting the
    whole scan. Path discovery remains available for such regular files.
11. **Clipboard.** Ctrl-Y sends the exact absolute selected path as an OSC 52
    clipboard payload on the terminal channel, leaves the search open, and
    reports that the payload was sent. Client clipboard acceptance cannot be
    inferred from a successful write. A write failure leaves the session usable.
    Shell quoting and relative-path copy formats are outside v0.1.
12. **Editor.** Resolve the editor from VISUAL, then EDITOR, then Neovim. Support
    literal executable arguments without shell evaluation. Neovim/Vim receive
    the selected 1-based line as a separate argument; file results use line one.
    Attach the child to the terminal channel so its output cannot contaminate
    a JSON directory-result stream. Generic editor commands may open file results,
    but unsupported line syntax must be reported for content actions rather than
    silently opening the wrong line. Release raw mode and the alternate screen before spawning the child,
    then reinitialize/redraw after success or failure. Keep the same query,
    filters, scope, selected result, and preview position when possible.
13. **Nushell protocol.** Provide an explicit JSON result mode and a supplied
    Nushell wrapper. Only cd & exit returns a directory action: a JSON object
    with an action string equal to cd and an exact absolute path string. Keep
    terminal output on a separate terminal channel. Cancellation, plain quitting,
    file opening, and copy produce no directory action. The wrapper validates
    successful output and invokes ordinary Nushell cd using the path as data,
    including paths with trailing whitespace or newlines. Reject malformed
    records without changing directories. Without JSON/shell integration mode,
    cd & exit shows setup guidance and retains the session. Supply setup
    instructions; do not automatically modify the user's shell configuration.
14. **Path fidelity.** Keep native path values through discovery and actions.
    Arguments are passed directly, not evaluated by a shell. Support valid
    UTF-8 paths with spaces, quotes, Unicode, leading dashes, trailing whitespace,
    and embedded newlines. If a path cannot be represented by the v0.1 JSON or
    OSC 52 text contract, report that limitation and retain the selection instead
    of substituting a different path. JSON escaping is transport, not a change
    to the underlying path.
15. **Keys.** F1/F2/F3 choose Files/Directories/Contents; F4 toggles content Regex;
    F7 switches Project/CWD; F8/F9 toggle Hidden/Ignored; F10 switches the active
    Results/Preview pane on narrow terminals; F12 shows help. Tab/Shift-Tab move
    through Query, Include, Exclude, and Scope. Up/Down select results; Enter
    applies scope, browses, or opens; Alt-Left goes Back. Ctrl-F/Ctrl-L focus
    Query/Scope; Ctrl-U clears the focused field; Ctrl-Y copies; Ctrl-G performs
    cd & exit for a directory result. PageUp/PageDown scroll preview. Esc closes
    help first and otherwise quits; Ctrl-C always quits. Prototype-only layout
    and scenario cycling do not become production commands.
16. **Wide and narrow layouts.** At 100 or more columns, use side-by-side results
    and preview. At 70–99 columns, show one main pane at a time, initially
    Results; retain the selected result when switching panes. Both layouts keep
    current mode, scope, query, filters, and toggles visible. Support 70x18 and
    larger terminals; below that, show resize guidance and retain working exit
    keys. Render and edit Unicode by terminal display width.
17. **Theme and cleanup.** Use Mocha throughout source, borders, labels, focus,
    selection, match highlights, and errors. Respect NO_COLOR while keeping
    focus and selection distinguishable through markers and attributes. Restore
    terminal flags and the alternate screen on normal exit, interrupted work,
    failed startup, and unwinding failures that Rust can clean up.
18. **Delivery.** Deliver a Cargo-installable command and packaged Nushell
    integration with documentation. First-release support targets the user's
    Linux/SSH environment. Include reproducible format, lint, test, and release
    build commands. Test installation in a temporary location, preserving the
    user's active commands and shell configuration.

### Testing Decisions

- Good tests assert user-observable results, state transitions, action payloads,
  and terminal/process behavior. They remain valid if internal functions,
  workers, or rendering widgets are reorganized.
- The main seam is the search-session application interface with real temporary
  filesystem fixtures. Exercise Files, Directories, and Contents through that
  interface instead of mocking traversal or retesting every helper separately.
- Fixtures cover ordinary Git repositories, linked worktrees, nested launch
  directories, a non-Git directory, and unavailable Git discovery. Verify both
  preset labels when Project and CWD resolve to the same directory.
- Table-driven scenarios cover fuzzy ranking classes, literal/regex queries,
  smart case, relative-path filters, exclude precedence, invalid regex recovery,
  ignore inheritance, hidden/ignored independence, Git metadata, and symlinks.
- Drive browsing, preset toggling, mode changes, Back, selection, and preview
  scrolling through commands; assert the preserved state and exact paths.
- Use controlled search-generation events to deliver obsolete results after a
  replacement search, avoiding timing sleeps. Assert stale results and actions
  never become visible. Use bounded fixtures to prove limits and partial status.
- Capture the action boundary for exact OSC 52 payload bytes and editor argv.
  Add isolated real child-process tests for editor failures and successful
  return. Exercise terminal release/reacquisition in an actual PTY, not solely
  with a fake renderer.
- Run the supplied Nushell adapter in an isolated Nu process. Verify actual PWD
  after a directory action and unchanged PWD after cancellation, missing output,
  invalid output, or failed picker execution. Include whitespace/newline paths.
- A small PTY suite drives representative complete workflows and resizing at
  120x32, 80x24, and the minimum size. Check focus, input, selection, pane
  switching, help, NO_COLOR, process exit, and restoration of terminal flags.
  Use semantic assertions rather than a complete screenshot baseline.
- The native prototype's manual PTY captures are prior art for interaction
  checks, not proof of production behavior. Docserve's Finder tests are prior
  art for edge cases; its web API and browser tests are not Dirpick interfaces.
- Finish with a temporary Cargo installation and documented Nushell setup
  smoke check. OSC 52 emission is automatable; successful delivery to the user's
  actual client clipboard remains an explicit manual acceptance check.

### Out of Scope

- Production versions of prototype layouts A and C or a browser UI.
- Bash/Zsh adapters, Windows guarantees, file editing inside Dirpick, or bulk
  filesystem modification.
- Persistent search history, session databases, a daemon/indexing service,
  configurable themes, or user-defined keymaps.
- Following symlinks during recursive search, binary content search, huge-file
  content search beyond the stated bound, or arbitrary non-UTF-8 action encoding.
- Shell evaluation of editor commands or selected paths; automatic changes to
  the user's terminal, editor, or shell configuration.
- Reusing Docserve's server/API or importing prototype fixtures as production
  search data.
- Uploading to a package registry, publishing release binaries, or automatically
  installing into the user's active environment.

### Further Notes

The accepted primary visual reference is the real Rust TUI on local branch
`prototype/search-layouts`, commit `e49d91b`, at
`/home/vimkim/gh/dirpick-prototype/prototypes/terminal`. Its run notes and actual
PTY screenshots are in that checkout's prototype documentation. Retain this
prototype off main; link this branch, commit, and entry point from the published
spec and relevant implementation tickets. The branch is local and has not been
pushed. The earlier browser study is historical evidence, not the UI target.

The public issue tracker is `vimkim/dirpick`. The source checkout begins as
an empty bootstrap, with design documents on `design/first-release`; there is
no existing production API, test suite, package, or dependency lockfile to extend.
Implement from the agreed contracts rather than merging the throwaway prototype.

Engineering references checked while drafting:

- Ignore-aware traversal can configure hidden filtering and symlink following;
  ripgrep ignore files require adding the custom ignore filename. See the
  [ignore walker documentation](https://docs.rs/ignore/latest/ignore/struct.WalkBuilder.html).
- The terminal framework has a recipe demonstrating editor spawn and return.
  See [Ratatui's editor recipe](https://ratatui.rs/recipes/apps/spawn-vim/).
- A Nushell wrapper that changes the caller's directory must preserve its
  environment changes. See [Nushell's custom-command environment documentation](https://www.nushell.sh/book/custom_commands.html#changing-directories-cd-in-a-custom-command).

## Draft implementation tickets

Each ticket will become a separate GitHub issue with the spec as its parent
and the ready-for-agent label. Resolve T-number references to actual issue
numbers when publishing; create native sub-issue and blocking relationships
where supported. These drafts contain no production module file paths.

### T1: Find and preview real files from Project or launch CWD

**What to build:** A runnable native Dirpick command that searches an actual
temporary or user repository in Files mode. Start in the preferred Mocha
Workbench with an editable query, real results, a plain source preview, and
the visible Project/CWD control. Establish the application test boundary through
this complete workflow rather than a standalone infrastructure ticket.

**Blocked by:** None (can start immediately).

**Acceptance criteria:**

- [ ] Cargo builds and runs the real command without a browser, Node, or fixture
  dataset. Normal exit restores terminal settings.
- [ ] Launching beneath a Git working tree selects its root, including a linked
  worktree/submodule; launching outside Git or without Git falls back to launch
  CWD. Equal preset roots retain meaningful selected-preset state.
- [ ] F7 changes scope while retaining query/filter settings and the immutable
  launch CWD; Custom path entry, invalid-path feedback, and Back work as specified.
- [ ] Empty queries enumerate real regular files; fuzzy multi-token smart-case
  matching and deterministic ranking operate on scope-relative paths.
- [ ] Results, selected path, plain preview, query focus, and current scope are
  visible in Workbench. No synthetic editor/clipboard/cd actions are advertised.
- [ ] Work runs outside input handling, uses bounded results and generation
  identity, and exposes scanning/partial/error status.
- [ ] Application-level real-directory tests and a focused PTY startup/input/
  exit check prove the workflow and terminal cleanup.

**Prototype evidence:** Workbench B and Project/CWD interaction on local branch
`prototype/search-layouts`, commit `e49d91b`, in the native terminal study. Use
the primary reference identified by the parent spec; implement production code.

### T2: Refine file searches with path filters and traversal toggles

**What to build:** Starting from a working real-file search, narrow results
with visible include/exclude regex fields and independently reveal hidden or
ignored entries. This is a complete Files search/refinement workflow, with the
shared traversal contract available to later modes.

**Blocked by:** T1.

**Acceptance criteria:**

- [ ] Tab/Shift-Tab reaches query, include, exclude, and scope; filters match
  slash-separated relative paths using independent smart case.
- [ ] Exclude wins; excluded directories exclude descendants; an unmatched
  directory include does not hide matching descendant files.
- [ ] Invalid filters show recoverable feedback even on an empty query/tree;
  stale results cannot be mistaken for a valid new search.
- [ ] Git/ordinary/ripgrep ignore files and parent ignore rules apply in Project,
  nested CWD, and Custom scopes. F8 Hidden and F9 Ignored are visible and
  independent; explicit path filters still apply with either toggle enabled.
- [ ] Git metadata files/directories and recursively encountered symlinks stay
  excluded. Permission failures and vanished paths produce useful scan status.
- [ ] Changing settings cancels/replaces the previous scan while keeping input
  responsive. Search caps show partial status rather than apparent completeness.
- [ ] Real temporary-repository tests cover filter precedence, inheritance,
  hidden/ignored combinations, symlink cycles, metadata, and regex recovery.

### T3: Inspect syntax previews and copy exact absolute paths

**What to build:** Find a real file, inspect a line-numbered syntax preview,
scroll it without losing selection, and copy the selected absolute path through
OSC 52 while remaining in the same search.

**Blocked by:** T1.

**Acceptance criteria:**

- [ ] Selected text files show syntax colors, line numbers, and matching text
  where applicable; unknown syntax falls back to readable plain source.
- [ ] PageUp/PageDown scroll bounded preview windows without changing selection.
  Large, binary, invalid-text, unreadable, and vanished files show clear status.
- [ ] Ctrl-Y sends the exact selected absolute UTF-8 path as an OSC 52 payload
  on the terminal channel and leaves search settings and selection unchanged.
- [ ] Copy feedback distinguishes payload emission from confirmed client
  acceptance; output failures keep the session usable.
- [ ] Spaces, quotes, Unicode, leading dashes, trailing whitespace, and newlines
  survive action encoding. Control characters are escaped only for display.
- [ ] NO_COLOR preserves readable source and distinguishable selection/focus.
- [ ] Application/action tests validate source context and decoded clipboard
  bytes; a focused PTY check verifies actual terminal output and cleanup.

### T4: Browse directories and change the Nushell directory

**What to build:** Search real directories with the same path filters and
traversal settings, inspect immediate children, browse within Dirpick, restore
prior searches with Back, and explicitly cd & exit into a supplied Nushell
wrapper that changes the invoking shell's directory.

**Blocked by:** T2.

**Acceptance criteria:**

- [ ] F2 finds recursive directory paths with fuzzy smart-case queries and the
  accepted shared filters/toggles; empty queries enumerate eligible directories.
- [ ] Directory previews show immediate children under current visibility rules.
- [ ] Enter browses and clears only the query; Back restores scope, settings,
  selection, and preview position. Browsing never changes the launch CWD preset.
- [ ] Ctrl-G on a directory exits with the exact absolute path in a validated
  JSON directory action; ordinary quit/cancel and other actions emit no cd action.
- [ ] Terminal display stays separate from the JSON result stream, and all exit
  paths restore the terminal. Direct invocation explains how shell integration
  enables a parent-shell directory change.
- [ ] The supplied Nushell wrapper performs ordinary cd from validated successful
  output, handles picker errors/cancellation without changing PWD, and preserves
  whitespace/newline paths without evaluation or trimming.
- [ ] Real directory/session tests and an isolated Nushell PWD test verify the
  complete workflow. Setup is documented without editing the user's config.

### T5: Search file contents and return from an editor at the match line

**What to build:** Search real source contents literally or with regex, refine
paths, inspect grouped matches and syntax context, and open the selected line
in the configured editor. Return to the same search after the editor exits.

**Blocked by:** T2, T3.

**Acceptance criteria:**

- [ ] F3 searches text with literal matching by default, F4 Regex, smart case,
  shared relative-path filters, and the same hidden/ignored/symlink policy.
- [ ] Empty content queries show guidance without scanning; invalid queries or
  filters produce recoverable errors. Shared fields survive mode changes.
- [ ] Each matching line is a result grouped by file with a 1-based line number;
  previews highlight all matching spans and start near the selected match.
- [ ] New queries and scope changes cancel old work; delayed obsolete events
  never replace the active result set. Entry/result/file limits and skipped
  content are visible, with input responsive throughout.
- [ ] Enter launches VISUAL/EDITOR/fallback Neovim with literal path arguments;
  Vim/Neovim receive the exact match line. Unsupported editor line syntax shows
  actionable feedback without silently opening a different location.
- [ ] Editor spawn releases the terminal. Successful exit and failed spawn both
  restore the TUI, query, filters, scope, selection, and preview state.
- [ ] Application tests cover literal/regex/case/filter combinations, late
  generations, multiline source positions, missing files, and content bounds.
  An isolated editor process plus PTY check proves argv and terminal handoff.

### T6: Use and install the complete workflow in wide and narrow terminals

**What to build:** Complete the first-release user journey across all modes
at ordinary and narrow terminal sizes, with discoverable keys, bounded search
feedback, and a documented temporary-installable command and Nushell integration.

**Blocked by:** T3, T4, T5.

**Acceptance criteria:**

- [ ] Wide terminals show results and preview side by side; 70–99 columns use
  F10-switchable Results/Preview panes with settings visible. Resizing preserves
  the search and selection; undersized terminals keep help/exit usable.
- [ ] All documented keys, focus transitions, shared mode state, history,
  Unicode display-width handling, Mocha/NO_COLOR, and action status work together.
- [ ] End-to-end PTY coverage at 120x32, 80x24, and 70x18 verifies file refinement
  and copy, directory browse/back/cd return, content-to-editor return, cancellation,
  resizing, and terminal restoration without pixel-coupled snapshots.
- [ ] Format, lint, behavioral tests, PTY checks, and a release build pass using
  documented commands. Exercise limits and failed OS actions proportionately.
- [ ] A temporary Cargo installation includes required highlighting data,
  integration assets, and dependency notices and runs without Node, a browser,
  a service, or an external ripgrep executable.
- [ ] User documentation covers launch scope, controls, ignore behavior, limits,
  editor resolution, JSON/Nushell setup, cancellation, and OSC 52 client support.
- [ ] Record automated evidence separately from the remaining manual local
  clipboard acceptance check. Preserve the user's active installation/config.
- [ ] Publish the implementation's review evidence against this spec; prototype
  code and synthetic data stay off the production branch.
