# Dirpick v0.1: approved spec and implementation graph

The user approved the engineering defaults, application-level test boundary,
and six-ticket breakdown on 2026-10-16. GitHub Issues are authoritative.
The review copy is preserved in Git history at commit f04168d.

## Published issues

[Spec #1: Dirpick v0.1 search, preview, and navigation](https://github.com/vimkim/dirpick/issues/1)
contains the full accepted contract and testing decisions.

| Ticket | Delivers | Blocked by |
| --- | --- | --- |
| [#2](https://github.com/vimkim/dirpick/issues/2) | Find and preview real files from Project or launch CWD | None |
| [#3](https://github.com/vimkim/dirpick/issues/3) | Refine file searches with path filters and traversal toggles | #2 |
| [#4](https://github.com/vimkim/dirpick/issues/4) | Inspect syntax previews and copy exact absolute paths | #2 |
| [#5](https://github.com/vimkim/dirpick/issues/5) | Browse directories and change the Nushell directory | #3 |
| [#6](https://github.com/vimkim/dirpick/issues/6) | Search file contents and return from an editor at the match line | #3, #4 |
| [#7](https://github.com/vimkim/dirpick/issues/7) | Use and install the complete workflow in wide and narrow terminals | #4, #5, #6 |

Every issue has the ready-for-agent label. Native sub-issue relationships and
all blocking edges were verified after publication. Work-tracker item 403
continues to record overall progress and handoffs.

## Testing boundary

Drive the search-session application interface against real temporary
filesystem/Git fixtures and assert observable state, results, previews, and
requested actions. Add focused real PTY, editor-process, and Nushell adapter
checks at their OS boundaries. Keep tests independent of private helpers,
widget structure, and screenshot pixels. The spec gives the complete criteria.

## Implementation context

Build production code on one integration branch, using a separate topic
worktree per ticket. Preserve the prototype on its local branch
prototype/search-layouts at e49d91b. The native study is in
/home/vimkim/gh/dirpick-prototype/prototypes/terminal, with run notes and actual
PTY screenshots in that checkout. It is a visual/interaction reference and
contains synthetic data and simulated effects.

Local integration to main and its associated push remain a separate review
step after implementation, checks, and code review.
