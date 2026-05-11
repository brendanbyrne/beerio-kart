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

## Iteration field

| Field | Value |
|---|---|
| Field name | `Iteration` |
| Field node ID | `PVTIF_lAHOAA6jO84BWue2zhScvcQ` |
| Duration | 14 days (per iteration) |
| Start day | Monday (`startDay = 1`) |

| Option | Title | Start date | Iteration ID |
|---|---|---|---|
| Iter 1 | Audit close + C1 (thiserror) | 2026-05-11 | `f114485e` |
| Iter 2 | H1 lint cleanup (high-signal) | 2026-05-25 | `11db1a9f` |
| Iter 3 | H1 lint cleanup (style) | 2026-06-08 | `1471cca6` |
| Iter 4 | Star kickoff (build chore + schema + lifecycle) | 2026-06-22 | `d77264fd` |
| Iter 5 | Star session APIs + UI | 2026-07-06 | `9263cfd3` |
| Iter 6 | Star run recording + photos | 2026-07-20 | `47ed1d7c` |

Iteration IDs are stable as long as the iteration's slot in the configuration list isn't renumbered. **Editing the iteration list via `updateProjectV2Field` rotates the IDs of every iteration**, even ones whose start date and duration didn't change — the input type `ProjectV2Iteration` has no `id` field, so the API treats every entry as new. If you regenerate iterations, refresh this table and re-set the iteration value on every project item that referenced an old ID.

REST creation via Composio (`GITHUB_ADD_FIELD_TO_USER_PROJECT` with `data_type: iteration`) returns 500 — the underlying `POST /users/.../projectsV2/N/fields` REST endpoint doesn't exist for iteration fields. The working path is `GITHUB_RUN_GRAPH_QL_QUERY` calling `createProjectV2Field` / `updateProjectV2Field` mutations directly. Updating supports partial overwrite (set `iterationConfiguration` to overwrite the full iteration list; other field properties stay).

## Roadmap view

| Field | Value |
|---|---|
| View name | `Roadmap` |
| View number | `6` |
| View node ID | `PVTV_lAHOAA6jO84BWue2zgKSLrw` |
| View numeric ID | `43134652` |
| Layout | `roadmap` |
| URL | https://github.com/users/brendanbyrne/projects/3/views/6 |

Created via `GITHUB_CREATE_VIEW_FOR_USER_PROJECT` (REST shim — works for views, unlike for iteration fields). Initial visible_fields: Title, Assignees, Status, Linked PRs, Sub-issues progress.

**Two view settings are NOT auto-set by `GITHUB_CREATE_VIEW_FOR_USER_PROJECT`, both required for a usable iter-anchored Roadmap.** Earlier revisions of this file claimed "the Iteration field anchors the timeline axis automatically since it's the only iteration field on the project." Wrong on two counts:

1. **Dates (timeline horizontal axis)** defaults to "Start date / Target date". Without those date fields populated on items, the timeline renders empty even when iteration values are set. Fix: view dropdown → **Dates → Iteration**.
2. **Group by (vertical sections)** defaults to none, so items appear as one flat list rather than grouped into iter sections. Fix: view dropdown → **Group by → Iteration**.

Both are settings-UI-only — neither is exposed on `ProjectV2View` in GraphQL (`groupByFields` is exposed but appears empty even after the UI sets it; the date-axis binding has no field at all). So after creating any Roadmap view via the REST shim, open it in the UI, flip both, and click **Save view**. The need to click Save is also non-obvious: changes appear applied but revert on reload until saved.

The GraphQL API does not expose mutations to manage views (no `createProjectV2View` / `updateProjectV2View`); Composio's REST shim is the only programmatic path. Settings-UI-only operations: changing the view's filter expression, toggling fields, changing the timeline axis field, sort order, group-by.

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
- 2026-05-09 — Added the Iteration field (`PVTIF_lAHOAA6jO84BWue2zhScvcQ`) and Roadmap view (#6, `PVTV_lAHOAA6jO84BWue2zgKSLrw`) per the Roadmap experiment. 6 iterations of 14 days each starting 2026-05-11 cover audit close, the C1 + H1+ Quality Pass, and Star kickoff through run recording. Backfilled iteration values on all 32 then-open Issues. The Iteration field section above also corrects two stale claims in this file's prose: (1) `data_type=iteration` via `GITHUB_ADD_FIELD_TO_USER_PROJECT` actually 500s — the GraphQL `createProjectV2Field` mutation works; (2) views CAN be created programmatically via Composio's `GITHUB_CREATE_VIEW_FOR_USER_PROJECT` REST shim, contrary to docs/CLAUDE.md's "Settings-UI-only" assertion (which is correct for fields' option lists and for the auto-add/auto-close workflows, but not for view creation or iteration fields).
- 2026-05-11 — Corrected the Roadmap view section's claim that "the Iteration field anchors the timeline axis automatically." It does not. A freshly-created Roadmap view defaults to Start/Target date for the timeline axis AND no Group by; both must be flipped to Iteration in the UI to get an iter-anchored Roadmap. The date-axis binding has no GraphQL field at all; `groupByFields` exists on `ProjectV2View` but stayed empty even when Group by was set via the UI (possibly an API/UI sync lag worth re-verifying). Save view in the UI is also required — changes appear applied but revert on reload until saved. Surfaced while filing Issue #114 (PR-G3): items had correct iter values but the Roadmap view was a flat list; Brendan flipped both settings, confirmed the view renders correctly, then saved.
