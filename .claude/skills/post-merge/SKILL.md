---
name: post-merge
description: >
  Clean up after a PR has been merged. Determines the branch to delete, checks out
  main, pulls the latest changes, and deletes the merged branch locally and remotely.
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

## Notes

- Squash and rebase merges produce a warning like *"deleting branch X that has
  been merged to refs/remotes/origin/X, but not yet merged to HEAD"* — that's
  expected and `-d` succeeds. No action needed.
- If the remote branch was already auto-deleted by GitHub, the remote-delete
  step prints "remote branch already deleted" and continues.
