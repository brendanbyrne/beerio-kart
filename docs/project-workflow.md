# Project workflow

How work moves through the project — Issues, Milestones, PRs, and the handoffs between Cowork (Claude Desktop) and Claude Code (WSL2 CLI).

This file is the operational guide. For high-level role split see `.claude/CLAUDE.md`; for cached project-board IDs see [`project-field-ids.md`](./project-field-ids.md); for handoff-file mechanics see [`.agents/handoffs/README.md`](../.agents/handoffs/README.md); for the cup-name milestone convention see the design record at [`docs/designs/2026-05-04-design-doc-restructure.md`](./designs/2026-05-04-design-doc-restructure.md) §12 (a copy of the cup mapping lands in `docs/roadmap.md` once that file is created in PR 3).

## Decision tree: where does this thing belong?

When you have *something* to communicate or capture, route it to one of:

| Destination | What goes there |
|---|---|
| **Issue** | Actionable work that fits in 1-3 PRs. Bugs, features, refactors with multi-session scope or user-visible impact. |
| **Issue comment** | Context update on an in-flight Issue. Discoveries that should stay attached to the Issue thread for grep-ability. |
| **PR comment** | Review feedback. Line-anchored where possible. |
| **ADR (`docs/decisions/`)** | Durable architectural decision worth being grepped-by-topic later (e.g., "we chose Argon2id"). |
| **Design record (`docs/designs/`)** | Multi-decision design session that produces ADRs and follow-up Issues. |
| **Handoff file (`.agents/handoffs/`)** | Non-task communication between assistants — research requests, design questions, urgent meta-changes, "this is a tag for those Issues I just filed." |
| **Self-notes (`.agents/memory/{cowork,claude-code}.md`)** | Session memory you keep for your own future sessions. **Not** a coordination channel. |
| **Chat (this conversation)** | Conversational only. Anything substantive that the other assistant needs gets written to one of the above. |

Two clarifying rules:

- **An Issue is the unit of "intent we want to remember."** Quick fixes done inline in the current branch don't need one. Anything that requires a new branch does.
- **Handoff files are not for things that fit Issue shape.** If it's actionable and structurable as Context / AC / References, it's an Issue. Handoffs are for the unstructured residual.

## Issue lifecycle

### Body format

```markdown
## Context
Why this exists, in 1-3 sentences.

## Acceptance criteria
- [ ] Specific testable thing 1
- [ ] Specific testable thing 2

## References (optional)
- design record / ADR / related Issues / docs links
```

Cowork-authored Issues append a footer: `_Filed by Cowork on Brendan's behalf._` Claude Code-authored Issues don't need attribution — context makes authorship obvious.

GitHub Issue templates at `.github/ISSUE_TEMPLATE/feature.md` and `bug.md` pre-populate this shape when filing via the UI. Cowork and Claude Code don't need the templates — they include the right body shape directly.

### Required fields at creation

- **Title** — descriptive, no leading prefix.
- **Body** — at minimum Context + Acceptance criteria.
- **Milestone** — *optional* for Backlog Issues. **Required** when the Issue moves to Ready (or beyond). Backlog status is the only milestone-free state.
- **Priority** — defaults to Medium (set by project auto-add). Adjust to Low or High if the default is wrong.
- **Labels** — `bug`, `enhancement`, `docs`, `blocked`, `good-first-task` is the initial set; add more as need arises.

### Status semantics

| Status | Meaning |
|---|---|
| Backlog | Known work, not yet scoped. Milestone-optional. |
| Ready | Scoped, has acceptance criteria, milestone set. Anyone (Brendan / Cowork via MCP / Claude Code via `gh`) can pull. |
| In Progress | Work has started; PR may or may not be open. |
| Done | Issue closed, PR merged (auto via `Closes #NN`). |

Transitions:

- **Backlog → Ready** — Brendan stamps. Cowork can stamp autonomously when (a) Brendan is in the conversation that filed the Issue, and (b) acceptance criteria are unambiguous. Otherwise filed at Backlog and surfaced for triage.
- **Ready → In Progress** — Whoever picks up the Issue moves it. No permission needed; status is a state-of-the-world signal, not a gate. **Do this as the first action on pickup, not at the end** — the value of the signal is the visibility that work has started.
- **In Progress → Done** — Automatic on PR merge for Issue-closing PRs (via `Closes #NN`). For Issues with no PR (Cowork-only work like updating a design record or running an MCP batch), close the Issue manually when done.

