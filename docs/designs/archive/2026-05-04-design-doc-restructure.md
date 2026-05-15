# Design doc restructure — design record (2026-05-04)

> **Status: complete.** Archived 2026-05-15. All seven implementation PRs landed. Retained for historical reference. The archive convention originally documented here in § 8.10 has been superseded — live archive rules now live in [`../../CLAUDE.md`](../../CLAUDE.md) § Design records → Archive, with the 3-month wait dropped and the scope broadened to cover completed-initiative artifacts (e.g., the compliance plan) alongside design records.

Date: 2026-05-04
Author: Cowork
Scope: `docs/design.md` and the surrounding `docs/` layout

## Executive summary

`docs/design.md` is at 954 lines / ~64 KB and growing. The file mixes content with very different lifecycles (immutable game rules ↔ weekly-changing build plan), has an accreting "Resolved Decisions" bullet list (35+ entries with no structure), and partly duplicates `docs/api-contract.md`. The result is high token cost on every session, awkward editing collisions, and poor isolation when handing scoped tasks to subagents.

This record proposes a moderate restructure that splits `design.md` into purpose-specific files, adopts MADR-format ADRs for decision records, moves the build plan to `docs/roadmap.md` plus GitHub Issues + a Project board, adds a `docs/README.md` index plus optional nested `CLAUDE.md` files in `backend/` and `frontend/`, and tightens the lifecycle conventions for `reviews/design/` and `reviews/pr/` so they cross-link cleanly with the new ADRs. No content is rewritten — most of the work is moving existing prose to better-shaped homes.

The proposal is grounded in current Anthropic guidance and community write-ups on Claude Code documentation patterns; sources are listed inline.

Each section below has a checkbox. Sign off by marking Approved / Needs discussion / Skip.

---

## 1. Context

### 1.1 What `design.md` looks like today

Section breakdown by approximate line count:

| Section | Lines | Lifecycle | Primary audience |
|---|---|---|---|
| Overview / Rules / Principles / Goals | ~30 | Effectively immutable | Everyone, once |
| Tech Stack / Observability / Coverage / Naming | ~70 | Slow change | Devs onboarding |
| Data Model | **~330** | Edited every schema PR | Schema work |
| User Workflows | ~80 | Per-feature | Product/UX |
| API Surface | ~100 | Per-endpoint | Backend ↔ frontend |
| UI Screens | ~55 | Per-screen | Frontend |
| Project Structure | ~95 | Rarely | Onboarders |
| Build Plan / Phases | ~85 | Edited weekly | Brendan, planning |
| Resolved Decisions | ~40 (and growing) | Append-only forever | Anyone asking "why?" |
| Backlog / Related docs / History | ~15 | Mixed | Mixed |

### 1.2 Concrete pain points

