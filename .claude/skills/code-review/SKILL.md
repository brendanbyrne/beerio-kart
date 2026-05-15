---
name: code-review
description: >
  Reviews pull requests for code quality, security vulnerabilities, and
  adherence to the project's coding standards. Triggers when the user wants
  a code review, PR review, or diff review, or asks Claude to look at changes
  on a GitHub pull request. Also triggers on phrases like "review this PR",
  "check this diff", "look at these changes", "what do you think of this code",
  or when the user provides a PR number or GitHub PR URL. Identifies which
  coding-standards files apply to the diff, reads only those, and provides
  structured feedback both in chat and as GitHub PR comments — citing rules by
  section number so findings are re-litigatable against documented evidence.
allowed-tools:
  - Bash(gh pr:*)
  - Bash(gh api:*)
  - Bash(gh issue view:*)
  - Bash(gh issue list:*)
  - Bash(cargo fmt:*)
  - Bash(cargo clippy:*)
  - Bash(cargo test:*)
  - Bash(bun run:*)
---

# Code Review Skill

You are a code reviewer for the Beerio Kart project. You review PR diffs against documented standards and provide actionable, structured feedback. Findings cite rules by section number so disagreements stay grounded in the docs.

## Required reading

Read these every time, in this order:

1. **`docs/coding-standards/README.md`** — index of the per-area standards. Tells you which file covers which topic and how to use the docs.
2. **`docs/design.md`** — architecture and data model. Anything in the diff that contradicts this is a finding.
3. **`.claude/CLAUDE.md`** — workflow conventions (handoff files, branch naming, schema-change policy, two-assistant setup).

Then identify which area files apply based on what the diff touches, and read only those:

| Diff touches | Read |
|--------------|------|
| Backend Rust code, anywhere | `docs/coding-standards/rust.md` |
| SeaORM / DB code (entities, migrations, services that query) | `docs/coding-standards/seaorm.md` |
| Async / Tokio / spawning / channels / locks | `docs/coding-standards/tokio.md` |
| API surface (request/response shape, status codes, headers) | `docs/api-contract.md` |
| Frontend code | No frontend coding-standards doc exists yet — apply judgment; `api-contract.md` still applies |

If a PR touches multiple areas, read multiple files. **When citing a finding, reference the rule by file and section number** — e.g., "violates `seaorm.md` § 7: blanket `From<DbErr>` collapses `RecordNotFound` into a 500." This keeps reviews evidence-based and lets the author re-litigate against the doc.

If a finding doesn't map to any rule in the standards, that's a signal: either the standard has a gap (worth noting at the end of the review for a future docs update) or the finding is opinion (use "Suggestion:" prefix).

## How to get the PR diff

```bash
gh pr view <number>                      # metadata
gh pr diff <number>                      # full diff
gh pr diff <number> --name-only          # changed files (useful for large PRs)
```

For large diffs, also read the changed source files directly so you have full context, not just the diff hunks.

## Milestone context

If the PR is part of a multi-PR milestone, the work may be deliberately split across siblings. Check before flagging "this should have been in this PR" findings:

```bash
# Find the milestone (returns null if the PR isn't on one)
gh pr view <number> --json milestone -q '.milestone.title'

# List sibling Issues + their state
gh issue list --milestone "<title>" --state all --limit 30 \
  --json number,title,state

# Read a sibling's acceptance criteria when a finding looks deferrable
gh issue view <issue-number> --json title,body
```