If you start work and find the Issue is broken (acceptance criteria wrong, dependency unclear, scope misjudged), pause: comment on the Issue and write a handoff for the other assistant — `.agents/handoffs/cowork.md` if you're Claude Code, `.agents/handoffs/claude-code.md` if you're Cowork. Don't "creatively interpret" the acceptance criteria — body changes go through the original creator or Brendan.

**If a workflow step itself is blocked** — a tool you don't have, a token scope you're missing, a permission you can't acquire — surface it the moment you hit it, don't silently skip. File a follow-up Issue if the gap is recurring; route the step through the other assistant if only one of them can perform it; or pause and ask. Silent-skip on a workflow signal is worse than not having the signal at all, because it makes the convention look optional. The conventions in this file are load-bearing — if you can't follow one, name it.

### Who can create, who can edit

- **Create:** all three (Brendan, Cowork, Claude Code).
- **Edit body:** original creator + Brendan only.
- **Comments:** append-only for everyone — that's the right channel for "AC update" (acceptance-criteria correction) or "I think we need a fourth criterion."

### Multi-PR Issues — tasklist epics

When a single Issue's scope is genuinely too big for one PR, split it as a tasklist epic:

```markdown
## Context
Implement Round-robin ruleset (Phase 4 / Shell milestone).

## Acceptance criteria
- [ ] #58
- [ ] #59
- [ ] #60
```

Each sub-Issue (`#58`, `#59`, `#60`) gets its own PR with its own `Closes #58`, etc. The parent's checkboxes auto-tick as sub-Issues close, and the parent itself closes when the last sub-Issue closes.

This is the **default for multi-PR work**. Don't pre-split — promote to epic when scope warrants.

There's an alternative pattern (one Issue, multiple PRs with `Refs #NN` on intermediate PRs and `Closes #NN` on the final). It's lighter weight but loses the per-sub-task milestone visibility. Use it only when the sub-tasks aren't worth tracking independently — e.g., a single behavioral change that just happened to need staging across two commits.

**Important:** for the one-Issue-many-PRs pattern, only the final PR uses `Closes #NN`; intermediate PRs use `Refs #NN` or `Part of #NN`. If every PR says `Closes #NN`, the first one to merge closes the Issue prematurely.

## Milestone lifecycle

### Naming

Two milestone types: **product cups** for user-facing feature work-chunks, **workstreams** for cross-cutting infrastructure that runs concurrent with product cups. The distinction was introduced 2026-05-11 after the Star milestone accumulated a heavy compliance-plan tail that obscured its actual product scope; pulling cross-cutting work into separate workstream milestones lets each milestone's progress bar mean what its name claims.

**Product cups.** Mario Kart 8 Deluxe cup names, claimed in chronological start order. The Nth product work-chunk gets the Nth cup. No semantic mapping — cup names are arbitrary chronological labels. Title format: `<CupName>: <Description>` (e.g., `Star: Sessions & Run Recording`).

**Workstreams.** Topical prefix instead of a cup name. Title format: `<Topic>: <Description>` (e.g., `Hardening: Backend compliance plan`, `Docs: Documentation overhaul`). Use this when the work is a cross-cutting concern (code hygiene, doc restructure, accessibility audit, observability buildout) that runs alongside product cups rather than being a discrete user-visible release.

Choosing between the two: ask "is this milestone's success criterion something a user would notice on the next release?" Yes → product cup. No → workstream. Workstreams can be long-lived (Hardening spans multiple product cups); product cups close when their feature deliverable ships.

**Prose form:** for product cups, write `Milestone <CupName>` (e.g., "frontend logic added in Milestone Star has somewhere to land tests"). The cup-prefix form disambiguates the cup name from the in-game item or the abstract work-chunk concept. For workstreams, plain prose is fine (`the Hardening milestone`, `the Docs milestone`) — workstream names don't have the same homonym problem.

