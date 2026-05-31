---
name: updating-skills
description: >
  Guides revising an EXISTING skill — editing or tightening its SKILL.md,
  rewriting the description for better triggering, restructuring the body,
  moving detail into reference files, or re-evaluating whether it still earns
  its keep. This is the revise-what-exists workflow, distinct from authoring a
  skill from scratch (use skill-creator for that). Bundles the skill-authoring
  best-practice recommendations as a reference. Use when the user wants to
  update, edit, improve, tighten, refactor, restructure, fix, or re-evaluate a
  skill or a SKILL.md, when a skill is "under-triggering" or "over-triggering"
  or "too long" or "stale", or when reviewing a skill before committing it —
  run it even if the user doesn't say the word "skill" but is clearly editing
  one (e.g. a file under .claude/skills/ or a SKILL.md anywhere).
allowed-tools:
  - Read
  - Grep
  - Glob
  - Edit
---

# Updating an Existing Skill

Revise a skill that already exists — tighten its prose, fix its triggering, restructure its body, or decide whether it still pays for itself. This is the **revise** path. To build a skill from nothing, reach for Anthropic's `skill-creator` skill instead; it scaffolds and evaluates new skills, and ships the upload tooling (`quick_validate.py`, `package_skill.py`) referenced below.

Before changing anything, read the bundled reference — [references/skill-authoring-best-practices.md](references/skill-authoring-best-practices.md) — for the rules each step below leans on. Read the section you need; you don't have to read it all.

## Method

Work the steps in order. Each is a re-check of one property a good skill must hold; fix what's drifted and move on.

1. **Read the skill as it stands.** Open the `SKILL.md` and every file it references. Note the install location — a skill under `.claude/skills/` is Claude-Code-only and may use extended frontmatter; a skill destined for upload (claude.ai / the API) is held to the validator's allow-list. The two have different rules (reference § 2), so settle which one you're editing first.

2. **Re-validate the frontmatter.** `name`: kebab-case, ≤64 chars, `[a-z0-9-]` only, no leading/trailing or doubled hyphens, no `claude`/`anthropic`. `description`: non-empty, ≤1024 chars, and **no angle brackets** (they break the parser). If the skill is upload-bound, confirm only allow-listed fields are present (`name`, `description`, optional `license` / `allowed-tools` / `metadata` / `compatibility`) — extended Claude-Code fields fail `package_skill.py`. See reference § 2.

3. **Re-check the description triggers well.** This is the single highest-leverage edit. Confirm it is **third person** (it lands in the system prompt, so "Reviews…", not "I/you review…"), states **both what it does and when to use it**, and names **concrete trigger phrases** — the literal words and file types a user would say. If the skill under-triggers, make it a little pushy (e.g. "…run it even if the user doesn't explicitly ask"). If it over-triggers, narrow the triggers and add an explicit non-goal. See reference § 4.

4. **Keep the body lean; push detail down.** The body is loaded on every trigger, so every line is a recurring cost. Target well under 500 lines / ~5k tokens. For each paragraph apply the conciseness test (reference § 6): does Claude really need this, or is it re-teaching something the model already knows? Cut the latter. Large, rare, or mutually-exclusive content belongs in a reference file, not the body (reference § 3).

5. **Verify run-vs-read intent for every script.** Each bundled script must say plainly whether Claude **runs** it or **reads** it: "Run `scan.sh` to surface candidates" vs "Read `algo.py` for the formula." Execution is preferred for deterministic utilities (more reliable, fewer tokens). Use forward slashes in all paths, even for Windows targets, and in Claude Code reference scripts via `${CLAUDE_SKILL_DIR}/scripts/…` so the path resolves wherever the skill installs. See reference § 5.

6. **Keep references one level deep, with a TOC.** SKILL.md should point *directly* at each reference (chained reference-to-reference links get partial reads). Name files by content (`form_rules.md`, not `doc2.md`). Any reference over ~100 lines needs a table of contents with anchor links at the top. Exactly one `SKILL.md` per skill — a nested second one breaks upload. See reference § 5.

7. **Re-run or extend the evals.** A revision can regress triggering or behavior. If the skill has eval scenarios, re-run them; if it doesn't, add ~3 (reference § 7). The reliable check is the two-Claude pattern: a fresh Claude uses the revised skill on a real task while you watch its navigation — missed links, overreliance, ignored content all point at the next fix. `skill-creator` ships an eval harness (parallel with/without-skill runs, graded assertions) plus `quick_validate.py` and `package_skill.py` for uploadable skills.

8. **Sweep for the anti-patterns.** Before you call it done, scan the whole skill against reference § 7's anti-pattern list. The ones that bite revisions most: a body that grew past 500 lines or got padded with what Claude knows; a description that drifted to vague or first-person; offering too many options instead of one default + an escape hatch; time-sensitive phrasing (fold superseded guidance into a short "Old patterns" note rather than narrating "recently changed"); inconsistent terminology for the same concept; heavy-handed ALL-CAPS "MUST" where explaining *why* would do; and overfitting to the test examples. Match the degrees of freedom to the task: prose for open work, exact scripts and "do not modify" only for genuinely fragile steps.

## Output

When the revision is done, report concisely:

- **What changed** — the edits, grouped (frontmatter / description / body / references / scripts / evals).
- **Why** — which property each edit restored, citing the reference section (e.g. "description was first-person → § 4").
- **Residual risk** — anything you couldn't verify (e.g. evals not re-run because none exist), so the gap is on the record rather than silently assumed closed.

Show the before/after for the `description` whenever you touched it — it's the field most worth a second pair of eyes.

## References

- [references/skill-authoring-best-practices.md](references/skill-authoring-best-practices.md) — the full, cited best-practice recommendations: SKILL.md anatomy and frontmatter, progressive disclosure, writing the description, bundling resources, the token budget, evaluation, the anti-pattern list, an annotated template, and cross-platform caveats. **Read it** when applying any step above.
