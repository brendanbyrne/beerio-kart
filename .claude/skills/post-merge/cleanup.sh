#!/usr/bin/env bash
# Post-merge branch cleanup with guardrails.
# Usage: cleanup.sh <branch-name>
#
# Refuses to delete main/master or an empty branch name.
# Switches to main, pulls, deletes local branch with -d (safe form),
# then deletes the remote branch if it still exists.
#
# On -d refusal (unmerged work), exits non-zero without escalating to -D.
# The caller must decide whether -D is appropriate and run it manually.

set -euo pipefail

TARGET="${1:-}"

if [ -z "$TARGET" ]; then
  echo "error: no branch name provided" >&2
  echo "usage: $0 <branch-name>" >&2
  exit 2
fi

if [ "$TARGET" = "main" ] || [ "$TARGET" = "master" ]; then
  echo "error: refusing to delete protected branch '$TARGET'" >&2
  exit 2
fi

# Verify the branch actually exists locally before doing anything.
if ! git show-ref --verify --quiet "refs/heads/$TARGET"; then
  echo "error: local branch '$TARGET' does not exist" >&2
  exit 2
fi

echo "==> Target branch: $TARGET"
echo "==> Switching to main"
git checkout main

echo "==> Pulling latest"
git pull

echo "==> Deleting local branch (safe: git branch -d)"
if ! git branch -d "$TARGET"; then
  echo "error: git branch -d refused to delete '$TARGET' — branch has unmerged work" >&2
  echo "       do NOT escalate to -D without confirming the PR was actually merged" >&2
  exit 1
fi

echo "==> Deleting remote branch"
if git push origin --delete "$TARGET" 2>/dev/null; then
  echo "    remote branch deleted"
else
  echo "    remote branch already deleted (or never pushed)"
fi

echo "==> Cleanup complete"
git status