- **Subagent token waste.** Spawning a subagent with relevant context today means dumping all 954 lines, because relevant bits are scattered. The right fix is task-scoped files an agent can load on demand.
- **Working memory tax.** Every session reads the entire file (per the `CLAUDE.md` "read at start of every session" rule). 64 KB is a real cost paid before any work happens.
- **ADR graveyard.** "Resolved Decisions" is a flat bullet list of 35+ entries with no individual dates, no context/consequence framing, no IDs, and no way to link to one decision from a code change. This is exactly the pain point ADRs were invented to solve ([Michael Nygard, 2011](https://www.cognitect.com/blog/2011/11/15/documenting-architecture-decisions); [adr.github.io](https://adr.github.io/)).
- **API duplication.** "API Surface" in `design.md` partially duplicates `docs/api-contract.md`. Two places describing the same contract is a maintenance trap.
- **Build-plan churn pollutes architecture.** A weekly-changing checklist sharing a file with stable architectural narrative puts edit pressure on a doc that should be stable.

### 1.3 Evidence supporting a split

- **Context rot.** LLM recall degrades nonlinearly as context grows; content placed in the middle of long inputs gets less attention than content at the start or end (the "lost in the middle" effect, Liu et al. 2023, summarized in [Salt Creative — From Monolithic Prompts to Modular Context](https://dev.to/salt_creative/from-monolithic-prompts-to-modular-context-a-practical-architecture-for-agent-memory-1lcp)). A 950-line file is the shape that triggers this.
- **CLAUDE.md instruction-budget guidance.** Multiple recent best-practice write-ups recommend keeping `CLAUDE.md` under 200 lines (some say 100) because Claude Code's system prompt already consumes ~50 instructions, and LLMs reliably follow ~150–200 total before degradation ([Builder.io — How to Write a Good CLAUDE.md](https://www.builder.io/blog/claude-md-guide); [DataCamp — Writing the Best CLAUDE.md](https://www.datacamp.com/tutorial/writing-the-best-claude-md)). The same logic applies, with a softer cap, to other every-session docs.
- **Progressive disclosure is the dominant Claude Code pattern.** Tiered structure: short root `CLAUDE.md` → skills loaded on demand → deep reference docs in `docs/` consulted only when relevant ([Best practices for Claude Code](https://code.claude.com/docs/en/best-practices); [Implementing CLAUDE.md and Agent Skills — Matthew Groff](https://www.groff.dev/blog/implementing-claude-md-agent-skills); [Anatomy of the .claude/ Folder](https://blog.dailydoseofds.com/p/anatomy-of-the-claude-folder)).
- **Steelman noted.** One contrarian piece argues monolithic scripts have an underrated advantage with capable models ([Modularity — An Overrated Anti-Pattern?](https://dev.to/embernoglow/modularity-an-overrated-anti-pattern-the-power-of-the-monolithic-script-in-the-age-of-ai-5oc)) and a research paper suggests modularity's main benefit is "preventing early failure on hard tasks" rather than universal speedup ([Modular and Hybrid Architecture for LLM Agents](https://openreview.net/pdf?id=gC3D2ESSyK)). The pain points listed in 1.2 are exactly the cases modularity solves; the steelman doesn't apply here.

- [x] Approved — context accurate
- [ ] Needs discussion
- [ ] Skip

---

## 2. Decision: target structure

The new `docs/` layout:

```
docs/
├── README.md                      # NEW. Short index: "if you're working on X, read Y."
├── design.md                      # KEPT, slimmed to ~250 lines:
│                                  #   rules, principles, goals, tech stack,
│                                  #   observability, naming. Links to the rest.
├── data-model.md                  # NEW. The schema section, lifted as-is.
├── workflows.md                   # NEW. Workflows + UI screens, merged.
├── roadmap.md                     # NEW. Phase narrative + success criteria.
│                                  #   Working checklist lives in GitHub Issues.
├── api-contract.md                # KEPT, absorbs the API Surface section.
├── decisions/                     # NEW. One MADR file per resolved decision.
│   ├── README.md                  # Index, status legend, template link.
│   ├── template.md                # MADR template for new ADRs.
│   ├── 0001-sqlite-strict-mode.md
│   ├── 0002-uuid-vs-integer-pks.md
│   └── …
└── coding-standards/              # KEPT as-is.
    ├── README.md
    ├── rust.md
    ├── seaorm.md
    └── tokio.md
```

Plus, in the repo root:

- `README.md` — absorbs the **Project Structure** section from `design.md` (it's repo-bootstrap content, not architecture).
- `backend/CLAUDE.md` — NEW. Backend-specific Rust/SeaORM rules currently in root `CLAUDE.md` and partly in `coding-standards/`. Loaded only when Claude works in `backend/`.
- `frontend/CLAUDE.md` — NEW. Frontend-specific TypeScript/React/Tailwind rules. Loaded only when Claude works in `frontend/`.

Source for the nested `CLAUDE.md` pattern: [Best practices for Claude Code](https://code.claude.com/docs/en/best-practices), [Anatomy of the .claude/ Folder](https://blog.dailydoseofds.com/p/anatomy-of-the-claude-folder).

- [x] Approved — target structure
- [ ] Needs discussion
- [ ] Skip

---

## 3. Decision: ADR format and conventions

### 3.1 Format: MADR

Use [MADR](https://adr.github.io/madr/) (Markdown Any Decision Records), not Nygard's prose-only original. Reasons:

- MADR has YAML frontmatter (`status`, `date`, `deciders`) that's machine-parseable. This makes the index queryable and lets agents filter by status without reading bodies.
- MADR has explicit Context / Decision Drivers / Considered Options / Decision Outcome / Consequences sections. The structure is what makes ADRs effective; flat prose drifts.
- MADR is the convention in agent-friendly repos and has dedicated tooling ([adr-agent](https://github.com/macromania/adr-agent), [MADR skill on Smithery](https://smithery.ai/skills/cmd-llm/adr)).

### 3.2 File naming and numbering

- Files named `NNNN-kebab-case-title.md`, four-digit zero-padded sequence, e.g. `0007-h2h-derivation.md`.
- Sequence is repo-global (not per-area). Simpler than nested numbering.
- Title should describe the decision, not the topic: `0001-use-sqlite-strict-on-static-tables.md`, not `0001-database.md`.

### 3.3 Index file (`docs/decisions/README.md`)

A short table: number → title → status → date. Auto-maintainable from frontmatter, but for the friend-group scale of this project, manual upkeep is fine. Status legend: `proposed`, `accepted`, `superseded`, `deprecated`. Most existing entries land as `accepted`.

### 3.4 What becomes an ADR vs. a `reviews/design/` record

These are complementary, not redundant:

- **`reviews/design/NNNN.md`** — the *sign-off process*. Checkbox-driven, conversational, time-bounded. May propose multiple decisions.
- **`docs/decisions/NNNN.md`** — the *canonical record* of one accepted decision. Lives forever. Linked from code, from `design.md`, from other ADRs.

A design review session can produce zero, one, or several ADRs. After sign-off, the relevant decisions get distilled into ADR files. The `reviews/design/` record stays in place as historical context for *how* the decision was reached.

### 3.5 Linking from code

Code that depends on a non-obvious decision should link the ADR in a comment, e.g. `// See docs/decisions/0007-h2h-derivation.md`. This is what makes the ADR worth the effort — when someone hits the code, they get the rationale immediately.

### 3.6 Model tiering for bulk ADR generation

Generating ADRs in bulk (e.g., the ~35 from Resolved Decisions in PR 2) uses a tiered approach to save heavy-model cost without losing quality:

- I draft one canonical ADR by hand as the example.
- I spawn a Haiku-tier subagent with the example, template, and remaining bullets, asking for one ADR per bullet.
- I review the drafts; the few decisions deserving richer treatment (auth strategy, H2H derivation, photo enforcement) I revise or rewrite by hand.

One-off ADRs that arise from a single new decision don't need this pattern — I just write them. Templated bulk work is where the tier-down pays off. See §6.4 for the broader model-tiering convention.

- [x] Approved — ADR format and conventions
- [ ] Needs discussion
- [ ] Skip

---

## 4. Decision: roadmap split

### 4.1 `docs/roadmap.md` (narrative)

Contains:

- Phase descriptions (goals, scope, what's deferred, success criteria).
- Cross-phase notes that don't belong in any single issue.
- A pointer to the GitHub Project board for current status.

Does **not** contain the working checklist. Status lives in Issues.

### 4.2 GitHub Issues + Project (working checklist)

- Each phase becomes a GitHub **milestone** (e.g., `Star: Sessions & Run Recording`). Milestones can carry a `due_on` date for phase boundaries.
- Each current `- [ ]` checkbox in `design.md`'s Build Plan becomes an **issue** with the phase as its milestone.
- A single **GitHub Project (v2)** holds the issues. The Project supports multiple views over the same data:
  - **Board view** — kanban: `Backlog → Ready → In progress → In review → Done`. Daily-driver view.
  - **Roadmap view** — Gantt-style timeline. Items appear as bars on a horizontal time axis. Useful for seeing phase boundaries (when does Phase 3 wrap, when does Phase 4 start).
- PRs use `Closes #NN` to auto-close the linked issue and move the card.

Relationship to `docs/roadmap.md`: the markdown file is the narrative — *why* a phase exists, scope, success criteria, what's deferred. The Project's Roadmap view is the visual — *when* phases run, driven by milestone `due_on` dates and (optionally) per-issue start/target date fields. Same underlying data sources (Issues + milestones), different presentation. They don't overlap.

Initial label set (light): `bug`, `enhancement`, `docs`, `blocked`, `good-first-task`. More can be added as need arises; over-labeling is a known drift hazard.

Milestone titles use Mario Kart cup names per §12 — see that section for the convention and current cup-to-work-chunk mapping.

### 4.3 Why split

- Issues are queryable (`is:open milestone:"Phase 3"`); markdown checklists aren't.
- PR ↔ task linkage makes status accurate without manual upkeep.
- Brendan can triage from the GitHub mobile app; a markdown file is harder to act on from a phone.
- Keeps weekly-churn task state out of the architecture doc.

Bound on cost: the Project board is one-time setup. Adding issues is incremental and Claude Code can do it in batches (`gh issue create ...`).

### 4.4 Model tiering for Issue creation

Generating Issues from a design's implementation plan follows the same tier-down pattern as bulk ADR generation (§3.6):

- I draft one canonical Issue as the example (title, body, milestone, labels).
- I spawn a Haiku-tier subagent with the example and the remaining plan items, asking for one Issue per item.
- I review for accuracy and adjust.

When GitHub MCP is authenticated, the subagent calls MCP tools directly. Until then, the subagent emits `gh issue create` commands and Claude Code executes them in batch. Either path produces the same result.

- [x] Approved — roadmap split
- [ ] Needs discussion
- [ ] Skip

---

## 5. Per-section migration plan

What moves where, with rationale.

### 5.1 Data Model → `docs/data-model.md`

The largest, most self-contained extraction. ~330 lines move verbatim. `design.md` keeps a one-paragraph summary plus a link. This single change recovers the largest chunk of token budget.

### 5.2 Resolved Decisions → `docs/decisions/NNNN-*.md`

Each of the ~35 bullets becomes a MADR file. Most will be short (30–80 lines). A few (auth strategy, H2H derivation, photo enforcement) deserve longer treatment with proper Considered Options and Consequences sections — flag those for closer attention rather than mechanical copying.

The `design.md` "Resolved Decisions" section becomes a one-line pointer to `docs/decisions/`.

### 5.3 Build Plan → `docs/roadmap.md` + GitHub Issues

Per § 4. Phase descriptions become roadmap content; checkboxes become issues. `design.md` references the roadmap.

### 5.4 API Surface → merge into `docs/api-contract.md`

`api-contract.md` currently covers wire-format conventions (error codes, ETag polling, idempotency, time format). It's the right home for the endpoint list too. Merge the API Surface section into it; remove from `design.md`.

### 5.5 User Workflows + UI Screens → `docs/workflows.md`

These describe the same flows from two angles (data flow vs. screen layout). Merging removes a sync hazard. Each workflow gets a "Screen" subsection with the UI bullets that previously lived in their own section.

### 5.6 Project Structure → `README.md` (repo root)

Repo-bootstrap content. Belongs at the repo entry point, not inside the architecture doc. The current `README.md` is a stub that points at `docs/`; this fills it out.

### 5.7 What stays in `design.md`

After extractions, `design.md` retains:

- Overview, Rules of the Game, High Level Principles, Design Goals (the immutable parts).
- Technical Constraints, Tech Stack (with ORM Usage subsection).
- Observability, Coverage & CI, Naming Conventions.
- Backlog, Related documents, Document history.
- Short pointers to extracted files.

Estimated post-trim length: ~250 lines.

- [x] Approved — per-section migration plan
- [ ] Needs discussion
- [ ] Skip

---

## 6. New additions

### 6.1 `docs/README.md` (the doc index)

Short, scannable, task-oriented. Example shape:

```markdown
# Beerio Kart docs

Start with `design.md` if you're new.

## I want to…

- **Touch the database schema** → `data-model.md` + relevant ADRs in `decisions/`.
- **Build or change an endpoint** → `api-contract.md` + `workflows.md`.
- **Add a session ruleset** → `workflows.md` (session loop) + a new ADR.
- **Understand why a decision was made** → `decisions/` (search the index).
- **Plan or pick up phase work** → `roadmap.md` + GitHub Issues.
- **Follow the coding standards** → `coding-standards/`.
```

Length cap: under one screen. Long indexes get ignored. If it grows past 30 lines, that's a signal to consolidate, not to make the index longer.

### 6.2 Nested `CLAUDE.md` files (`backend/`, `frontend/`, `docs/`)

Nested `CLAUDE.md` files load only when Claude is working in that subtree. This keeps stack-specific and area-specific rules out of every-session context.

Suggested content moves:

- **`backend/CLAUDE.md`** — the testing rules from root CLAUDE.md (currently at "Testing"), the schema-changes-in-prelaunch policy, pointers to `coding-standards/rust.md` / `seaorm.md` / `tokio.md`.
- **`frontend/CLAUDE.md`** — TBD. Frontend conventions are currently sparse; this can land empty-with-a-stub and grow as frontend-specific rules emerge.
- **`docs/CLAUDE.md`** — doc-area conventions. Specifically: design-record lifecycle (drafts → tracked → archived; see §8); MADR format and numbering; cross-linking conventions (ADR `Source:` line, design records' "ADRs produced" list); the implementation-plan section convention; model-tiering rules for bulk doc work (see §6.4 and §3.6 / §4.4); and the "where does this content go?" decision tree (design record vs. ADR vs. roadmap vs. Issue vs. handoff file).

The root `CLAUDE.md` keeps cross-cutting rules: handoff files, repo location, two-assistant workflow, git workflow, who-does-what table, documentation history rule.

Source on nested loading behavior: [Anatomy of the .claude/ Folder](https://blog.dailydoseofds.com/p/anatomy-of-the-claude-folder), [Best practices for Claude Code](https://code.claude.com/docs/en/best-practices).

### 6.3 Trim root `CLAUDE.md` to under 200 lines

Current root `CLAUDE.md` is ~200 lines; right at the recommended ceiling. After moving stack-specific and doc-area content to nested files (§6.2), it should be comfortably under. No content needs to be deleted — just moved.

### 6.4 Model tiering convention

Process work falls into three tiers:

- **Heavy LLM** — open-ended judgment, novel reasoning, holding lots of context. Examples: design conversations, code review, implementation handoff drafting, content triage during migrations, research synthesis where citation accuracy matters.
- **Light LLM** (Haiku-tier) — templated extraction, structured reformatting, summarization. Examples: bulk ADR generation (§3.6), Issue creation from implementation plans (§4.4), index-row updates, PR comment summary lines, standup digests.
- **No LLM** — pure tooling. Examples: link checking via `lychee-action`, sign-off state parsing, PR template enforcement.

The general principle: as process formalizes (templates, conventions, structured inputs/outputs), tasks shift from heavy → light → no-LLM. Bake the tier choice into the workflow so future sessions don't re-derive it.

Where to encode tier choices:

- Specific bulk operations (ADRs, Issues): documented inline in their respective sections (§3.6, §4.4).
- General doc-area tier rules: `docs/CLAUDE.md` (per §6.2) so any session working in `docs/` sees them.
- Skill descriptions: when we add custom skills (e.g., a future `adr-from-bullets` or `issues-from-plan`), the skill description states its tier.

Implementation note: the `Agent` tool accepts a `model` parameter (`haiku`, `sonnet`, `opus`). Subagents I spawn for templated work pass `model: "haiku"` explicitly. This is available today; no infrastructure changes needed.

### 6.5 `docs/research/` (already exists; formalize)

`docs/research/` already exists in the repo with two files: `ocr-strategy.md` (May 2026 research on the OCR pipeline for time extraction) and `seaorm_2_0_migration.md` (May 2026 evaluation of partial-unique-index support in the SeaORM 2.0 entity-first workflow). It was added during the SeaORM audit and was not in scope of the original §1–§5 restructure plan. Worth formalizing now so the new structure acknowledges it.

Purpose: long-form technical investigations that inform designs but don't propose decisions themselves. Examples:

- Vendor / library evaluations (SeaORM 2.0 evaluation).
- Strategy explorations ahead of a future design (OCR strategy).
- Industry surveys (deep-dives into how others have solved a problem).
- Output of subagent deep research that's too long to inline in a design and will inform multiple future designs.

Conventions:

- Naming: `<topic>.md` (no date prefix; research is per-topic, not per-session — different from designs).
- Format: free-form prose. No required sign-off section because the artifact is informational, not a decision-gate.
- Cross-linking: design records cite research files via relative path (e.g., `docs/research/seaorm_2_0_migration.md`). Multiple designs can cite the same research.
- Lifecycle: created as needed; not signed off; can stay indefinitely or be deleted when superseded. No archive convention (volume is low, churn is low).
- Document history rule: applies (these are durable narrative). Both existing files already have history sections — keep that pattern.

When research outgrows itself: if a research file accumulates explicit decisions (a "Decision:" section, recommended path, sign-off-style conclusions), that's a signal it's actually a design record and should move to `docs/designs/`. The two existing research files were evaluated for this; both correctly stay in research. The `seaorm_2_0_migration.md` file's "Decision: stay schema-first" is a research conclusion that the corresponding design record (`entity-codegen-strategy.md`) adopts — it's not the project's primary decision record.

Default rule for new research: live in `docs/drafts/` during exploration. After: either get distilled into a design's "Decision Drivers" / "Considered Options" section, **or** — if the material is long enough to live alongside (multi-thousand words), durable enough to outlive the single design that prompted it, or has standalone informational value — promoted to `docs/research/<topic>.md`. The second path is the right answer when the research will inform multiple future designs.

- [x] Approved — new additions
- [ ] Needs discussion
- [ ] Skip

---

## 7. Effects on existing conventions

### 7.1 Document history rule (`CLAUDE.md` → "Documentation history")

The current rule says any AI-authored PR that changes a `docs/` file body must append a dated bullet to a `## Document history` section.

After this restructure:

- The rule still applies to `design.md`, `api-contract.md`, `compliance-plan.md`, and files in `coding-standards/`. These are durable narrative docs where history adds value.
- The rule **does not need to apply to ADR files** in `docs/decisions/`. Each ADR has a `date` field in frontmatter and is intrinsically a historical record; a "history" section inside an ADR is meta-recursion. If an ADR's decision later changes, the right move is to mark the original `superseded` and create a new ADR — not to edit and append history.
- The rule **does not apply to `roadmap.md`** by default. It's task tracking; history lives in Issues and the Project board.
- The rule **does apply to `data-model.md` and `workflows.md`** — they're durable narrative.

Net change to the CLAUDE.md rule text: add a one-line carve-out for `decisions/` and `roadmap.md`.

### 7.2 Prelaunch schema change rule

Unaffected. Still lives in `CLAUDE.md` (or `backend/CLAUDE.md` after § 6.2).

### 7.3 Two-assistant workflow

Unaffected. The handoff file pattern (`cowork-handoff.md` ↔ `claude-code-handoff.md`) is well-aligned with published Claude Code multi-agent workflow guidance ([MindStudio — 5 Claude Code Workflow Patterns](https://www.mindstudio.ai/blog/claude-code-agentic-workflow-patterns); [Claude Cowork Multi-Agent Orchestration](https://fast.io/resources/claude-cowork-multi-agent-orchestration/)). No change recommended.

- [x] Approved — convention effects
- [ ] Needs discussion
- [ ] Skip

---

## 8. Design records, drafts, and task handoffs

This section establishes the going-forward lifecycle for design conversations and tasks: where in-flight work lives, where finalized design records live, how decisions cross-link to ADRs and code, how executable task work is handed off, and what happens to the legacy `reviews/` directory.

This is a substantial revision of the original §8 (which was scoped narrowly to the `reviews/` directory). The new scope reflects four decisions made during the 2026-05-04 Cowork session: gitignored drafts with a `WIP_` checkpoint exception, `docs/designs/` replacing `reviews/design/`, GitHub Issues + Project as the task-handoff medium replacing `cowork-handoff.md` for executable work, and PR reviews living on GitHub only.

### 8.1 Lifecycle of a design decision

End-to-end flow:

```
docs/drafts/topic.md             ← gitignored work-in-progress
        ↓ (sign-off complete)
docs/designs/YYYY-MM-DD-topic.md ← tracked, durable record
        ↓ (decisions distilled)
docs/decisions/NNNN-*.md         ← MADR ADRs, cross-linked back to design
        ↓ (implementation plan decomposed)
GitHub Issues + Project          ← executable work units
        ↓ (work merges)
PRs reference ADRs               ← Closes #NN ties Issues to commits
        ↓ (eventually)
docs/designs/archive/            ← archived after sign-off + merged + 3 months
                                   (filenames keep the YYYY-MM-DD prefix)
```

The design record stays in place after sign-off and serves as the durable record of how the decision was reached. The ADR carries the canonical decision. Issues carry the executable work. The PR carries the change.

### 8.2 `docs/drafts/` — gitignored in-flight work

Working drafts live in `docs/drafts/`, which is gitignored by default with a `WIP_` prefix exception:

```gitignore
# .gitignore
docs/drafts/*
!docs/drafts/WIP_*.md
!docs/drafts/.gitkeep
```

Behavior:

- **Default:** Drafts in `docs/drafts/topic.md` don't appear in `git status` and don't pollute clean clones. They persist on disk across sessions because gitignore doesn't unlink — the file system just hides them from git.
- **Checkpoint via `WIP_` prefix:** When you need to save state (cross-machine sync, branch switching, or just "save my place"), copy `docs/drafts/topic.md` to `docs/drafts/WIP_topic.md`. The `WIP_` filename is checked in via the gitignore exception.
- **`WIP_` files stay static after commit.** Resume work by copying the WIP file back to `docs/drafts/topic.md` and iterating there. When ready to checkpoint again, overwrite the `WIP_` file from drafts.
- **Promotion:** When sign-off completes, rename `docs/drafts/topic.md` → `docs/designs/YYYY-MM-DD-topic.md` and delete the matching `WIP_topic.md` if any.
- **Abandonment:** Just delete from `drafts/`. Nothing in git history.

Edge cases:

- **`WIP_` divergence across branches.** If branches A and B both have a `WIP_topic.md` from divergent edits, ask Brendan case-by-case which to keep.
- **`git clean -fdx` risk.** Aggressive cleanup commands wipe gitignored files. Worth a one-line warning in `CLAUDE.md` so anyone running cleanup commands doesn't accidentally trash drafts.

### 8.3 `docs/designs/` — tracked design records

Replaces the legacy `reviews/design/` directory. Single artifact type called a **design record**. No formal plan-vs-design distinction — design records can include implementation plans (see §8.5) when actionable.

Conventions:

- Naming: `YYYY-MM-DD-kebab-case-topic.md`.
- Format: numbered sections with checkbox sign-offs (`- [ ] Approved / - [ ] Needs discussion / - [ ] Skip`). Established convention; unchanged.
- Each design record's sign-off summary section lists ADRs spawned (see §8.4).

### 8.4 ADR cross-linking

- **Each ADR has a `Source:` line** in its frontmatter or context section pointing to its parent design record: `Source: docs/designs/2026-05-04-design-doc-restructure.md`. ADRs from informal conversation say `Source: ad-hoc`.
- **Each design record's sign-off summary lists ADRs spawned**, once known: "ADRs produced: 0042, 0043." Marked `TBD` while implementation hasn't yet happened.
- **Code references the ADR**, not the design: `// See docs/decisions/0007-h2h-derivation.md`.
- **Implementation PR descriptions reference the ADR**, not the design. Readers want the decision, not the conversation. The ADR carries the link back to the design for those who want full history.

### 8.5 Implementation plan section

A design record with actionable implementation steps puts them under a heading literally titled `## Implementation plan`. This makes them easy for Claude Code (or a subagent generating Issues) to find via grep.

Optional autonomy framing for high-trust implementation plans:

```markdown
## Implementation plan (high-autonomy)

Claude Code: work through items 1–N in order. Don't check in between
items unless blocked. Acceptable to skip ## discussion below if path is clear.
Definition of blocked: <explicit list>
```

The implementation plan section is what feeds Issue creation in §8.6.

### 8.6 GitHub Issues + Project as task handoff

For executable task work (build this, refactor that, fix this bug), the medium is GitHub Issues + a Project board, not in-repo handoff files.

Mechanics:

- Phases → GitHub **milestones** (e.g., `Star: Sessions & Run Recording`). See §12 for the cup-name convention.
- Implementation plan items → GitHub **Issues** within the relevant milestone.
- A single GitHub **Project** board: `Backlog → Ready → In progress → In review → Done`.
- PRs reference Issues via `Closes #NN` to auto-close on merge.
- Issue creation uses the model-tiered approach per §4.4: Cowork drafts one canonical Issue, then spawns a Haiku-tier subagent for the rest.
- When GitHub MCP is authenticated, Cowork creates Issues directly. Until then, the subagent emits `gh issue create` commands and Claude Code executes them in batch.

This replaces `.claude/cowork-handoff.md` for *task* work — the existing handoff-file mechanic is overkill once a task has structured inputs (title, scope, acceptance criteria) that fit naturally into an Issue.

### 8.7 Handoff files: narrowed to non-task communication

`.claude/cowork-handoff.md` ↔ `.claude/claude-code-handoff.md` are retained, but their scope narrows to *non-task* communication:

- Research requests ("look into X and report back").
- Design questions or discussion that aren't yet structured enough to be an Issue.
- Ad-hoc reviews not tied to a specific Issue.
- Anything one assistant needs from the other that doesn't fit the Issue shape.

The "writer creates, reader deletes when done" convention from `CLAUDE.md`'s Development Workflow section stays unchanged. Just narrower in what flows through it.

### 8.8 PR reviews live on GitHub

`reviews/pr/` is **deleted** as part of the migration in §8.11. PR review feedback going forward lives entirely on GitHub:

- New PR reviews are posted via `gh pr review` (or the GitHub MCP equivalent), with **line-anchored comments** wherever feedback is line-specific. Bulk-comment summaries on the whole PR are a worse experience for both humans and AI reviewers.
- The `engineering:code-review` skill needs updating: all feedback (any importance level) goes to the PR as comments. The skill no longer writes to a file. PR comments are the durable artifact.
- GitHub preserves PR comments indefinitely, so the storage durability concern is satisfied.

Trade-off accepted: PR reviews are no longer grep-able locally. Searching old reviews uses GitHub search.

The `CLAUDE.md` "who does what" table updates from `PR reviews | Claude Code (writes to reviews/pr/)` to `PR reviews | Claude Code (posts as PR comment via gh pr review)`.

### 8.9 Asset files (mockups, viewers, screenshots)

Non-`.md` artifacts produced during a design session (mockup `.jsx`, viewer `.html`, screenshots) live alongside their parent design record:

- During drafting: `docs/drafts/assets/<topic>/`.
- After promotion: `docs/designs/assets/<topic>/`. Migrate as part of the design record's promotion step.
- The parent design record names the assets it depends on.

Existing assets in `reviews/design/` are triaged during migration (see §8.11).

### 8.10 Archive convention

> **Superseded 2026-05-15.** Live convention lives in [`../../CLAUDE.md`](../../CLAUDE.md) § Design records → Archive. The 3-month wait described below was retired, and the scope was broadened to admit non-design-record artifacts (e.g., the compliance plan) alongside MADR-shaped records.

To keep `docs/designs/` scannable as the project ages, archive design records once durably resolved.

A design record is eligible for archive when **all three** are true:

1. Sign-off complete (all checkboxes Approved or Skip).
2. Implementation merged.
3. At least 3 months have passed since (2).

When eligible, move to `docs/designs/archive/`. Filenames already carry a `YYYY-MM-DD` prefix, so no per-year subdirectories are needed at current scale — flat directory with date-prefixed filenames sorts chronologically. If volume ever grows enough to make scanning the archive painful, we can split into per-year subdirs as a follow-up. Files remain in git history; only their location changes. Active directory stays focused on what's still in motion.

The deeper question — whether design records should live in main at all, vs. an orphan branch (e.g., `refs/designs`) accessed via `git worktree`, vs. a separate repo — is **deferred**. Triggers to revisit:

- Active `docs/designs/` exceeds ~50 files.
- Records start feeling intrusive in clean clones (e.g., when sharing the repo).
- Two-assistant workflow friction makes them a coordination problem.

At current scale, the gitignored-drafts + `docs/designs/` + archive convention is sufficient. The orphan-branch path adds workflow complexity that's hard to justify until the cognitive-load problem is real.

### 8.11 Migration of existing `reviews/`

The `reviews/` directory disappears entirely as part of PR 1 (per §9). Migration steps:

1. **Triage `reviews/design/*.md`:**
   - For each existing record, decide: still relevant or stale?
   - **Keepers** → move to `docs/designs/YYYY-MM-DD-topic.md` (date prefix preserved from the existing filename, or recovered via `git log --diff-filter=A` if absent).
   - **Stale** → delete. Examples likely to be stale: phase plans whose phase is complete (`2026-04-02-phase2-plan.md`), one-off mockup critiques where the implementation has shipped.
2. **Triage `reviews/design/` assets** (mockup `.jsx`, viewer `.html`):
   - Keepers → move to `docs/designs/assets/<topic>/` next to the parent design record.
   - Stale → delete.
3. **Triage `reviews/pr/*.md`:**
   - Scan each existing file for insights worth surfacing elsewhere (a code-review observation that should become an ADR, a refactoring idea worth tracking as an Issue).
   - After triage, **delete all `reviews/pr/` files**. They're going-forward replaced by GitHub comments per §8.8.
4. **Delete the empty `reviews/` directory.**

Initial triage candidates from current state (Brendan should approve outcomes before deletions land):

- `2026-04-15-rust-audit.md` — likely keeper. If audit fully implemented, archive directly to `docs/designs/archive/`. If still in flight, promote to active `docs/designs/`.
- `2026-04-19-pending-races-and-grace-period.md` — likely keeper; durable architectural decisions.
- `2026-05-02-entity-codegen-strategy.md` — likely keeper.
- `2026-05-02-sessions-created-by-removal.md` — likely keeper (signed off; implementation status to confirm).
- `2026-05-04-design-doc-restructure.md` — **this very record**. Promotes to `docs/designs/2026-05-04-design-doc-restructure.md` once signed off.
- `2026-04-01-design-consistency-review.md` — assess relevance.
- `2026-04-02-phase2-plan.md` — Phase 2 is done; likely stale. Absorb any still-relevant points into `roadmap.md`, then delete.
- `2026-04-02-phase3-breakdown-and-session-ux.md` — Phase 3 mostly done; similar handling.
- `2026-04-04-session-screen-mockup-critique.md` — assess relevance; mockup work largely complete.
- `pr-3c2-run-entry-handoff-draft.md` (in `reviews/pr/`) — read for insights, then delete.
- `session-screen-mockup.jsx` / `session-screen-mockup-viewer.html` — assess as assets; if still useful as reference, move to `docs/designs/assets/session-screen-mockup/`; otherwise delete.

### 8.12 Where these conventions live going forward

The conventions in this section get codified in `docs/CLAUDE.md` (per §6.2) so future Cowork or Claude Code sessions working in `docs/` see them without re-reading this entire design record. See §6.2 for the specific list of items that move into `docs/CLAUDE.md`.

### 8.13 Why this is bundled in this design record

Introducing ADRs, GitHub Issues, and gitignored drafts together creates a coordinated lifecycle question that this rework directly produces. Solving it in a separate design record would create two records that have to stay in sync, which defeats the purpose. The tradeoff is that §8 carries more weight than other sections — accepted, given the cohesion.

- [x] Approved — design records, drafts, and task handoffs (REWRITTEN; previous approval reset)
- [ ] Needs discussion
- [ ] Skip

---

## 9. Implementation sequencing

Each step leaves the repo in a working state. Steps can be paused between PRs without leaving anything broken.

**PR 1 — Skeleton + biggest extraction (Cowork drafts; Claude Code commits).**
- Create `docs/decisions/` with `README.md` and `template.md` (template includes the `Source:` line per §8.2).
- Create `docs/README.md` index.
- Add `.github/pull_request_template.md` with the index-update checkbox per §10.1.
- Add `.github/workflows/link-check.yml` running `lychee-action` per §10.4.
- Add `docs/CLAUDE.md` from `WIP_pr1-docs-claude.md`.
- Add `docs/workflow.md` from `WIP_pr1-workflow.md`.
- Add `.github/ISSUE_TEMPLATE/feature.md` and `.github/ISSUE_TEMPLATE/bug.md` from their respective WIP drafts.
- Update `CLAUDE.md`'s "who does what" table per §8.3: `PR reviews | Claude Code (posts as PR comment via gh pr comment)`.
- Move Data Model → `docs/data-model.md`.
- Replace the section in `design.md` with a one-paragraph summary + link.
- Update the Document history section in `design.md`.

**Prerequisites already in `main`** (from the 2026-05-05 handoff-convention move): `docs/handoffs/README.md`, `.gitignore` rules for `docs/handoffs/*`, removal of `.claude/*-handoff.md` rules. Don't re-do these.

**PR 2 — Resolved Decisions → MADR files.**
- One ADR per existing bullet, ~35 files.
- Use the model-tiered approach per §3.6: Cowork drafts one canonical ADR by hand, then spawns a Haiku-tier subagent to draft the rest from the remaining bullets. Cowork reviews and rewrites the few decisions that warrant richer treatment (auth strategy, H2H derivation, photo enforcement).
- Replace the Resolved Decisions section in `design.md` with a one-line pointer to `docs/decisions/`.
- Update `decisions/README.md` index.

**PR 3 — Roadmap split.**
- Move Build Plan → `docs/roadmap.md` (narrative parts).
- Brendan creates the GitHub Project board manually.
- Issues for open Phase 3 work created via the model-tiered approach per §4.4: Cowork drafts one canonical Issue, spawns a Haiku-tier subagent for the rest. If GitHub MCP is authenticated, the subagent uses MCP tools directly; otherwise it emits `gh issue create` commands for Claude Code to execute in batch.
- Replace Build Plan section in `design.md` with a pointer.

**PR 4 — API + Workflows merger.**
- Merge API Surface into `docs/api-contract.md`.
- Merge User Workflows + UI Screens into `docs/workflows.md`.
- Remove sections from `design.md`.

**PR 5 — Final trim and root cleanup.**
- Move Project Structure → repo-root `README.md`.
- Final trim of `design.md` to ~250 lines.
- Update cross-references across all docs.

**PR 6 (optional) — Nested CLAUDE.md.**
- Create `backend/CLAUDE.md` and `frontend/CLAUDE.md` per §6.2.
- Trim root `CLAUDE.md` accordingly.
- Update the Document history rule's carve-out (§ 7.1).

Steps 1–5 are mechanical content moves. Step 6 requires a judgment call about which root-CLAUDE.md content is genuinely cross-cutting vs. area-specific.

- [x] Approved — sequencing
- [ ] Needs discussion
- [ ] Skip

---

## 10. Risks

### 10.1 Index drift

A `docs/README.md` index that goes stale is worse than no index. Mitigations:

- Keep it short (under one screen). Long indexes get skipped by both humans and agents.
- **PR template checklist.** Add a `[ ] Updated docs/README.md if I added, moved, or renamed a doc` line to the GitHub PR template (`.github/pull_request_template.md`). Visible at PR creation to both humans and AI assistants. Cheap, no tooling required.

### 10.2 Subagent context underspecification

If a subagent gets pointed at a small file but really needed two more files, the split has actively hurt. Mitigations:

- **Name files explicitly when delegating.** Use the `docs/README.md` task map to identify the canonical set, e.g. "read `data-model.md` and `decisions/0007-h2h-derivation.md`."
- **Hand the agent the index.** Include `docs/README.md` in the subagent's prompt so it can pull additional files if its task expands. Costs ~30 lines of context; transfers the load-more decision to the agent.
- Tell subagents to ask before assuming when they suspect they're missing context. Costs a turn occasionally; avoids silent failures.

### 10.3 ADR proliferation

35 ADRs is fine. 350 is not. Mitigations:

- **High-bar rule in `decisions/README.md`.** "An ADR is for a non-obvious choice with tradeoffs that a future contributor will revisit. Not for: routine library upgrades, naming choices, or dependency bumps."
- **Combine related decisions when natural.** Don't be dogmatic about one-decision-per-file. Auth strategy can be a single ADR covering algorithm + token model + rotation rather than three.
- **Future option (not adopted now): periodic prune.** Mark stale ADRs `superseded` or `deprecated` during scheduled cleanups. Worth revisiting if the count exceeds ~75. Adopting it now would add maintenance burden without solving a current problem.

### 10.4 Cross-link rot

Splitting creates link maintenance. Mitigations:

- **Use relative paths consistently** (`../decisions/0007-...`).
- **CI link check via [`lychee-action`](https://github.com/lycheeverse/lychee-action).** Runs on every PR; catches broken internal and external links. Fast (Rust-based) and configurable; adds roughly 30 seconds to CI. If transient external URLs cause false positives, the action's config supports skip-list patterns.

### 10.5 GitHub Issues vs roadmap.md duplication

If both list the same items, they'll drift. Mitigations:

- **Strict role separation.** `roadmap.md` describes phases (goals, scope, success criteria, what's deferred). Issues track tasks. Don't restate the issue list in the roadmap.
- **Roadmap embeds a live milestone URL** for current status, e.g., `Open Phase 3 work: [filter](https://github.com/<owner>/beerio-kart/issues?q=is%3Aopen+milestone%3A%22Phase+3%22)`. Readers click through to the live source of truth — no copy in markdown.

- [x] Approved — risks acknowledged (mitigations selected)
- [ ] Needs discussion
- [ ] Skip

---

## 11. What this record does not cover

- **Whether to retire any specific ADR or merge several** — that's per-ADR judgment during PR 2.
- **The exact wording of trimmed `design.md` sections** — drafts go in PR 5.
- **Frontend `CLAUDE.md` content** — frontend conventions haven't been documented in depth yet; this can stay a stub initially.
- **Migration off `compliance-plan.md`** — out of scope. That doc has its own life.
- **Whether to add additional skills (e.g. a `data-model-review` skill)** — possible follow-up after the structure settles. Not blocking.

- [x] Approved — scope
- [ ] Needs discussion
- [ ] Skip

---

## 12. Amendment 2026-05-05: Cup-name milestones

Added after the original sign-off. Adopts Mario Kart 8 Deluxe cup names as the canonical milestone-naming convention for the build-phase namespace, replacing "Phase N" labels. Resolves the three-way "Phase X" collision documented in `cowork-notes.md`: build phases, `docs/compliance-plan.md`'s Phase A–J, and the WIP CI-adoption draft's Phase A–D.

### 12.1 Convention

Milestones use MK8 Deluxe cup names, claimed in chronological **start order** — the Nth major work-chunk gets the Nth cup. No semantic mapping; cup names are arbitrary chronological labels, not categories. Closed milestones keep their cup name forever (closed cups are project history, not slots to recycle).

Title format: `<CupName>: <Description>` — e.g., `Star: Sessions & Run Recording`. The cup name is the stable identifier; the description is the human-readable theme and may evolve while the milestone is open if the chunk's theme tightens.

Cup pool (20 total):

- **Base 8 (MK8 originals):** Mushroom, Flower, Star, Special, Shell, Banana, Leaf, Lightning.
- **MK8 Deluxe additions:** Crossing, Bell, Egg, Triforce.
- **Booster Course Pass:** Golden Dash, Lucky Cat, Turnip, Propeller, Rock, Moon, Fruit, Boomerang.

20 cups is enough for any plausible Beerio Kart lifetime.

### 12.2 Initial cup mapping

| Cup | Work chunk | Status as of 2026-05-05 |
|-----|------------|--------------------------|
| Mushroom | Foundation (was Phase 1) | Closed |
| Flower | Deployment (was Phase 2) | Closed |
| Star | Sessions & Run Recording (was Phase 3) | Open, in progress |
| Special | Documentation overhaul (this design record + PRs 1–6) | Open, in progress |
| Shell | Session Rulesets (was Phase 4) | Future |
| Banana | Stats & Leaderboards (was Phase 5) | Future |
| Leaf | Social & Head-to-Head (was Phase 6) | Future |
| Lightning | (next thing — TBD) | Reserved (do not create yet) |

OCR (was Phase 7) is **not** assigned a cup yet — too speculative; defer milestone creation until the work is next-up. Reserving Lightning empty preserves chronological-claim ordering.

### 12.3 Affected sections of this record

In-place wording fixes applied at promotion time (when this record moves to `docs/designs/` in PR 1):

- **§4.2** — example milestone name `Phase 3: Sessions & Run Recording` becomes `Star: Sessions & Run Recording`. Add a sentence pointing readers to §12 for the cup convention.
- **§8.6** — same example replacement, same pointer.

§12 itself is the durable record of *when* and *why* the cup convention was adopted. The in-place edits make §4.2 and §8.6 self-consistent for future readers; §12 stays as the changelog entry.

### 12.4 Why cup names

- The repo currently has three independent "Phase X" namespaces. Cup names exit that collision in one move and reserve "Phase" for unambiguous build-phase use only (after the renames in §12.5 land).
- On-brand for a Mario Kart project.
- Arbitrary-chronological-claim avoids the failure mode where every new cup needs a "right" semantic theme — no decision overhead per claim.

### 12.5 Follow-up work created by this amendment

Each item below is tracked here so nothing falls through. Most are small.

1. **Create milestones on GitHub** — Cowork-only task via the GitHub MCP. Open milestones for Star, Special, Shell, Banana, Leaf. Closed milestones for Mushroom and Flower with their actual completion dates (Mushroom = ?, Flower = ?, dates TBD). Lightning **not** created yet — empty reserved milestones clutter the milestone list. Pre-PR-1 work; can run anytime after this amendment is signed off.
2. **Rename "Phase A–J" in `docs/compliance-plan.md`** to "Stream A–J". Preserves "Phase" exclusively for build-phase use. Standalone small Cowork PR (the file isn't already in PR 1's scope).
3. **Rename "Phase A–D" in `docs/drafts/WIP_pr1-design-ci-adoption.md`** to "Stream A–D". This file is being installed as `docs/designs/2026-05-04-ci-adoption.md` in PR 1 — apply the rename in the installed version.
4. **Add a cup-mapping table to `docs/roadmap.md`** when that file is created in PR 3. The §12.2 mapping above is the seed; PR 3 can extend with target dates and short scope summaries.
5. **Update `docs/design.md`'s phase-list (Build Plan section)** to reference cup names alongside (eventually replacing) "Phase N" labels. Touch when in the file for other reasons; not urgent on its own.
6. **Add the milestone-naming convention to root `.claude/CLAUDE.md`** under § GitHub access. The in-flight CLAUDE.md handoff PR (`docs/handoffs/cowork-handoff.md`, Edit B) has been updated to include this paragraph; that's the durable home for the rule once the PR lands.
7. **Add Priority and Estimate custom fields to the GitHub Project board** — Brendan-manual since the Composio MCP can't create Project custom fields. Quick UI task; document the new field IDs in `docs/project-field-ids.md` after creation.

### 12.6 Sign-off

- [x] Approved — cup-name milestones
- [ ] Needs discussion
- [ ] Skip

(The "All sections approved — clear to begin PR 1" checkbox below is preserved as-is; this amendment is independent of the original sign-off and PR 1 is not blocked on it. PR 1 may proceed once the amendment is approved or skipped, whichever comes first.)

---

## 13. Amendment 2026-05-05 (later): PR 1 scope additions

Added during the same Cowork session that produced §12. Captures additions to PR 1's deliverables list that weren't in the original §9 sequencing.

### 13.1 Additions

PR 1 picks up these new files, drafted in `docs/drafts/` as `WIP_pr1-*` per the existing convention:

- **`docs/workflow.md`** (drafted as `WIP_pr1-workflow.md`) — operational guide covering the decision tree, Issue lifecycle, milestone lifecycle, PR conventions, triage, and multi-assistant coordination. Decisions worked through across the 2026-05-05 design session (D1-D3 / I1-I5 / M1-M3 / P1-P2 / T1-T2 / C1-C3).
- **`.github/ISSUE_TEMPLATE/feature.md`** (drafted as `WIP_pr1-issue-template-feature.md`) — feature/enhancement Issue template using the I1 body shape.
- **`.github/ISSUE_TEMPLATE/bug.md`** (drafted as `WIP_pr1-issue-template-bug.md`) — bug Issue template (What happened / What you expected / How to reproduce / Acceptance criteria / References).

### 13.2 Already in `main` from this same session

These landed during the session-handoff convention move and are out of PR 1's scope (already done). §9 should reference them in a "prerequisites already in main" note, not duplicate the work.

- `docs/handoffs/README.md` — convention documentation, tracked.
- `docs/handoffs/*` gitignore exception (`!docs/handoffs/README.md`).
- `.gitignore` removal of the stale `.claude/*-handoff.md` rule.

### 13.3 §9 (Implementation sequencing) amendments

The PR 1 and PR 6 bullet lists in §9 are amended (changes applied at promotion time when the design record moves to `docs/designs/` in PR 1):

- **PR 1 — Add** after `Add .github/workflows/link-check.yml ...`:
  - `Add docs/CLAUDE.md from WIP_pr1-docs-claude.md.` (Clarifying — implicitly part of PR 1 via the `WIP_pr1-` naming convention but not listed in the original §9.)
  - `Add docs/workflow.md from WIP_pr1-workflow.md.`
  - `Add .github/ISSUE_TEMPLATE/feature.md and .github/ISSUE_TEMPLATE/bug.md from their respective WIP drafts.`
- **PR 1 — Add** a "Prerequisites already in main" sub-list noting the handoff-convention files per §13.2.
- **PR 6 — Fix.** Strike "and `docs/CLAUDE.md`" from the first PR 6 bullet so it reads `Create backend/CLAUDE.md and frontend/CLAUDE.md per §6.2.` Reason: PR 1 creates `docs/CLAUDE.md`; the PR 6 reference was vestigial from an earlier draft.

### 13.4 Other small things from this session worth flagging

These don't change the design record's plan — they're follow-ups already captured in §12.5 or already done:

- **PR template content overhaul** — `WIP_pr1-github-pr-template.md` was rewritten to be reviewer-action-oriented (Reviewer notes / Summary / Linked work / How to verify / Author checklist), replacing the original author-hygiene-only draft. Content change to a file already in PR 1's scope; the §9 bullet list doesn't need updating, just the file content does.
- **`WIP_pr1-design-ci-adoption.md` rename** — Phase A-F → Stream A-F (per §12.5 follow-up #3). Applied to the WIP draft; PR 1 installs the renamed version.
- **`docs/compliance-plan.md` rename** — Phase A-J → Stream A-J (per §12.5 follow-up #2). Applied directly in `main` via Cowork edit, since that file isn't in PR 1's scope; effectively a small standalone Cowork-only change. Note: the original §12.5 follow-up #2 said "standalone small Cowork PR" but Cowork can't open PRs; the change landed as a direct working-tree edit and Brendan or Claude Code commits it.
- **`docs/project-field-ids.md` updates** — refreshed via MCP to capture the 2026-05-05 UI changes (Todo→Backlog rename, Ready added, In Review never created, Priority field added with Low/Medium/High options, PR auto-add disabled).

### 13.5 Sign-off

- [x] Approved — PR 1 scope additions
- [ ] Needs discussion
- [ ] Skip

---

## Sign-off summary

When all eleven checkboxes above are Approved (or Skip with rationale), implementation can proceed via PRs 1–5 (PR 6 optional). Each PR gets its own description with a link back to this record.

- [x] **All sections approved — clear to begin PR 1**

---

## Appendix: Sources cited

- [Best practices for Claude Code](https://code.claude.com/docs/en/best-practices)
- [Builder.io — How to Write a Good CLAUDE.md](https://www.builder.io/blog/claude-md-guide)
- [DataCamp — Writing the Best CLAUDE.md](https://www.datacamp.com/tutorial/writing-the-best-claude-md)
- [Anatomy of the .claude/ Folder — Avi Chawla](https://blog.dailydoseofds.com/p/anatomy-of-the-claude-folder)
- [How to Structure .Claude/ Folder for Maximum Efficiency — Youssef Hosni](https://levelup.gitconnected.com/how-to-structure-claude-folder-for-maximum-efficiency-c26ef3f552ba)
- [Implementing CLAUDE.md and Agent Skills — Matthew Groff](https://www.groff.dev/blog/implementing-claude-md-agent-skills)
- [Salt Creative — From Monolithic Prompts to Modular Context](https://dev.to/salt_creative/from-monolithic-prompts-to-modular-context-a-practical-architecture-for-agent-memory-1lcp)
- [Modular and Hybrid Architecture for LLM Agents (paper)](https://openreview.net/pdf?id=gC3D2ESSyK)
- [Modularity — An Overrated Anti-Pattern? (steelman)](https://dev.to/embernoglow/modularity-an-overrated-anti-pattern-the-power-of-the-monolithic-script-in-the-age-of-ai-5oc)
- [Michael Nygard — Documenting Architecture Decisions (2011)](https://www.cognitect.com/blog/2011/11/15/documenting-architecture-decisions)
- [adr.github.io](https://adr.github.io/)
- [MADR — Markdown Any Decision Records](https://adr.github.io/madr/)
- [adr-agent](https://github.com/macromania/adr-agent)
- [MindStudio — 5 Claude Code Workflow Patterns](https://www.mindstudio.ai/blog/claude-code-agentic-workflow-patterns)
- [Claude Cowork Multi-Agent Orchestration — Fast.io](https://fast.io/resources/claude-cowork-multi-agent-orchestration/)

## Document history

- 2026-05-05 — Promoted from `reviews/design/` to `docs/designs/` as part of PR 1 (docs restructure foundation). In-place fixes from §12.3 / §13.3 applied at promotion: example milestone names in §4.2 and §8.6 changed from `Phase 3: Sessions & Run Recording` to `Star: Sessions & Run Recording`; three new §9 PR 1 bullets added; "Prerequisites already in `main`" note added; §9 PR 6 `docs/CLAUDE.md` line struck (file landed in PR 1). PR #41.
- 2026-05-05 — Struck the `Create reviews/README.md per §8.6` bullet from §9 PR 1 list. The bullet was internally inconsistent (§8.11 deletes `reviews/`; §8.6 isn't about a reviews README) and was a leftover from a pre-restructure draft. Surfaced by Claude Code's post-PR-1 handoff (item 5).
