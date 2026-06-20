# Branch protection for `main`

This repo's two load-bearing git conventions — **never push directly to `main`**
and **all changes go through a PR with passing checks** — are currently kept by
trust. Issue [#195](https://github.com/brendanbyrne/beerio-kart/issues/195) brought
CI to a state where they can be **machine-enforced** by a GitHub repository
ruleset. This doc is the ready-to-apply artifact plus the reasoning behind it.

The ruleset itself is checked in at
[`.github/branch-protection-ruleset.json`](../.github/branch-protection-ruleset.json).

> **Status: active.** Applied 2026-06-20 as ruleset id `17919103` (enforcement
> `active`, no bypass actors) after #195's PR (#236) merged. A direct push to
> `main` is now rejected (`GH013`), and a PR needs the five required checks green
> to merge. The procedure below is retained for reference, edits, and rollback.

## What the ruleset enforces

Applied to `~DEFAULT_BRANCH` (`main`), `enforcement: active`, **no bypass actors**:

| Rule | Effect |
|---|---|
| `pull_request` (0 required approvals) | Every change reaches `main` through a PR — no direct pushes. Squash is the only allowed merge method (matches the project's squash-merge convention). |
| `required_status_checks` | A PR can't merge until the five checks below are green. |
| `non_fast_forward` | No force-pushes to `main`. |
| `deletion` | `main` can't be deleted. |

### Required status checks

The required set is **only checks that always post a status on every PR** — a
required check that can be *absent* (skipped, never emitted) deadlocks the PR at
"Expected / Pending" forever, and with no bypass actors that blocks everyone,
including the repo owner. Each entry below is safe on that axis:

| Context | Source | Why it always posts |
|---|---|---|
| `Frontend lint & typecheck` | `checks.yml` | Always-run job; internally skips to a green pass when no frontend files changed (`detect-changes.sh`). |
| `Backend clippy & fmt` | `checks.yml` | Same always-run + internal-skip shape for the backend half. |
| `Backend DTO ↔ frontend types sync` | `dto-drift.yml` | Always-run; `dto-drift-check.sh` exits 0 with "nothing to check" when no DTO file changed. |
| `codecov/patch/frontend` | Codecov app | `frontend.yml` runs on **every** PR (sub-second suite), so Codecov always gets ≥1 upload and processes the commit. |
| `codecov/patch/backend` | Codecov app | Carries forward from base when `backend.yml` path-skips; posts once Codecov processes the commit (guaranteed by the frontend upload above). |

**Deliberately *not* required:**

- **`codecov/project/*`** — not yet required, but the likely *cause* of it not
  posting is now fixed. Project coverage is a base-relative comparison ("did
  total coverage regress vs. `main`?"), so it needs a coverage report at the
  PR's base commit — and many `main` commits had none (a docs- or one-sided
  commit where the path-filtered coverage workflows skipped uploaded nothing,
  so Codecov had no base to compare against and withheld the status, while
  `patch` — an absolute threshold on the diff — still posted). This PR closes
  that gap: `frontend.yml` always-runs, and `backend.yml` always-runs on `main`
  pushes, so every `main` commit now carries a coverage report for both flags
  (a one-sided PR's missing flag fills via carryforward from that base). Once
  this lands, `codecov/project/*` should begin posting. The remaining follow-up
  is to confirm it posts reliably on a few PRs, *then* add the two contexts here
  and to the ruleset — requiring a context that doesn't post would deadlock
  every PR, so it stays verify-then-require.
- **`link-check`** — path-filtered to `**/*.md` (skips on code-only PRs, so it
  can't be required without the always-run treatment) and prone to transient
  per-host flakes that would block merges on infrastructure unrelated to the PR.
  It stays advisory.

**Operational note.** Because the `codecov/patch/*` checks only post once Codecov
processes an upload, a **failing frontend test suite** (which aborts `frontend.yml`
before its upload step) leaves *both* `codecov/patch/*` checks sitting at
"Expected / pending" rather than red — Codecov never received the commit. This is
not a Codecov outage and not a permanent deadlock: the always-run `Checks` job
will also be red (showing the real cause), and the checks clear as soon as the
failing test is fixed and pushed. The ruleset's `required_status_checks` rule also
sets `do_not_enforce_on_create: false` deliberately, so the checks are enforced
even on the first push that creates a ref — there is no create-time gap.

**Toolchain pinning.** The required `Backend clippy & fmt` check runs clippy on
bare `cargo`, which resolves to the stable version pinned in
[`rust-toolchain.toml`](../rust-toolchain.toml) (repo root) — the same file the
local lefthook hook honors, so the two never disagree. This closes a real
surprise-breakage hole: on an *unpinned* `stable`, the day a GitHub runner bumps
the default rustc to a release with a new default lint, the required clippy check
goes red on **every open PR** until the new violations are fixed — blocking all
merges. (That exact drift surfaced when this CI first ran: CI's stable had
`clippy::duration_suboptimal_units`, the local stable didn't.) Bump the pin
deliberately and fix any new lints in the same PR. `fmt` runs on floating nightly
(rustfmt.toml needs nightly-only options) — a smaller surface left to match
lefthook.

## The always-run + internal-skip pattern

The deadlock this avoids: GitHub waits for a required check's status; a workflow
filtered out by a top-level `on.pull_request.paths` posts **no** status for a PR
that touches none of its paths; GitHub can't distinguish "skipped on purpose"
from "not finished," so it parks the check at Expected/Pending and the PR is
unmergeable. (Surfaced wiring up the DTO Drift check — see Issue #195.)

The fix, applied to every required workflow: drop the workflow-level `paths:`,
trigger on every PR, and move the path check **inside** the job so it always
posts a real status — a true "all clear" when there's nothing to verify, instead
of an absent one. `checks.yml` does this via `detect-changes.sh` (step-level
`if:` guards keep the job itself running to a green finish); `dto-drift.yml`
relies on `dto-drift-check.sh`'s early `exit 0`.

The coverage workflows are the exception that proves the rule: `backend.yml`
stays path-filtered because `cargo llvm-cov` is slow, and Codecov's flag
carryforward covers the skipped side. `frontend.yml` is always-run (its suite is
sub-second) specifically so Codecov receives an upload on every PR and therefore
always posts the required `codecov/patch/*` checks.

## How to apply

Apply only **after** the #195 PR has merged to `main`, so the
`Frontend lint & typecheck` and `Backend clippy & fmt` jobs have run on `main`
at least once and their exact context names are confirmed.

1. **Confirm the five context names** appear on a recent PR (a misspelled
   required context is unmatchable and, with no bypass actors, blocks all merges):

   ```bash
   PR=<a recent PR number>
   SHA=$(gh pr view "$PR" --json headRefOid --jq '.headRefOid')
   gh api "repos/brendanbyrne/beerio-kart/commits/$SHA/check-runs" --jq '.check_runs[].name' | sort -u
   ```

   You should see `Frontend lint & typecheck`, `Backend clippy & fmt`,
   `Backend DTO ↔ frontend types sync`, `codecov/patch/frontend`,
   `codecov/patch/backend` among them.

2. **Create the ruleset:**

   ```bash
   gh api --method POST repos/brendanbyrne/beerio-kart/rulesets \
     --input .github/branch-protection-ruleset.json
   ```

3. **Verify it's active:**

   ```bash
   gh api repos/brendanbyrne/beerio-kart/rulesets --jq '.[] | {id, name, enforcement}'
   ```

4. **Smoke-test** by opening a throwaway PR and confirming the merge button is
   blocked until the five checks pass, and that a direct `git push origin main`
   is rejected.

### Rollback

```bash
# Find the id, then disable (keeps it for later) or delete:
ID=$(gh api repos/brendanbyrne/beerio-kart/rulesets --jq '.[] | select(.name=="main branch protection") | .id')
gh api --method PUT    repos/brendanbyrne/beerio-kart/rulesets/$ID -f enforcement=disabled
# or
gh api --method DELETE repos/brendanbyrne/beerio-kart/rulesets/$ID
```

To edit the required set later, update
`.github/branch-protection-ruleset.json` and re-`PUT` it to the ruleset id.

## What this does and doesn't enforce

**Enforced:** all changes reach `main` through a squash-merged PR; the five
checks must be green; no force-push or deletion of `main`.

**Not enforced — and why:** "**never merge your own PR**" cannot be machine-
enforced here. Both assistants and the maintainer operate GitHub as the single
`brendanbyrne` identity, so a required-approval rule would ask that identity to
approve its own PRs — which GitHub forbids, deadlocking every merge. The ruleset
therefore sets `required_approving_review_count: 0`; "don't self-merge an
assistant's PR without review" remains a human convention. If a distinct
reviewer identity (a second account or a review bot) is ever added, raise the
count to 1 and the convention becomes enforceable.

**Consequence for docs:** with a PR required for *all* pushes, the former
"documentation-only changes may be committed to `main` directly" carve-out is
retired (see [`.claude/CLAUDE.md`](../.claude/CLAUDE.md) and
[`project-workflow.md`](./project-workflow.md) § Review and merge). Docs changes
now go through a PR like everything else; the post-merge skill's doc sign-offs
become a small follow-up PR rather than a direct commit.

## Document history

- 2026-06-15 — Created for Issue #195. Documents the ready-to-apply `main`
  ruleset and the always-run + internal-skip CI pattern that makes its required
  checks deadlock-safe. Required set decided as the always-posting checks only
  (`codecov/project/*` and `link-check` excluded, with reasons). Records the
  single-identity limitation on enforcing "never merge your own PR."
