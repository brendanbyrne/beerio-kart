# Build Orchestration & Repo Layout Review

**Date:** 2026-05-04
**Author:** Cowork (Claude)
**Trigger:** Brendan asked whether the current Cargo + Bun + justfile arrangement is sensible, given his Bazel background.

## 1. Current state (as of this review)

Top-level layout:

```
beerio-kart/
├── package.json           # name=beerio-kart, workspaces=["frontend"], dev script via concurrently
├── bun.lock               # single lockfile for the JS half of the repo
├── justfile               # dev / test / fmt / build / coverage recipes
├── .editorconfig
├── .gitattributes
├── backend/
│   ├── Cargo.toml         # workspace + root crate "beerio-kart"
│   ├── Cargo.lock
│   └── migration/         # second workspace member
└── frontend/
    ├── package.json       # name=frontend, dev/build/lint/typecheck/preview scripts
    ├── vite.config.ts
    └── (no own node_modules — hoisted to root via Bun workspaces)
```

Layer breakdown:

- **Layer 1 (per-language build):** Cargo for Rust, Bun + Vite for TS. Each owns its language's resolution, lockfile, incremental compilation.
- **Layer 2 (repo-wide orchestration):** `justfile` *and* `package.json` scripts both exist. They overlap.

## 2. What's awkward, specifically

### 2.1 Two `dev` commands that do the same thing

Root `package.json` has:

```json
"dev": "concurrently -n backend,frontend -c blue,green \"cd backend && cargo run\" \"cd frontend && bun run dev\""
```

`justfile` has:

```just
dev:
    #!/usr/bin/env bash
    trap 'kill 0' EXIT
    (cd backend && cargo run) &
    (cd frontend && bun run dev) &
    wait
```

Two entry points, same effect. New contributors have to guess which is canonical.

### 2.2 Root `package.json` makes Bun feel like the boss

There's a `package.json` at the *root* of the repo even though no JS code lives there. It exists because Bun (and npm/pnpm/yarn) workspaces require a manifest at the workspace root that declares the member packages. The side effect: `node_modules/` lands at the repo root, the only top-level lockfile is `bun.lock`, and the project superficially looks like "a JS repo that happens to contain Rust."

That's a perception issue, not a correctness issue — but it's the perception that prompted the question.

### 2.3 `concurrently` is dragged in only to power the duplicate command

`concurrently` is a root devDependency whose only job is to back the duplicate `package.json` `dev` script. The justfile's `dev` recipe does the same thing in plain bash and doesn't need it.

## 3. Recommended changes

### 3.1 Make `just` the single repo-wide entry point

Remove the `dev` script and `concurrently` dep from the root `package.json`. The root manifest becomes a workspace-declaration shell with no behavior:

```json
{
  "name": "beerio-kart",
  "private": true,
  "workspaces": ["frontend"],
  "devDependencies": {
    "lefthook": "^2.1.4"
  }
}
```

`just dev` becomes the documented way to start the stack. `just --list` doubles as the discovery surface (no equivalent for `bun run` across workspaces without extra scripting).

Rationale: a command runner that works for both halves of the repo gives you symmetric ergonomics. Right now, Rust commands flow through `just`, and JS commands have two paths (`just` *and* `bun run`).

- [ ] Approved
- [ ] Needs discussion

### 3.2 Keep the root `package.json` (don't fight Bun workspaces)

The alternative — drop root `package.json`, move install/build entirely into `frontend/` — does buy a more symmetric `backend/`/`frontend/` feel, but it costs:

- A second `node_modules/` at `frontend/node_modules/` (no real downside, just less elegant).
- No single `bun install` from root; you'd `cd frontend && bun install`.
- Loses the option of adding shared JS packages later (e.g., a generated-types package consumed by the frontend) without re-introducing workspaces.

Recommendation: leave root `package.json` in place, but treat it as a manifest, not a script hub. All "what to do" lives in the justfile.