Cup pool: 8 base cups (Mushroom, Flower, Star, Special, Shell, Banana, Leaf, Lightning), 4 MK8 Deluxe additions (Crossing, Bell, Egg, Triforce), 8 Booster Course Pass cups (Golden Dash, Lucky Cat, Turnip, Propeller, Rock, Moon, Fruit, Boomerang). 20 total. (Note: `Special` was originally used for the documentation overhaul; the 2026-05-11 convention update freed the cup name and renamed that milestone to the `Docs:` workstream. Special is available for the next product cup that needs it.)

The current cup-to-work-chunk mapping and workstream list live in [`roadmap.md`](./roadmap.md).

### When to open

**Just-in-time** — create a milestone when you know you need it (typically when the previous one is closing). No empty placeholder milestones.

Optional exception: pre-emptive creation as a *commitment device*. If you want to commit publicly that "yes, this work is happening eventually even though it's months out," create the milestone now. The cost is low (a closed-but-empty cup name) and the value is the commitment signal.

### Due dates

**Don't set them.** For a side project, due dates create false urgency more often than they help. Set a date only if there's a real driver (a friend's visit when you want X working, a conference you're presenting at). Otherwise leave the milestone open-ended.

### When to close

**Every Issue resolved.** A milestone closes when zero Issues remain open in it. Each remaining open Issue at close time gets one of:

