#!/usr/bin/env bash
#
# Backend DTO ↔ frontend types.ts drift check (PR-H1, Issue #185).
#
# Fails when a PR changes a backend wire-contract file (a serialized request/
# response struct or a serialized enum) without also touching
# frontend/src/api/types.ts — the hand-maintained Zod mirror of those Rust
# types. ADR-0039 chose hand-maintained mirrors over Rust→TS codegen for MVP;
# this is the cheap safety net that keeps the two from silently drifting.
#
# False positives are acceptable by design: if a flagged backend change does
# not actually alter the wire shape, a one-line edit to types.ts (e.g. a
# comment) clears the check. See docs/coding-standards/typescript.md § 11.
#
# The DTO_PATHS_REGEX below MUST stay in sync with the `paths:` filter in
# .github/workflows/dto-drift.yml. When you add a new DTO-bearing backend
# module, add it to BOTH.
#
# Inputs (env): BASE_SHA, HEAD_SHA — the PR's base-branch tip and head commit.
set -euo pipefail

TYPES_FILE='frontend/src/api/types.ts'

# Backend files whose change implies a possible wire-contract change:
#   - routes/**         — the request/response structs returned by handlers
#   - domain/enums.rs   — SessionStatus / SessionRuleset (serialized enums)
#   - the services/ modules that define serialized DTOs
DTO_PATHS_REGEX='^backend/src/(routes/|domain/enums\.rs$|services/sessions/types\.rs$|services/sessions/detail\.rs$|services/runs/read\.rs$|services/users\.rs$|services/notifications\.rs$)'

# Three-dot diff: changes on the PR head since its merge base with the target
# branch (ignores commits that landed on the base after the branch point).
changed="$(git diff --name-only "${BASE_SHA}...${HEAD_SHA}")"

dto_changed="$(printf '%s\n' "${changed}" | grep -E "${DTO_PATHS_REGEX}" || true)"
types_changed="$(printf '%s\n' "${changed}" | grep -Fx "${TYPES_FILE}" || true)"

if [[ -n "${dto_changed}" && -z "${types_changed}" ]]; then
  echo "::error::Backend DTO files changed but ${TYPES_FILE} did not."
  {
    echo ""
    echo "Changed backend wire-contract files:"
    printf '%s\n' "${dto_changed}" | sed 's/^/  - /'
    echo ""
    echo "${TYPES_FILE} is the hand-maintained Zod mirror of the backend's"
    echo "serialized types (ADR-0039 — no Rust→TS codegen for MVP). If this"
    echo "backend change alters a request/response shape or a serialized enum,"
    echo "update types.ts to match. If it genuinely does not touch the wire"
    echo "contract, a one-line edit to types.ts (e.g. a comment) clears this check."
  } >&2
  exit 1
fi

if [[ -n "${dto_changed}" ]]; then
  echo "Backend DTO files changed and ${TYPES_FILE} was updated alongside — OK."
else
  echo "No backend DTO files changed — nothing to check."
fi
