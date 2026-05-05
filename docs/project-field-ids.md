# Project field IDs reference

Reference for the GitHub Projects V2 board associated with this repo. Cowork (via Composio's GitHub MCP) and Claude Code (via the official github-mcp-server) both need these IDs — particularly the Status and Priority option IDs — to move items between columns or update field values. Looking them up costs an API call each time, so they're cached here.

If anyone changes the project's fields or option names in the GitHub UI, this file must be updated. There's no tooling to detect drift; treat it as schema documentation.

## Project

| Field | Value |
|---|---|
| Owner | `brendanbyrne` (user, not org) |
| Project number | `3` |
| Project node ID | `PVT_kwHOAA6jO84BWue2` |
| URL | https://github.com/users/brendanbyrne/projects/3 |

## Status field (single-select — the board columns)

| Field | Value |
|---|---|
| Field name | `Status` |
| Field node ID | `PVTSSF_lAHOAA6jO84BWue2zhSAUeI` |
| Field numeric ID | `343953890` |

| Option | Color | Option ID | Meaning |
|---|---|---|---|
| Backlog | green | `f75ad846` | Known work, not yet scoped. Milestone-optional. |
| Ready | blue | `1619f959` | Scoped, has acceptance criteria, milestone set. Anyone can pull. |
| In Progress | yellow | `47fc9ee4` | Work has started; PR may or may not be open. |
| Done | purple | `98236657` | Issue closed, PR merged. |

The Backlog option ID (`f75ad846`) was originally named "Todo"; it was renamed in the UI on 2026-05-05. The ID is preserved across the rename, so existing items on Todo automatically show as Backlog with no data migration needed.

## Priority field (single-select)

| Field | Value |
|---|---|
| Field name | `Priority` |
| Field node ID | `PVTSSF_lAHOAA6jO84BWue2zhSAv-I` |
| Field numeric ID | `343982050` |

| Option | Color | Option ID |
|---|---|---|
| Low | green | `f6bb992c` |
| Medium | yellow | `ae0843f9` |
| High | red | `89730d5f` |

Convention: **Medium is the default** for new Issues. The API exposes no "default" flag on a field option — the default is enforced by the project's auto-add workflow (configurable in the Settings UI), not the field schema itself. If you set Priority via the API on a freshly-added item, set it explicitly to `ae0843f9` rather than relying on auto-population.

## Other built-in fields

These rarely need to be set via the API (most are populated automatically by GitHub when an issue/PR is added), but listed for completeness.

| Field | Type | Node ID |
|---|---|---|
| Title | title | `PVTF_lAHOAA6jO84BWue2zhSAUeA` |
| Assignees | assignees | `PVTF_lAHOAA6jO84BWue2zhSAUeE` |
| Labels | labels | `PVTF_lAHOAA6jO84BWue2zhSAUeM` |
| Linked pull requests | linked_pull_requests | `PVTF_lAHOAA6jO84BWue2zhSAUeQ` |
| Milestone | milestone | `PVTF_lAHOAA6jO84BWue2zhSAUeU` |
| Repository | repository | `PVTF_lAHOAA6jO84BWue2zhSAUeY` |
| Reviewers | reviewers | `PVTF_lAHOAA6jO84BWue2zhSAUec` |
| Parent issue | parent_issue | `PVTF_lAHOAA6jO84BWue2zhSAUeg` |
| Sub-issues progress | sub_issues_progress | `PVTF_lAHOAA6jO84BWue2zhSAUek` |

Per the Projects V2 GraphQL API, `updateProjectV2ItemFieldValue` cannot set Assignees, Labels, Milestone, or Repository — those are properties of the underlying issue/PR, not the project item. Use the issue/PR mutations (`addAssigneesToAssignable`, `addLabelsToLabelable`, `updateIssue`, etc.) instead.

## Common write patterns

Reference snippets for the most common project-board operations. Argument shapes follow Composio's GitHub toolkit.

### Move an item to In Progress

```
GITHUB_UPDATE_USER_PROJECT_ITEM
  projectId  = PVT_kwHOAA6jO84BWue2
  itemId     = <PVTI_... node ID of the project item, NOT the issue ID>
  fieldId    = PVTSSF_lAHOAA6jO84BWue2zhSAUeI
  value      = { "singleSelectOptionId": "47fc9ee4" }
```

### Move an item to Ready

```
GITHUB_UPDATE_USER_PROJECT_ITEM
  projectId  = PVT_kwHOAA6jO84BWue2
  itemId     = <PVTI_...>
  fieldId    = PVTSSF_lAHOAA6jO84BWue2zhSAUeI
  value      = { "singleSelectOptionId": "1619f959" }
```

### Set Priority to High

```
GITHUB_UPDATE_USER_PROJECT_ITEM
  projectId  = PVT_kwHOAA6jO84BWue2
  itemId     = <PVTI_...>
  fieldId    = PVTSSF_lAHOAA6jO84BWue2zhSAv-I
  value      = { "singleSelectOptionId": "89730d5f" }
```

### Set a date field

```
value = { "date": "2026-05-15" }
```

### Set a number field

```
value = { "number": 3 }
```

### Set an iteration field

```
value = { "iterationId": "<iteration node ID>" }
```

### Clear a field

```
GITHUB_CLEAR_PROJECT_V2_ITEM_FIELD_VALUE
  projectId, itemId, fieldId
```

## Document history

- 2026-05-05 — Initial capture after first successful Composio MCP connection. Project was created with default fields only (Board template).
- 2026-05-05 — Renamed Todo → Backlog (option ID `f75ad846` preserved). Added Ready (`1619f959`) as a new Status option to support the Cowork-queues / Claude-Code-pulls workflow. Removed In Review (never created — workflow ultimately decided to track Issues only and rely on GitHub's native Pulls tab for PR-review state). Added Priority field (`PVTSSF_lAHOAA6jO84BWue2zhSAv-I`) with Low / Medium / High options. PR auto-add workflow disabled in Project Settings (board now tracks Issues only). All changes refreshed via `GITHUB_LIST_PROJECT_FIELDS_FOR_USER`.