- **Closed** — the work is done (or won't be done; close with a comment explaining).
- **Deleted** — the Issue is no longer relevant (duplicate, obsoleted, etc.).
- **Moved to Backlog** — work is still wanted but not committed to a release.
- **Moved to another milestone** — work belongs in the next cup or a future commitment.

The discipline is "this milestone is the memory of what we wanted; close it knowing every item has a definite disposition." Never leave a milestone open with phantom Issues.

### Milestones on PRs (don't)

GitHub's milestone progress bar counts both Issues *and* PRs. If you milestone an Issue *and* its corresponding PR, the milestone shows two items per piece of work and the progress bar lies.

**Convention:** Issues carry milestones; PRs don't, when they have a linked Issue. The Issue is the durable "what release does this belong to" record. The PR's milestone field stays empty.

Exception: chore PRs without a linked Issue can be milestoned if you want them tracked toward release completion. In practice, most chore PRs don't move the milestone needle either way.

## PR conventions

### Branch naming

- **Issue-linked PR:** `<issue_number>/<short-slug>` — e.g., `42/add-leaderboard`, `87/time-validation-edge-case`.
- **Chore PR (no linked Issue):** `<slug>` — e.g., `bump-axum-version`, `fix-typo-in-readme`. Distinguishable from Issue-linked branches by the absence of the leading number.

Issue number first lets `git checkout 42<TAB>` complete fast and gives tooling a clean lookup key.

### Title

PR title format: `<issue_number>: <Title>` — e.g., `42: Add leaderboard`, `87: Fix lap-time validation off-by-one`.

Mirrors the commit-message convention below; keeps the Issue number visible in the PR list and in merge-commit history on `main`.

For chore PRs without an Issue, omit the prefix (just the title).

### Linked work

`Closes #NN` mandatory for any PR closing an Issue. Chore PRs without Issues skip the reference (nothing to reference).

For multi-PR Issues, only the final PR uses `Closes #NN`; intermediates use `Refs #NN` or `Part of #NN`. (See "Multi-PR Issues — tasklist epics" above; tasklist epics dodge this entirely since each sub-Issue gets exactly one PR.)

The PR template at `.github/pull_request_template.md` has a "Linked work" section that enforces this in practice.

### Commit messages

Each commit's title follows the format `<issue_number>: <summary>` — e.g., `42: extract leaderboard service`, `87: fix lap-time validation off-by-one`.

Why:

- Cross-references the work to its tracking Issue at a glance.
- `git log --oneline` becomes a scannable history of which Issues were touched when.
- Especially valuable in PRs with many commits (e.g., one-commit-per-ADR distillation in Issue #39 / PR 2).

For chore PRs without an Issue, omit the prefix (just the summary). For multi-Issue PRs, use the primary Issue number in the prefix; the commit body can reference others (`Refs #NN`).

### Review and merge

- **Never push directly to `main`.** All code changes require a PR.
- **Never merge your own PR.** Only Brendan merges.
- Documentation-only changes can commit to `main` directly per `.claude/CLAUDE.md`.

### PR template structure

The template at `.github/pull_request_template.md` has these sections, in order:

1. **Reviewer notes** — surprises, gotchas, dependent PRs, alternatives tried. At the top so it lands first.
2. **Summary** — what the PR did, in 1-3 sentences.
3. **Linked work** — `Closes #NN`, ADRs implemented, design records cited.
4. **How to verify** — reviewer's step-by-step checklist. Each step has its success criterion built in. Embed commands in fenced code blocks so they get a copy button. If no manual verification is needed (typo fix, dep bump), write "Skip — diff review only." Do not leave this section blank.
5. **Author checklist** — confirms before requesting review (Issue linked, tests added, docs updated, schema-change verified, tested locally).

## Triage

### Triggers

Triage is event-driven, not calendar-driven. Two natural triggers:

- **Milestone close-out.** Per the milestone-close rule, every milestone close requires deciding the disposition of remaining open Issues. This is the most reliable triage moment.
- **Backlog growth.** When Backlog gets large enough that scanning it feels heavy (~20+ Issues, rough heuristic). Cowork flags this proactively and proposes a triage session.

Optional weekly cadence is overkill for solo dev. Skip it unless rhythm pressure shows up.

### Process

Triage is **Brendan + Cowork together in a session**. Cowork preps beforehand:

- Reads all Backlog Issues.
- Groups them by topic / cup-affinity.
- Suggests milestone assignments and labels.
- Identifies Issues that should be deleted (duplicates, stale, no longer relevant).

Brendan approves / adjusts in batch. Promotion to Ready happens during the session. Cowork doesn't run triage autonomously — it requires too much subjective judgment about which work is worth doing.

## Multi-assistant coordination

### Cowork's autonomy in filing Issues

- **Direct-file (no review gate):** single Issue, acceptance criteria unambiguous, Brendan in the conversation that surfaced it. Same as the implicit-stamp rule for Backlog → Ready.
- **Draft-and-review:** multi-Issue batches (3+), or single Issues with ambiguous scope. Cowork writes a draft, Brendan signs off, Cowork bulk-files via MCP.
- **Default to direct-file** if uncertain — recovery from a wrongly-filed Issue (close it) is cheaper than the friction of a review gate.

All Cowork-created content on GitHub appears under `brendanbyrne` (the MCP authenticates as that user). The `Filed by Cowork on Brendan's behalf.` footer on Issue bodies makes the project history clearly distinguish Cowork's writes from Brendan's.

### Claude Code's autonomy in moving Issue status

- **Ready → In Progress:** when starting work. No permission needed.
- **In Progress → Done:** automatic via `Closes #NN` on PR merge.
- **Body edits:** never. If AC is wrong, comment on the Issue and write a handoff for Brendan or Cowork.

Claude Code's `gh` CLI token carries the `project` scope (which implicitly includes `read:project`), letting it set the Status field on project items via `gh api graphql` calls to `updateProjectV2ItemFieldValue`. Project field IDs are cached in `docs/project-field-ids.md`. If a fresh checkout fails with `INSUFFICIENT_SCOPES`, refresh with `gh auth refresh -h github.com -s project`. The token also carries `repo`, `read:org`, `gist`, and `admin:public_key` for unrelated workflows.

### Handoff-as-tag pattern

When one assistant files Issues for the other to consider, it appends a small note to the corresponding handoff file. The handoff is a **lightweight notification with cross-references**, not a description of the work.

Example (Claude Code → Cowork):

```markdown
# Claude Code → Cowork (2026-05-12)

Filed during work on the leaderboard PR (#42):

- #4 — perf concern in the rivals query, needs design input
- #56 — race condition in session handoff, careful design pass required
- #3 — small follow-up to #56, can be tackled together

All on Backlog. Prioritize as you see fit.
```

Or for the simplest case:

```
Filed for design review: #4 (perf), #56 (race), #3 (follow-up). All on Backlog.
```

Two rules that follow:

1. **Don't duplicate Issue content in the handoff.** Details live in the Issues; the handoff just points at them.
2. **Brendan or the next Claude Code session deletes the handoff** after Cowork acknowledges in chat. Cowork can't delete files (sandbox `unlink()` block per `.agents/handoffs/README.md`).

The same convention applies in reverse — Cowork → Claude Code handoffs that file Issues use the same minimal format.

### Plan deviations during PR work

When Claude Code deviates from a handoff or design record during implementation:

- **Always:** describe the deviation in the PR's "Reviewer notes" section. That's the durable artifact attached to the diff.
- **Additionally, write `.agents/handoffs/cowork.md` if** the deviation has implications beyond this PR — the handoff plan was wrong, the design record needs updating, or future PRs are affected. The handoff cross-references the PR with a one-line description of what Cowork needs to address. **Don't duplicate the Reviewer notes content; point at it.**

Same pattern as the handoff-as-tag for filed Issues above: the handoff is a tag with action prompts, not a description of the work. Keep it small enough that future Cowork can scan it in seconds.

### What does NOT belong in handoff files

- **Anything that fits Issue shape.** That's an Issue, not a handoff.
- **PR review feedback.** That's a PR comment, line-anchored where possible.
- **Self-notes.** Those go in `.agents/memory/cowork.md` or `.agents/memory/claude-code.md`.
- **Anything intended to outlive the assistant's "I'm done with this" moment.** That goes to a durable artifact — Issue, ADR, design record, or self-notes.

## Document history

- 2026-05-05 — Initial draft, captured as part of the documentation overhaul (PR 1). Decisions worked through across the 2026-05-05 Cowork session covering the decision tree, Issue conventions, milestone conventions, PR conventions, triage cadence, and multi-assistant coordination. Sourced from `cowork-notes.md` and the design record amendment §12.
- 2026-05-05 — Added `### Commit messages` subsection (`<issue_number>: <summary>` title format) and `### Plan deviations during PR work` subsection (PR Reviewer notes is primary; `claude-code-handoff.md` cross-references when deviation has implications beyond the PR). Surfaced during PR 1's implementation when Claude Code deviated from the handoff plan; the deviation pattern wasn't documented.
- 2026-05-06 — Added a paragraph under `### Claude Code's autonomy in moving Issue status` documenting the `project` scope on Claude Code's `gh` token, the `updateProjectV2ItemFieldValue` mutation, and the `gh auth refresh` recovery for `INSUFFICIENT_SCOPES` errors. Companion to PR #45's #44-closing change — with this paragraph and a matching trim of `.claude/CLAUDE.md` § GitHub access (committed together), `workflow.md` becomes the canonical home for these details and `.claude/CLAUDE.md` keeps a one-line pointer.
- 2026-05-05 — Hardened the Ready → In Progress transition rule (do it as the *first* action on pickup) and added a paragraph on surfacing workflow-step blockers immediately rather than silently skipping. Surfaced during PR 2 when Claude Code deferred the status move on Issue #39 instead of attempting it (and discovering the `gh` token lacks `read:project` / `project` scopes — now tracked in #44).
- 2026-05-05 — Removed the now-stale parenthetical from the workflow-step-blocker rule that gave "handoff to Cowork for project-board mutations Claude Code can't do" as the example. Closes the loop with #44 — Claude Code's `gh` token now has `project` scope, so project mutations are no longer routed through Cowork.
- 2026-05-08 — Renamed file from `workflow.md` to `project-workflow.md`. The s/no-s distinction with the new sibling `user-workflows.md` (added in PR 4) was too fragile (grep noise, easy typos, tab-completion ambiguity); the rename makes intent explicit and matches the doc's `# Project workflow` title. Cross-references updated in `.claude/CLAUDE.md`, `docs/CLAUDE.md`, `docs/README.md`, `docs/handoffs/README.md`, `docs/roadmap.md`, and `docs/user-workflows.md`.
- 2026-05-08 — Updated handoff and self-notes path references throughout (`docs/handoffs/` → `.agents/handoffs/`, `.claude/*-notes.md` → `.agents/memory/*.md`) per the AI-state reorg in [#79](https://github.com/brendanbyrne/beerio-kart/issues/79).
- 2026-05-14 — Added `### Title` subsection under `## PR conventions` documenting the `<issue_number>: <Title>` PR title format (mirrors the existing commit-message convention). Captured so Claude Code applies the format when creating PRs.