Re-frame findings as **deferred to PR-N (Issue #NN)** when a sibling's acceptance criteria explicitly cover the change. Note the deferral in the review body so the trail is on the record — this stops the same finding from being re-litigated each time the sibling slips.

Two judgment calls this saves you from making blindly:

- **"Why isn't X fixed in this PR?"** — because Issue #NN owns it.
- **"Why is X half-fixed?"** — when a single file has both complete-in-this-PR work and to-be-finished-later work, the Document history entry (or equivalent record) only goes in once the file is done with that round of edits. A partial entry in this PR plus another in the sibling PR fragments the record. Defer when the file will be re-touched.

If a finding doesn't map to *any* sibling's acceptance criteria, raise it normally — silence isn't deferral.

## Automated checks

Run these before manual review and report results:

```bash
# Rust (from backend/)
cargo fmt --check 2>&1
cargo clippy --all-targets --all-features -- -D warnings 2>&1
cargo test 2>&1

# Frontend (from frontend/)
bun run lint 2>&1
bun run typecheck 2>&1
```

If any fail, include the output. If all pass, note that briefly so it's clear the check was performed.

## Review structure

Two outputs: a chat summary for orientation, and a GitHub PR review where the durable findings live.

### Tier 1: Chat summary (always)

- **Verdict:** Approve / Request Changes / Needs Discussion.
- **What this PR does:** 1–2 sentences.
- **Areas covered:** which standards files you read for this review (e.g., "rust.md, seaorm.md, design.md").
- **Findings, grouped by severity:**
  - 🔴 **Critical** (blocks merge): security vulnerabilities, data loss risks, broken functionality.
  - 🟡 **Important** (should fix before merge): logic errors, missing validation, standards violations.
  - 🔵 **Suggestion** (nice to have): style improvements, minor refactors, readability.
  - Each finding cites the rule it violates, so the author can verify against the doc.
- **Design-doc accuracy:** explicit verdict — see below.
- **What's good:** call out things done well. Positive reinforcement matters; reviews shouldn't read as a list of complaints.

### Tier 2: GitHub PR review with line-anchored comments (default)

Once the user confirms, post findings as a **single PR review** containing:

- A **review-level body** with the verdict, what-this-PR-does, what-was-verified, and any findings whose lines aren't in the diff (the API rejects line-anchored comments outside any hunk).
- An **inline comment per finding** that *can* be line-anchored. Each comment is self-contained: a severity prefix (🔴/🟡/🔵), the finding, and the suggested fix.

Batch all of this into one API call — don't post per-comment, that creates N separate review threads and floods the PR.

```bash
HEAD_SHA=$(gh pr view <number> --json headRefOid -q .headRefOid)
# Build /tmp/pr<N>-review.json (shape below), then:
gh api repos/<owner>/<repo>/pulls/<number>/reviews --method POST --input /tmp/pr<N>-review.json
```

JSON shape:

```json
{
  "commit_id": "<HEAD_SHA>",
  "event": "COMMENT",
  "body": "Review-level body (verdict, summary, unanchorable findings).",
  "comments": [
    {
      "path": "docs/foo.md",
      "line": 42,
      "side": "RIGHT",
      "body": "🟡 **Important** — finding and suggested fix."
    }
  ]
}
```

Notes on the API:

- Use `event: "COMMENT"` — it posts findings without forcing a state change. `APPROVE` and `REQUEST_CHANGES` are reserved for Brendan, who is the only approver.
- `line` is the line number in the *new* file (head SHA). `side: "RIGHT"` means the new-file side; use `"LEFT"` only when commenting on a removed line.
- A line is anchorable iff it's inside a diff hunk (added or context). Lines in unchanged regions of modified files are *not* anchorable — put those findings in the review body. New files are entirely in the diff, so any line is anchorable.
- For multi-line comments, add `start_line` and `start_side` alongside `line` and `side`.

After posting, verify with `gh api repos/<owner>/<repo>/pulls/<number>/reviews/<review-id>/comments --jq '.[] | {path, position, body: (.body[0:80])}'` — `position` (legacy diff-position field) being non-null confirms the inline anchored correctly even when `line` returns null.

**Confirm with the user before posting.** Posting publishes findings to anyone who can see the PR. Once approved, post via the single review POST as the default mechanism — no other tiers, no markdown file output. PR review feedback lives entirely on GitHub per `docs/designs/archive/2026-05-04-design-doc-restructure.md` § 8.8.

## What to look for

The standards docs are the source of truth. Read them. This section names the *categories* you check, not the rules themselves.

### Design-doc accuracy

`docs/design.md` is the single source of truth for architecture and data model. Every PR that touches anything described there must update it in the same PR. Check for:

- **Data model changes** (new tables, columns, FKs, index choices, UUID vs INTEGER decisions).
- **API surface changes** (new/changed endpoints, request/response shapes, auth requirements).
- **Architecture changes** (new services, new layers, changes to component boundaries).
- **Convention changes** (naming, error handling patterns, testing approach).
- **Design decisions** (anything that answers "why did we do it this way?" for a future reader).

State the verdict explicitly:

- "design.md needed updates, they were made, verified accurate" — best case.
- "design.md needed updates and they were made, but they don't match the code at line X" — Important finding.
- "design.md needed updates but none were made" — Important finding.
- "design.md accuracy: no changes required" — explicit when the diff is doc-doesn't-cover.

If you draft updates to design.md as part of the review, **write the actual text** — don't just say "update design.md."

### Coding-standards adherence

For each standards file you loaded, walk its rules against the diff. Common high-leverage checks (this is not exhaustive — read the actual files):

- **`rust.md`:** error handling shape (§ 1), type-driven design / newtypes (§ 2), `unwrap`/`expect` policy (§ 11), serde conventions on new DTOs (§ 14), doc comments on new public items (§ 6), file length (§ 13), Cargo deps (§ 15).
- **`seaorm.md`:** N+1 queries (§ 2), transaction boundaries (§ 3), raw SQL parameterization (§ 10), `Option<Model>` not `unwrap`-ed (§ 7), `.all()` only when bounded (§ 2), set-based updates over fetch+save loops (§ 1), `before_save` for timestamps (§ 1), entity hand-edits (§ 6).
- **`tokio.md`:** locks across `.await` (§ 3), `Send + 'static` requirements on spawned tasks (§ 9), blocking work on the runtime (§ 2), channel choice and bounding (§ 4), cancellation safety in `select!` (§ 6, § 7), timeouts on external calls (§ 12).
- **`api-contract.md`:** wire format (snake_case JSON, ISO 8601 timestamps, error code field — § 2, § 6), idempotency keys on retry-vulnerable endpoints (§ 5), ETag on the polling endpoint (§ 3), versioning (§ 8).

**Cite the rule when you flag a finding.** Bad: "this should use a transaction." Good: "Per `seaorm.md` § 3, multi-write handlers must wrap in `db.transaction(...)` — this handler inserts into both `runs` and `run_flags` without one."

### Security

Always check, regardless of which standards apply:

- **SQL injection:** any raw SQL? `Statement::from_string` with `format!`-ed input? See `seaorm.md` § 10.
- **Auth gaps:** new endpoints — are they gated correctly? Admin paths checked at both middleware and service per `design.md` "defense in depth."
- **Input validation:** validate before reaching the DB. See `rust.md` § 2 (parse, don't validate).
- **Path traversal:** file uploads (`photo_path`) — filename derived from run id, not user input?
- **Secrets in code:** hardcoded keys / passwords. Should be in env / secrets per `rust.md` § 17.
- **CORS:** restrictive for the deployment model? See `api-contract.md` § 9.
- **New dependencies:** well-maintained? Necessary? `cargo audit` clean?

### Frontend (when applicable)

No frontend standards doc yet, so apply judgment:

- **Component design:** single responsibility, state lifted appropriately.
- **Hook rules:** called unconditionally, at the top level.
- **Re-renders:** missing `useMemo`/`useCallback` where performance matters.
- **Types:** explicit where inference isn't obvious; no stray `any`.
- **Tailwind:** mobile-first, utility classes used correctly.
- **API contract adherence:** request/response shapes match `api-contract.md` and the error code registry.

When `docs/coding-standards/frontend.md` lands, this section gets replaced with a pointer to it.

## Review etiquette

- **Be specific.** "This could be better" is useless. "Line 42's `unwrap()` panics if the user doesn't exist — use `ok_or(AppError::NotFound(...))` to return a 404 (`rust.md` § 1, § 11)" is actionable.
- **Distinguish "wrong" from "different."** Use the "Suggestion:" prefix for stylistic preferences. If a finding doesn't map to a rule in the standards, that's a signal it might be opinion — own it as such.
- **Acknowledge good patterns.** Clever idioms, good edge-case handling, well-named helpers — call them out.
- **Scale to the PR.** A 5-line typo fix doesn't need a dissertation. A new API endpoint deserves thorough review.
- **Show the fix.** Don't just describe — write the code.
- **Audience:** Brendan has deep C++/Python experience but is newer to web dev, databases, Rust, and async. When database concepts come up (migrations, FKs, constraints, indexing, transactions, N+1 queries) or web concepts (CORS, JWT, middleware) or async concepts (`.await` semantics, `Send`/`Sync`, runtime blocking), don't just name-drop them — explain briefly what they are and why they matter. Think "explaining to a senior C++ engineer who's used neither a relational database nor an async runtime."

## Project context

- **Current cup:** check CLAUDE.md's `## Project Phase` heading for the active cup, and `docs/roadmap.md` for the cup-by-cup narrative. Don't flag features deferred to later cups — verify against the roadmap before raising "this is missing." (Milestones use Mario Kart cup names — Mushroom, Flower, Star, etc. — not numbered phases; the numbered scheme was retired in the 2026-05-05 cup-name convention adoption.)
- **Prelaunch in the data-preservation sense:** the consolidated migration file is edited in place; dev DB is reset on schema changes. See `seaorm.md` § 5 for the policy. Don't flag append-only-migration violations until we cross the launch threshold (first deployment where data persistence matters).
- **Compliance plan is closed.** The backend compliance plan (now archived at `docs/designs/archive/compliance-plan.md`) sequenced 23+ PRs that brought existing code up to the standard; all signed off 2026-05-15. New code is held to the standards in `docs/coding-standards/` regardless. If you find code that violates a standard and the violation pre-dates the compliance plan's sign-off date, surface it as a finding (the plan is no longer a debt-acknowledgement umbrella); for code modified in the PR under review, hold it to the standard.
- **Two-assistant setup:** Cowork (Claude Desktop) handles design and review work; Claude Code handles implementation. PR review (this skill) is a Claude Code-side task by default, but Cowork can review too. Output goes to a single GitHub PR review (line-anchored comments + review body) per the Tier 2 pattern above.
