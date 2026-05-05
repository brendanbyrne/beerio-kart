# Beerio Kart

## Start of Session
Read `docs/design.md` at the start of every session. It is the single source of truth for the project's design and reflects the full history of design decisions.

### Handoff files

Two handoff channels enable async task passing between assistants. The writer creates the file; the reader deletes it when done — the file's existence is the signal.

Handoff files live under `docs/handoffs/`. The directory is gitignored (handoffs are transient and shouldn't pollute git history); only `docs/handoffs/README.md` is checked in, and it documents the format and lifecycle for humans browsing the repo.

- **`docs/handoffs/cowork-handoff.md`** — Cowork → Claude Code. Check before starting work. Contains task instructions from the architecture/design assistant. Claude Code deletes after completing the work.
- **`docs/handoffs/claude-code-handoff.md`** — Claude Code → Cowork. Write this when you have questions, research requests, or design decisions for Cowork. Brendan or the next Claude Code session deletes after Cowork acknowledges in chat — Cowork's sandbox blocks `unlink()` repo-wide, so Cowork can't clean up its own inbox.

For task-specific handoffs that shouldn't collide with the canonical filenames, use a dated variant: `cowork-handoff-<YYYY-MM-DD>-<slug>.md` or `claude-code-handoff-<YYYY-MM-DD>-<slug>.md`.

**Anything intended for the other assistant to act on — task specs, code review findings, bug reports, design decisions, answers to their questions — MUST be written to the appropriate handoff file.** Delivering it only in chat means the other assistant won't see it. If you find yourself composing a substantive response that the other assistant needs, stop and write it to the handoff file instead.

**Do not use handoff files for your own session notes.** The handoff files are one-way channels between assistants — if the file exists, the recipient assumes there's work to do. For self-notes or session state you want to preserve across your own sessions, use `.claude/cowork-notes.md` (Cowork) or `.claude/claude-code-notes.md` (Claude Code).

**Why `docs/handoffs/` and not `.claude/`:** Cowork's `Write` and `Edit` tools refuse to write under `.claude/` — that path is protected at the Cowork tool layer to prevent the AI from silently rewriting its own context (CLAUDE.md, skills, settings). Bash writes still succeed, but routing routine handoffs through bash is awkward and obscures intent. `docs/handoffs/` keeps the protected zone protected and lets handoffs use the normal file tools.

## Project Phase
Star — Sessions & Run Recording (core gameplay loop). ("Phase 3" under the old naming. See § GitHub access → Milestone naming for the cup-name convention.)

## UI Reference Device
Use the **Pixel 9 Pro** as the reference phone for all UI mockups and layout work. Physical resolution: 1280 × 2856 pixels at 495 ppi. Logical (CSS) resolution: ~427 × 952 px at 3× device pixel ratio.

## Overview
Beerio Kart is a mobile-first web app for tracking times and stats for a Mario Kart 8 Deluxe drinking game. Players race time trials, optionally drink, and the app tracks personal bests, leaderboards, and run history. Non-drinkers are equally welcome — inclusive by default is a core design principle.

## Architecture at a Glance
React handles the UI, Vite serves it and proxies API calls, Axum handles the API, SeaORM (backed by sqlx) is the ORM, SQLite is the database (with a path to PostgreSQL later), and Tailwind handles the styling. Bun is used instead of npm for package management.

## Preferences
- Suggest better approaches when you see them, with reasoning and sources.
- Keep responses concise but explain the "why."
- Don't assume knowledge — Brendan has deep C++/Python experience but is new to web dev, databases, and Rust.
- If a command fails because it needs sudo or a password, **stop and ask Brendan** before trying alternative approaches. Don't attempt workarounds (e.g., installing via conda, pip, or other package managers) — just explain what's needed and let Brendan handle the installation.
- When introducing web/database concepts, explain them briefly.

## Repo Location
- **Single checkout:** `C:\Users\obiva\beerio-kart` (Windows), accessible from WSL2 at `/mnt/c/Users/obiva/beerio-kart`
- Both Cowork (Claude Desktop) and Claude Code (WSL2 CLI) work on this same checkout. No syncing needed.
- **Performance note:** WSL2 accessing `/mnt/c/` is slower than the native Linux filesystem, especially for `cargo build`. If build times become painful, configure Cargo to put build artifacts on the Linux filesystem while keeping source on Windows:
  ```toml
  # backend/.cargo/config.toml
  [build]
  target-dir = "/home/bbyrne/.cargo-target/beerio-kart"
  ```

## Conventions
- Use LF (`\n`) line endings, not CRLF (`\r\n`).
- Keep `.gitattributes` in the repo root. Only add nested ones if a subdirectory needs genuinely distinct Git behavior (e.g., LFS for large assets).
- Database naming: Tables plural snake_case, columns snake_case, FKs `{singular}_id`, PKs `id`.
- Rust style: Follow standard `rustfmt` and `clippy` conventions.
- Frontend style: TypeScript, functional React components, Tailwind for styling.
- Drafts in `docs/drafts/` are gitignored except for `WIP_*.md` files. Aggressive cleanup commands (`git clean -fdx`) will wipe them — check `docs/drafts/` before running them.

### Schema changes (prelaunch)

While the project is prelaunch, **all schema lives in a single consolidated migration file**. New schema work edits that file rather than appending a new one. Rationale: pre-launch we don't preserve dev data, so the append-only history that migrations normally provide isn't earning its keep — it's just N files where 1 would do.

Operating rules:

- **Edit, don't append.** Adding a table, column, index, or constraint means modifying the consolidated migration file (currently `backend/migration/src/m20260101_000001_initial_schema.rs` or whatever name we settle on after the squash). Do not create a new migration file.
- **Reset the dev DB after schema edits.** Delete the local SQLite file (or run the project's `dev-reset` task if/when one exists) before booting. SeaORM will recreate the schema from the consolidated migration on next startup.
- **No data preservation between schema versions.** If you have meaningful local test data, recreate it via seed or test fixtures after the reset, not by hand.
- **Code that depends on schema must change in the same PR as the migration edit.** Entities, services, tests — all in one atomic commit.

When we exit prelaunch (decided when we have real user data we don't want to lose), this convention flips back to standard append-only migrations: every schema change becomes a new file, and the consolidated initial migration becomes the immutable starting point. CLAUDE.md will be updated at that time.

### Documentation history

**Scope:** files in `docs/` only (`design.md`, `api-contract.md`, `compliance-plan.md`, and every file in `coding-standards/`). Files under `.claude/` are *not* covered — those describe current behavior, not its history, and don't carry a history section.

Each in-scope file keeps a `## Document history` section at the bottom. Any AI-authored PR (Cowork or Claude Code) that changes the body of one of these files **must append a dated bullet to that section in the same PR**. Brendan is not bound by this rule.

- **Format:** `- YYYY-MM-DD — <one-line summary of the change, with PR # if applicable>`. Use absolute ISO dates, never "today" or "last week."
- **What requires an entry:** Adding, removing, or rewording a rule; restructuring sections; marking a previously-deferred item as done; changing rationale or sources. Pure typo or formatting fixes do not.
- **When the previous entry foreshadowed this change** (e.g., "Noted upcoming X removal"), the new entry must explicitly close the loop ("Removed X. PR #N.") so a future reader can see the upcoming/completed pair.
- **Why:** The history is the durable record of why a `docs/` rule says what it says. Without an entry a reader can't tell whether a rule has stood for a year or was added yesterday, and a reviewer can't audit whether a referenced "upcoming" change was ever completed.

## Testing
**Tests are a deliverable, not optional.** Every PR that adds business logic must include tests. PRs should not be opened without them.

- **Unit tests:** Use `#[cfg(test)] mod tests { }` in the same file as the code being tested. Cover business logic: validation rules, service functions, data transformations, error cases.
- **Integration tests:** Use `tests/` directory or Axum's test utilities to test HTTP endpoints end-to-end. Cover the happy path and key error cases (bad input, auth failures, not found, conflicts).
- **Verification tests:** Drift checks that exercise structural invariants between layers, not feature behavior. They live in `tests/` like integration tests but their contract is "two layers must stay in sync," not "this endpoint returns the right value." First instance: [`tests/schema_drift.rs`](../../backend/tests/schema_drift.rs) — verifies every entity in `backend/src/entities/` can `SELECT` its declared columns from the freshly-migrated schema. Add a verification test whenever a class of cross-layer drift is hard to catch by review alone (the schema-drift case used to be caught implicitly by codegen output diffing — once entities became hand-written committed source per `seaorm.md` § 6, the implicit signal was gone and an explicit test took its place).
- **What doesn't need tests:** Hand-written entities (declarations of column shape — no testable logic to unit-test; the schema-drift verification test covers mismatches between migration and entity), `mod.rs` re-exports, one-time startup code (seeding, migration runner), and simple config loading. Use judgment — if it has logic, it needs tests.
- **Test naming:** Descriptive names that read as sentences: `test_login_with_wrong_password_returns_401`, not `test_login_2`.

## Development Workflow

### Two-assistant setup

This project uses two AI environments:

- **Cowork (Claude Desktop):** Design, architecture, documentation, research, review. Accesses the repo via a Windows mount (`C:\Users\obiva\beerio-kart`). Cannot access WSL2 filesystem and **cannot run git commands** — the Cowork sandbox mounts the repo via virtiofs with `unlink()` blocked at the mount layer (every file delete returns `EPERM`, including files Cowork just created). Git relies on creating and removing `.git/index.lock` and temp objects, so any git invocation from Cowork either fails outright or leaves a stale lock that breaks the next attempt. Cowork edits files only. For GitHub API operations (issues, PRs, project board) see § GitHub access below.
- **Claude Code (WSL2 CLI):** Coding, building, testing, git operations. Accesses the same checkout via `/mnt/c/Users/obiva/beerio-kart/`. WSL2's `/mnt/c` (9P/DrvFs) supports unlink, so git works fine there.

### GitHub access

Cowork can read and write GitHub data — issues, pull requests, project board items, milestones — through Composio's GitHub MCP connector. The connection authenticates as the GitHub user `brendanbyrne`. Anything Cowork does via the MCP appears in the GitHub UI under that account.

**What Cowork can do via the MCP:**

- File, label, assign, triage, and close issues; add comments.
- Read PR diffs, conversation threads, and review state. (Creating commits or PRs still requires Claude Code — that's a git operation, not an API operation.)
- Add items to the project board, move them between Status columns, set custom field values, attach milestones.
- Create and manage milestones.
- Run arbitrary GraphQL queries against `api.github.com/graphql` when no purpose-built tool exists.

**What the MCP cannot do, regardless of which assistant calls it:**

- **Run `git`.** The MCP only talks to GitHub's API. Branch creation, commits, pushes, and merges remain Claude Code's job.
- **Create or edit project custom fields, status field options (board columns), views, or built-in workflows.** GitHub's API does not expose these — they are configured in the project Settings UI only.
- **Set Assignees, Labels, Milestone, or Repository via the project field mutation.** Those are properties of the underlying issue/PR; use the issue/PR mutations instead.

**Field IDs reference:** `docs/project-field-ids.md` caches the project's field IDs and Status option IDs. Consult that file before issuing project-board write calls — Composio's tools require IDs, not names. Update the file if anyone changes the project's fields in the GitHub UI.

**Milestone naming:** Project milestones use Mario Kart 8 Deluxe cup names (Mushroom, Flower, Star, Special, Shell, Banana, Leaf, Lightning, plus Crossing, Bell, Egg, Triforce, plus the eight Booster Course Pass cups: Golden Dash, Lucky Cat, Turnip, Propeller, Rock, Moon, Fruit, Boomerang), claimed in chronological start order — the Nth major work-chunk gets the Nth cup. No semantic mapping; cup names are arbitrary chronological labels. Closed milestones keep their cup name forever. Title format: `<CupName>: <Description>` (e.g., `Star: Sessions & Run Recording`). Rationale: avoids the three-way "Phase X" collision (build phases, `docs/compliance-plan.md`'s Phase A–J, the WIP CI-adoption draft's Phase A–D). The current cup-to-work-chunk mapping lives in the design record's 2026-05-05 amendment (`reviews/design/2026-05-04-design-doc-restructure.md` §12) until PR 3 creates `docs/roadmap.md`.

**When to use Cowork vs. Claude Code for GitHub work:** anything that ends in a commit, push, or merge → Claude Code. Anything that stays inside GitHub's API surface (issue triage, project board updates, PR comments, milestone management) → Cowork is fine, and often faster because it can stay in conversation.

### Git workflow

**Branching:** Simple feature branches. All code changes go through pull requests — never push directly to `main`.

**Branch naming:** `phase-N/description` for phase work, `feature/description` for standalone features, `fix/description` for bug fixes.

**Pull request workflow:**

1. Claude Code creates a feature branch, commits work, and pushes to GitHub.
2. Claude Code opens a PR via `gh pr create` with a description summarizing what changed and why.
3. Brendan reviews the diff on GitHub (or via `gh pr diff` / Cowork in Chrome).
4. Brendan approves and merges (GitHub UI or `gh pr merge`).

**Rules:**
- **Never push directly to `main`.** All code changes require a PR.
- **Never merge your own PR.** Only Brendan merges.
- PR descriptions should summarize the changes, call out anything non-obvious, and list any open questions.
- Documentation-only changes (CLAUDE.md, docs/design.md) can be committed to `main` directly — they don't need code review.

**Coordination between assistants:**

- Both assistants work on the same checkout — no push/pull needed to see each other's changes.
- **Cowork** cannot run git at all (its sandbox mount blocks `unlink`). When Cowork wants a change committed, it edits the working tree and notes the intended commit in `docs/handoffs/cowork-handoff.md` or chat; Brendan or Claude Code then stages, commits, and pushes.
- **Claude Code** must `git push` after making changes so the remote stays current.
- Both should check `git status` before starting work to avoid conflicts.
- If both need to edit the same file, coordinate through the user (Brendan).

### Who does what

| Task | Tool |
|------|------|
| Architecture & design docs | Cowork |
| Code implementation | Claude Code |
| Building & testing | Claude Code |
| Git commits | Claude Code or Brendan (Cowork cannot run git) |
| Git pushes | Claude Code (or Brendan) |
| Code review & research | Either |
| Project board / issue triage | Either (Cowork via MCP, Claude Code via `gh`) |
| Deployment config | Claude Code (with Cowork for planning) |
| Browser-based tasks | Cowork |
| Design reviews | Cowork (writes to `reviews/design/`) |
| PR reviews | Claude Code (posts as PR comment via `gh pr comment` or MCP) |

### Review directories

- **`reviews/pr/`** — Claude Code writes PR review explanations here.
- **`reviews/design/`** — Cowork writes design session records here. These use a checkbox format so Brendan can sign off section by section.

### Design session format

When Cowork conducts a design review or audit, the findings should be written as a markdown file in `reviews/design/` with numbered sections and checkboxes:

```markdown
## 3. Security Concerns
### 3.1 CSRF / SameSite cookie policy
[Finding and recommendation]
- [ ] Approved
- [ ] Needs discussion
```

Brendan signs off on sections. Approved decisions get integrated into docs/design.md. Open items carry to the next session.
