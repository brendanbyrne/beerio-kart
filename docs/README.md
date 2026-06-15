# Beerio Kart docs

Start with `design.md` if you're new.

## I want to…

- **Touch the database schema** → `data-model.md` + relevant ADRs in `decisions/`.
- **Build or change an endpoint** → `api-contract.md` + `user-workflows.md`.
- **Add a session ruleset** → `user-workflows.md` (session loop) + a new ADR.
- **Understand why a decision was made** → `decisions/` (search the index).
- **Read background research on a topic** → `research/`.
- **Plan or pick up phase work** → `roadmap.md` + GitHub Issues.
- **Look up a workflow convention** (Issue lifecycle, branch naming, statuses, triage) → `project-workflow.md`.
- **Write a new design record** → start a draft in `drafts/`. Promote to `designs/` when signed off.
- **Follow the coding standards** → `coding-standards/`.

## Layout

- `design.md` — architectural overview (rules, principles, architecture)
- `data-model.md` — database schema
- `user-workflows.md` — user workflows + UI screens
- `api-contract.md` — wire-format conventions and endpoint list
- `roadmap.md` — phase narrative
- `project-workflow.md` — operational workflow (Issue lifecycle, branch naming, triage, multi-assistant coordination)
- `branch-protection.md` — the ready-to-apply `main` branch-protection ruleset + the always-run CI pattern behind it
- `decisions/` — Architecture Decision Records (MADR format)
- `designs/` — design records (durable narrative of how decisions were reached)
- `drafts/` — work-in-progress design records (gitignored except `WIP_*.md`)
- `research/` — long-form technical investigations that inform designs
- `coding-standards/` — backend coding standards
