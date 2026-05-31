# Skill-authoring best practices

The cited best-practice recommendations behind the `updating-skills` skill, drawn from Anthropic's Agent Skills documentation, engineering write-up, the `anthropics/skills` repo, the Claude Code skills doc, and the `skill-creator` skill. Read the section you need; you don't have to read it all. Source tags `[S1]`–`[S6]` resolve in [Sources](#sources).

## Contents

- [What a Skill is, and when to use one](#what-a-skill-is-and-when-to-use-one)
- [SKILL.md anatomy & frontmatter](#skillmd-anatomy--frontmatter)
- [Progressive disclosure (the core principle)](#progressive-disclosure-the-core-principle)
- [Writing the description for reliable triggering](#writing-the-description-for-reliable-triggering)
- [Bundling resources](#bundling-resources)
- [Conciseness / token budget](#conciseness--token-budget)
- [Evaluating & iterating; anti-patterns](#evaluating--iterating-anti-patterns)
- [Annotated SKILL.md template](#annotated-skillmd-template)
- [Cross-platform caveats](#cross-platform-caveats)
- [Sources](#sources)

## What a Skill is, and when to use one

A Skill = a folder with a `SKILL.md` (YAML frontmatter + Markdown body) plus optional bundled resources (scripts, references, assets). It packages procedural knowledge/context — "an onboarding guide for a new hire" — that Claude discovers and loads dynamically (S1, S3).

Use a Skill when you'd otherwise paste the same instructions/checklist/procedure repeatedly (S1, S5).

In Claude Code: put always-true *facts* in CLAUDE.md (always in context); put *procedures* in skills (load on demand, "long reference material costs almost nothing until you need it") (S5).

Skill vs tool/MCP: a tool/MCP *connects* capabilities; a Skill *teaches the workflow* for using them, and can reference MCP tools by fully-qualified `Server:tool` name (S2, S3).

Skill vs subagent: a skill is reusable knowledge; it can run inside a subagent (`context: fork`) or be preloaded by one (S5).

Prefer a bundled *script* for deterministic/fragile/repeated operations rather than having the model regenerate code (S3).

## SKILL.md anatomy & frontmatter

**Required fields:**

- `name` — ≤64 chars, lowercase letters/numbers/hyphens only (no leading/trailing or doubled hyphens), no XML tags, not "anthropic"/"claude" (S1, S2, S6).
- `description` — non-empty, ≤1024 chars, NO angle brackets, must state BOTH what it does AND when to use it (S1, S2, S6); the single most important field.

**Upload-safe optional fields** (validator allow-list): `license`, `allowed-tools`, `metadata` (nested ok), `compatibility` (≤500 chars) (S6).

**Body skeleton:** `# Title` then `## Instructions` / `## Examples`, plus Workflow / Guidelines / Output-format / reference-pointers as needed (S1, S6).

**FLAG — Claude-Code-only frontmatter is not upload-portable.** Claude Code supports many extra frontmatter fields (`when_to_use`, `argument-hint`, `arguments`, `disable-model-invocation`, `user-invocable`, `allowed-tools` / `disallowed-tools`, `model`, `effort`, `context`, `agent`, `hooks`, `paths`) but these are Claude-Code-only and are NOT in the upload validator's allow-list — a SKILL.md using them fails `package_skill.py` and won't upload to the API / claude.ai (S5, S6). For portability use only `name` / `description` (+ optional `license` / `metadata` / `allowed-tools` / `compatibility`); use extended fields only for `.claude/skills/` skills that are never uploaded.

## Progressive disclosure (the core principle)

Three levels (S1, S3, S6):

- **L1 — metadata** = `name` + `description`, ALWAYS loaded into the system prompt (~100 tokens/skill). Discovery info only.
- **L2 — SKILL.md body**, loaded when the skill triggers. Target under ~5k tokens / under 500 lines — the core workflow plus navigation pointers.
- **L3 — bundled files** (read on demand) and scripts (executed without their code entering context). Effectively unlimited, ~zero cost until used.

Put every-time guidance in the body; put large/optional/mutually-exclusive content in reference files; put deterministic logic in scripts. "There's no context penalty for bundled content that isn't used" (S1) — so over-bundling reference is cheap, but bloating the body is expensive.

## Writing the description for reliable triggering

(S2, S6)

- **Third person always** — it's injected into the system prompt, so not "I/you can…".
- **State BOTH what it does AND when to use it.**
- **Include concrete key terms / trigger phrases** — the literal words and file types a user would say.
- **Be a little "pushy"** to fight under-triggering (skill-creator adds, e.g., "Make sure to use this skill whenever the user mentions … even if they don't explicitly ask").

GOOD: "Extract text and tables from PDF files, fill forms, merge documents. Use when working with PDF files or when the user mentions PDFs, forms, or document extraction."

BAD: "Helps with documents" / "Processes data".

Note: trivial one-step requests may not trigger a skill even on a perfect match (Claude handles them directly); complex/multi-step requests trigger reliably (S6).

## Bundling resources

(S1, S2, S6)

**Layout:** `skill-name/SKILL.md` (required) + `scripts/` (executed code) + `references/` (docs loaded on demand) + `assets/` (templates/icons used in output).

- Reference files BY NAME from SKILL.md so Claude knows what each holds.
- Make execution intent EXPLICIT: "Run `x.py` to …" vs "See `x.py` for the algorithm" — execution preferred for utilities (more reliable, fewer tokens).
- Keep reference files ONE level deep from SKILL.md (chained references get partial reads).
- Add a table of contents to any reference file over ~100 lines.
- Forward slashes in ALL paths (even on Windows).
- Name files by content (`form_rules.md`, not `doc2.md`).
- Exactly ONE `SKILL.md` per skill (nested ones break upload).
- In Claude Code, reference scripts with `${CLAUDE_SKILL_DIR}/scripts/…` so the path resolves regardless of install location.

## Conciseness / token budget

(S1, S2, S5)

"Concise is key — the context window is a public good." Body under 500 lines / ~5k tokens.

Conciseness test per sentence: "Does Claude really need this? Can I assume Claude knows it? Does this justify its token cost?" Assume Claude is smart — don't explain domain basics. Push detail to references.

In Claude Code, every body line is a recurring per-turn cost — "state what to do rather than narrating how or why."

## Evaluating & iterating; anti-patterns

(S2, S3, S6)

**Create evaluations BEFORE writing extensive docs** (ensures you solve real, not imagined, problems):

1. Identify gaps by running Claude WITHOUT the skill.
2. Write ~3 eval scenarios.
3. Baseline.
4. Write minimal instructions.
5. Iterate.

**Two-Claude pattern:** Claude A authors/refines; a fresh Claude B uses it on real tasks. Observe B's navigation (unexpected paths, missed links, overreliance, ignored content) and refine. Test across every model you'll use.

`skill-creator` ships an eval harness (with/without-skill parallel runs, graded assertions) plus `quick_validate.py` and `package_skill.py`.

**ANTI-PATTERNS:**

- Bloated SKILL.md (>500 lines or padded with what Claude knows).
- Vague description or wrong point-of-view.
- Teaching Claude what it already knows.
- Offering too many options (give one default + an escape hatch).
- Deeply nested references.
- Windows paths (use forward slashes).
- Time-sensitive info (use a collapsed "Old patterns" section instead).
- Inconsistent terminology.
- Undocumented "voodoo constant" magic numbers.
- Scripts that punt errors to Claude.
- Unqualified MCP tool names.
- Heavy-handed ALL-CAPS "MUST" / over-rigid structure (reframe to explain WHY; reserve rigidity for genuinely fragile steps).
- Overfitting to your test examples.

**Match "degrees of freedom" to task fragility:** high freedom (prose) for open tasks; low freedom (exact scripts, "do not modify") for fragile ones. For complex tasks, give numbered steps + a copyable checklist + validator→fix→repeat loops.

## Annotated SKILL.md template

Adapt rather than copy verbatim:

```markdown
---
name: <verb-ing>-<noun>            # kebab-case, ≤64, [a-z0-9-], no claude/anthropic
description: <What it does, third person, concrete terms>. Use when <explicit trigger phrases / file types / situations, including when not named outright>.   # ≤1024 chars, no angle brackets
# optional, upload-safe: license / allowed-tools / metadata / compatibility (≤500)
---

# <Skill Title>
## Quick start            # the single most common path, minimal example, ONE default + escape hatch
## Workflow               # numbered steps + a copyable checklist for multi-step tasks; validate→fix→repeat
## Output format          # template pattern: strict only for fragile/contractual formats, else "sensible default"
## Examples               # concrete input→output pairs when output quality depends on seeing them
## Utility scripts        # state EXECUTE vs READ per script; forward slashes; ${CLAUDE_SKILL_DIR} in Claude Code
## Advanced / reference   # push large/rare detail to ONE-level-deep reference files; TOC if >100 lines
```

## Cross-platform caveats

Runtime varies:

- **API skills** have no network / no runtime installs (pre-installed packages only).
- **claude.ai** varies by settings.
- **Claude Code** has full local access.

List required packages in SKILL.md; don't assume installs. Skills don't sync across surfaces.

Project-local skills live in `.claude/skills/<name>/` (commit to version control); Claude Code auto-discovers them.

Security: treat installing a skill like installing software — audit bundled scripts; behavior must match stated intent ("principle of lack of surprise").

## Sources

Primary sources cited above:

- **[S1]** Agent Skills (overview), Claude Docs — https://platform.claude.com/docs/en/agents-and-tools/agent-skills/overview
- **[S2]** Skill authoring best practices, Claude Docs — https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices
- **[S3]** "Equipping agents for the real world with Agent Skills", Anthropic Engineering — https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills
- **[S4]** anthropics/skills repo (README + template/SKILL.md) — https://github.com/anthropics/skills
- **[S5]** "Extend Claude with skills" (Claude Code skills doc) — https://code.claude.com/docs/en/skills
- **[S6]** skill-creator skill (Anthropic-authored; SKILL.md + references/schemas.md + scripts/quick_validate.py + package_skill.py)
