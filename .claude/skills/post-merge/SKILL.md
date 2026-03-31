---
name: post-merge
description: >
  Clean up after a PR has been merged. Checks out main, pulls the latest changes,
  and deletes the merged PR branch both locally and from the remote. Use this skill
  when the user says a PR has been merged, or asks to clean up after a merge.
---

# Post-Merge Cleanup

After a PR has been merged, clean up the local and remote branches.

## Steps

1. Identify the current branch name (the one being cleaned up).
2. Check out `main`.
3. Pull the latest changes from the remote.
4. Delete the merged branch locally.
5. Delete the merged branch from the remote (if it still exists).
6. Confirm cleanup is complete.

## Implementation

```bash
# 1. Capture the current branch before switching
BRANCH=$(git branch --show-current)

# 2. If already on main, ask the user which branch to delete
#    Otherwise, proceed with the current branch

# 3. Check out main
git checkout main

# 4. Pull latest
git pull

# 5. Delete local branch
git branch -d "$BRANCH"

# 6. Delete remote branch (if it exists)
git push origin --delete "$BRANCH" 2>/dev/null || echo "Remote branch already deleted"
```

## Notes

- Use `git branch -d` (not `-D`) so Git refuses to delete an unmerged branch — a safety net.
- If the branch has already been deleted on the remote (e.g., GitHub auto-deletes), that's fine — just note it.
- If the user is already on `main`, ask which branch to clean up.
