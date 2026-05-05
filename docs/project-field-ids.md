# Project field IDs reference

Reference for the GitHub Projects V2 board associated with this repo. Cowork (via Composio's GitHub MCP) and Claude Code (via the official github-mcp-server) both need these IDs — particularly the Status option IDs — to move items between columns or update field values. Looking them up costs an API call each time, so they're cached here.

If anyone changes the project's fields or option names in the GitHub UI, this file must be updated. There's no tooling to detect drift; treat it as schema documentation.

## Project

| Field | Value |
|---|---|
| Owner | `brendanbyrne` (user, not org) |
| Project number | `3` |
| Project node ID | `PVT_kwHOAA6jO84BWue2` |
| URL | https://github.com/users/brendanbyrne/projects/3 |

## Custom fields

The project currently uses only GitHub's default fields — no custom fields have been added. Per `CLAUDE.md`, phases are tracked via Milestones rather than a custom Phase field.

## Status field (single-select — the board columns)

| Field | Value |
|---|---|
| Field name | `Status` |
| Field node ID | `PVTSSF_lAHOAA6jO84BWue2zhSAUeI` |
| Field numeric ID | `343953890` |

| Option | Color | Option ID |
|---|---|---|
| Todo | green | `f75ad846` |
| In Progress | yellow | `47fc9ee4` |
| Done | purple | `98236657` |

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
