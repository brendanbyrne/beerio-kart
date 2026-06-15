#!/usr/bin/env bash
#
# checks.yml change-detection (Issue #195).
#
# Emits `run=true` / `run=false` to $GITHUB_OUTPUT depending on whether the
# current PR/push changed any file relevant to the named check AREA. The jobs
# in .github/workflows/checks.yml ALWAYS run — so they always post a status,
# which is what lets them be *required* checks without deadlocking a PR that
# happens to touch the other half of the repo (a workflow-level `paths:` filter
# would skip the whole job and post no status, parking a required check at
# "Expected/Pending" forever). Each job calls this script to decide whether to
# do the real lint/typecheck/clippy/fmt work or short-circuit to a green
# "nothing relevant changed" pass.
#
# Usage:  detect-changes.sh <frontend|backend>
#
# Inputs (env), in priority order:
#   BASE_SHA + HEAD_SHA   — a PR's base-branch tip and head commit. Diffed
#                           three-dot (BASE...HEAD): changes on the head since
#                           its merge base, ignoring commits that landed on the
#                           base afterwards. Mirrors dto-drift-check.sh.
#   BEFORE_SHA + HEAD_SHA  — a push's previous and new tip. Diffed two-dot.
#
# If neither pair is usable (first push, force-push, history not fetched), the
# script fails OPEN — emits run=true — so a check never silently passes for
# lack of a diff range.
set -euo pipefail

AREA="${1:?usage: detect-changes.sh <frontend|backend>}"

# Files that, when changed, mean this area's checks must run. The CI plumbing
# itself (this script + checks.yml) is in BOTH lists, so a change to it is
# validated by running both halves.
CI_PLUMBING='\.github/workflows/checks\.yml$|\.github/scripts/detect-changes\.sh$'
case "${AREA}" in
  frontend) AREA_REGEX="^(frontend/|${CI_PLUMBING})" ;;
  backend)  AREA_REGEX="^(backend/|Cargo\.toml$|Cargo\.lock$|rust-toolchain\.toml$|${CI_PLUMBING})" ;;
  *) echo "detect-changes.sh: unknown area '${AREA}' (expected frontend|backend)" >&2; exit 2 ;;
esac

emit() { echo "run=$1" >> "${GITHUB_OUTPUT:-/dev/stdout}"; }

ZERO='0000000000000000000000000000000000000000'
if [[ -n "${BASE_SHA:-}" && -n "${HEAD_SHA:-}" ]]; then
  range="${BASE_SHA}...${HEAD_SHA}"      # PR: three-dot (since merge base)
elif [[ -n "${BEFORE_SHA:-}" && "${BEFORE_SHA}" != "${ZERO}" && -n "${HEAD_SHA:-}" ]]; then
  range="${BEFORE_SHA}..${HEAD_SHA}"     # push: two-dot
else
  echo "detect-changes.sh: no usable diff range; running ${AREA} checks unconditionally."
  emit true
  exit 0
fi

# Capture the diff in an assignment (not as a pipeline condition) so a git
# failure — an unreachable range from a force-push race, a GC'd SHA, or a
# partial fetch — is distinguishable from "no files matched". Inside
# `if git diff … | grep -q`, set -e/pipefail are suppressed and a failed git
# diff feeds grep empty input, matching nothing and falling through to
# run=false: a *required* check passing green having validated nothing. Route a
# genuine diff failure to fail-OPEN (run=true), as the header promises.
if ! changed="$(git diff --name-only "${range}")"; then
  echo "detect-changes.sh: 'git diff ${range}' failed (unreachable range?); failing open — running ${AREA} checks." >&2
  emit true
  exit 0
fi

if printf '%s\n' "${changed}" | grep -Eq "${AREA_REGEX}"; then
  echo "detect-changes.sh: ${AREA}-relevant files changed in ${range} — running checks."
  emit true
else
  echo "detect-changes.sh: no ${AREA}-relevant files changed in ${range} — skipping (all-clear)."
  emit false
fi
