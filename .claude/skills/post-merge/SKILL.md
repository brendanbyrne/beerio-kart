---
name: post-merge
description: >
  Clean up after a PR has been merged. Determines the branch to delete, checks out
  main, pulls the latest changes, and deletes the merged branch locally and remotely,
  then checks whether the merge now makes any follow-up documentation updates necessary
  (e.g. design-record or compliance-plan sign-off) and commits those directly to main.
  Use when the user says a PR has been merged, or asks to clean up after a merge.
---

# Post-Merge Cleanup

After a PR has been merged, clean up the local and remote branches.

The guardrails live in [cleanup.sh](cleanup.sh) — the script refuses to delete
`main`/`master`, refuses empty branch names, uses `git branch -d` (not `-D`),
and exits non-zero on an unmerged-work refusal instead of escalating.

## Procedure

1. **Determine the target branch.** Run:

   ```bash
   git branch --show-current
   ```

   - If output is `main` or `master` → ask the user which branch to clean up.
     Do NOT pass `main`/`master` to the script (it would refuse anyway, but
     don't rely on that).
   - If output is empty → detached HEAD. Ask the user.
   - Otherwise → that's your candidate. **Do not trust the session's initial
     gitStatus context** — the checkout may have changed. Always re-check.

2. **State the target in chat and confirm before running.** Example:
   "Cleaning up `feature/foo` — proceeding." This is the last human-visible
   checkpoint before destructive ops.

3. **Run the cleanup script** with the confirmed branch name:

   ```bash
   .claude/skills/post-merge/cleanup.sh <branch-name>
   ```

   The script will switch to `main`, pull, delete the local branch (`-d`), and
   delete the remote branch. It prints a `git status` at the end.

4. **On script failure with "not fully merged"**: stop. Do not run `git branch
   -D` on your own. Ask the user whether the PR was actually merged (e.g., via
   a different merge strategy locally) and get explicit approval before using
   `-D`.

5. **Check for follow-up documentation the merge now makes necessary.** The
   branch is gone, but merging can leave docs that referenced the work as *in
   flight* now stale. Look for updates the merge — not the PR's own diff —
   requires:

   - **Design-record / compliance-plan sign-off** — if the PR implemented an
     item in a `docs/designs/*` record, tick its checkbox/row, add the
     merged-PR link, and append a `## Document history` entry. If that record
     is now fully signed off *and* all its PRs are merged, it may be
     archive-eligible (`docs/CLAUDE.md` § Design records → Archive).
   - **Roadmap** — if the merge completes a milestone or phase item tracked in
     `docs/roadmap.md`.
   - **Cross-references** — docs that named the branch or PR as planned or
     pending and should now read as done.

   If any apply, make a **documentation-only** commit and push it **directly to
   `main`** — docs-only changes skip the PR flow by project rule (root
   `.claude/CLAUDE.md` § Git workflow). If nothing applies, say so and stop;
   don't manufacture churn.

   **Guardrail — docs only.** This direct-to-`main` commit may touch only
   documentation. Any code, test, schema, or config change still requires a
   PR; never fold one into this commit.

## Notes

- Squash and rebase merges produce a warning like *"deleting branch X that has
  been merged to refs/remotes/origin/X, but not yet merged to HEAD"* — that's
  expected and `-d` succeeds. No action needed.
- If the remote branch was already auto-deleted by GitHub, the remote-delete
  step prints "remote branch already deleted" and continues.