Sources for the precedent: this is the layout the Turborepo repo itself uses (Cargo workspace + JS workspace co-located, orchestration at the top). See [Turborepo multi-language guide](https://turborepo.dev/docs/guides/multi-language) and the [Turborepo source organization](https://deepwiki.com/vercel/turborepo).

- [ ] Approved
- [ ] Needs discussion

### 3.3 Add a few missing recipes to the justfile

If `just` is the single entry point, it should actually cover the surface area. Today the justfile has `dev`, `test`, `fmt`, `entities`, `build`, `coverage*`. Missing:

- `lint` — runs `cargo clippy --workspace --all-targets -- -D warnings` *and* `cd frontend && bun run lint`.
- `typecheck` — runs `cd frontend && bun run typecheck`. (No Rust equivalent needed; `cargo check` is implied by `cargo build`.)
- `test-frontend` — placeholder for when frontend tests exist.
- `install` — runs `cd frontend && bun install` (and noted that backend deps are fetched on first `cargo build`). Useful for CI and onboarding.
- `clean` — `cd backend && cargo clean` plus removing `node_modules/` and `frontend/dist/`.

Ideally `just check` runs the same checks CI runs, so a contributor can validate locally with one command.

- [ ] Approved
- [ ] Needs discussion

### 3.4 Document the layer split in `docs/design.md`

Two-line addition under whatever architecture section already exists, so a future reader knows the contract:

> **Build layers.** Cargo owns the Rust graph (resolution, incremental compilation, lockfile). Bun + Vite own the TS graph. The root `justfile` is the only repo-wide orchestrator; it shells out to `cargo` and `bun` and never tries to model their dependency graphs itself.

This is the kind of decision that's worth pinning so we don't drift later (e.g., someone adds a Turborepo config and we end up with three layer-2 tools).

- [ ] Approved
- [ ] Needs discussion

## 4. Optional upgrade: `mise` for tool versions

The justfile already says `cargo +nightly fmt` and notes "requires nightly rustfmt — see README Setup." That's a hint the project has implicit toolchain requirements. `mise` (or `asdf`) lets us pin `rust`, `bun`, and `just` versions in a `.mise.toml` so `mise install` reproduces the exact toolchain. ([mise docs](https://mise.jdx.dev/))

Concretely:

```toml
# .mise.toml
[tools]
rust = { version = "1.84", components = "rustfmt,clippy" }
"rust:nightly" = "latest"   # for nightly rustfmt
bun = "1.1"
just = "1.36"
```

Cost: one config file, contributors run `mise install` once. Benefit: deletes the "see README Setup" trail and matches CI exactly.

This is *optional*. Worth doing the next time a toolchain version difference bites someone.

- [ ] Approved
- [ ] Needs discussion (defer)
- [ ] Skip

## 5. Things explicitly NOT to do (yet)

These came up in the survey and I'm flagging them so we don't reach for them prematurely.

### 5.1 Turborepo / Nx

Both add a layer-2 task graph with caching. The wins (remote cache, affected-graph) only matter once CI is slow or there's a second deployable. With one Rust crate + one Vite app, neither earns its config cost. ([Turborepo's own multi-language guide](https://turborepo.dev/docs/guides/multi-language) says non-JS targets still need a `package.json` and have no automatic dep analysis.)

Trigger to revisit: a second deployable, OR CI consistently >5 min that's mostly redundant work.

- [ ] Approved
- [ ] Needs discussion

### 5.2 Bazel / Buck2

Bazel is what Brendan's Bazel intuition is reaching for — a unified action graph, hermetic builds, content-addressed caching. The cost in this repo is enormous:

- A `BUILD.bazel` for every crate plus repinning Cargo lockfiles via `rules_rust`'s `crate_universe`.
- An `aspect_rules_js` configuration that re-implements npm semantics over `bun.lock`.
- Loss of native `cargo` / `bun` ergonomics (rust-analyzer integration, `sea-orm-cli`, `cargo llvm-cov`, `bun run dev` HMR all need shims or break).
- Two-person team carrying that complexity for the duration of the project.

Bazel earns its keep around 100+ engineers, multi-hour CI, or hard hermeticity requirements (regulated builds, supply-chain attestation). Below that it consistently costs more than it returns. ([Bazel: when to use it — Earthly](https://earthly.dev/blog/bazel-build/), [Buck2 is younger and less documented](https://www.buildbuddy.io/blog/buck2-review/), [Pants doesn't yet support Rust or JS well](https://earthly.dev/blog/monorepo-tools/).)

The conceptual gap to bridge: with `just + Cargo + Bun`, there is no single source of truth for the build graph. Each tool owns its own language's graph; the orchestrator is a thin wrapper. That's the deliberate trade — losing graph unification in exchange for using each tool's native, well-supported workflow.

Trigger to revisit: never, for this project. If the project ever grows into multiple deployables sharing protobuf/IDL across languages, look at Buck2 or Nx with the Rust plugin first, not Bazel.

- [ ] Approved
- [ ] Needs discussion

### 5.3 cargo-make

Rust-flavored task runner; would replace `just`. Works fine, but `just` is already in place, is language-neutral (matters for the bun half), and has better discovery (`just --list`). No reason to switch. ([cargo-make](https://sagiegurari.github.io/cargo-make/), [just](https://github.com/casey/just))

- [ ] Approved
- [ ] Needs discussion

## 6. Summary of proposed action

If sections 3.1–3.4 are approved:

1. Edit root `package.json`: drop `scripts.dev` and `devDependencies.concurrently`.
2. Run `bun install` to regenerate `bun.lock` without `concurrently`.
3. Add `lint`, `typecheck`, `install`, `clean`, and a `check` umbrella recipe to the justfile.
4. Append a "Build layers" subsection to `docs/design.md` with the layer split.
5. Update `docs/design.md` history per the Documentation history convention.

Section 4 (mise) is independently deferrable.

## 7. Sources

- [Turborepo multi-language guide](https://turborepo.dev/docs/guides/multi-language)
- [Turborepo's own monorepo layout (DeepWiki)](https://deepwiki.com/vercel/turborepo)
- [Nx vs Turborepo (Nx docs)](https://nx.dev/docs/guides/adopting-nx/nx-vs-turborepo)
- [Earthly: Building a Monorepo with Rust](https://earthly.dev/blog/rust-monorepo/)
- [Earthly: Monorepo Build Tools comparison](https://earthly.dev/blog/monorepo-tools/)
- [Earthly: When to use Bazel](https://earthly.dev/blog/bazel-build/)
- [matklad: Large Rust Workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html) — flat layout justification
- [just (casey/just)](https://github.com/casey/just)
- [Justfile became my favorite task runner](https://tduyng.com/blog/justfile-my-favorite-task-runner/)
- [Taskfile vs Justfile](https://nguyenhuythanh.com/posts/taskfile-vs-justfile/)
- [Just Make a Task — appliedgo](https://appliedgo.net/spotlight/just-make-a-task/)
- [cargo-make](https://sagiegurari.github.io/cargo-make/)
- [Buck2: Why Buck2](https://buck2.build/docs/about/why/)
- [Buck2 Unboxing — BuildBuddy review](https://www.buildbuddy.io/blog/buck2-review/)
- [Bazel (Wikipedia) — language support](https://en.wikipedia.org/wiki/Bazel_(software))
- [Bun: Workspaces](https://bun.com/docs/pm/workspaces)
- [mise docs](https://mise.jdx.dev/)
- [example: spa5k/monorepo-typescript-rust](https://github.com/spa5k/monorepo-typescript-rust)

## Document history

- 2026-05-04 — Initial review created in response to a question about whether the Cargo + Bun + justfile arrangement is sensible. (This file lives under `reviews/`, not `docs/`, so the project-wide Document history convention doesn't strictly apply, but tracking it here keeps the design-session record self-describing.)
