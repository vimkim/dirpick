# Issue tracker: GitHub

Specs and tickets use GitHub Issues in `vimkim/dirpick`.
Use the `gh` CLI. Pass `--repo vimkim/dirpick` explicitly for issue commands,
so another checkout cannot redirect an operation to its own tracker.

Work-tracker item 403 records the overall effort and session handoffs.
It does not replace feature specs or implementation tickets.

## Issue operations

- Create: `gh issue create --repo vimkim/dirpick --title "<title>" --body-file <path>`.
- Read: `gh issue view <number> --repo vimkim/dirpick --json number,title,body,labels,comments`.
- List: `gh issue list --repo vimkim/dirpick --state open --json number,title,labels`.
- Comment: `gh issue comment <number> --repo vimkim/dirpick --body-file <path>`.
- Apply or remove a label: `gh issue edit <number> --repo vimkim/dirpick --add-label "<label>"`
  or `--remove-label "<label>"`.
- Close: `gh issue close <number> --repo vimkim/dirpick --comment "<completion evidence>"`.

Use a temporary body file for multiline issue text, descriptions, and comments.
Preserve actual newlines.

## Specs and tickets

When a skill says to publish to the issue tracker, create a GitHub issue.
Use one issue for the approved spec and one issue per implementation ticket.
Link each ticket to its spec issue.

Use native GitHub sub-issues and blocking dependencies when available.
For a dependency, add the blocker to the dependent ticket:

```sh
gh api --method POST repos/vimkim/dirpick/issues/<dependent-number>/dependencies/blocked_by -F issue_id=<blocker-database-id>
```

Resolve the blocker's database ID with
`gh api repos/vimkim/dirpick/issues/<number> --jq .id`.
Use the database ID, not the issue number or node ID.

When native links are unavailable, record `Part of #<spec-number>` and
`Blocked by: #<number>, #<number>` in the ticket body and add a task list to
the spec issue. A ticket is ready only after all its blockers are closed.

## Pull requests as a triage surface

PRs as a request surface: no.
