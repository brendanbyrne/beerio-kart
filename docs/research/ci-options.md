# CI options for Beerio Kart

Survey of GitHub Actions worth considering for the Beerio Kart repo. Companion to design record `2026-05-04-ci-adoption.md` which holds the adoption decisions.

Date of survey: 2026-05-04
Author: Cowork (research)
Scope: GitHub Actions for `beerio-kart`. Comprehensive survey, ranked recommendations, phased adoption plan.

## Executive Summary

The repo today has **one** CI workflow (backend coverage via `cargo-llvm-cov` + Codecov) plus a lefthook pre-commit suite (rustfmt nightly, clippy, eslint, prettier). That's a reasonable starting point for a solo prelaunch project but leaves several gaps that are cheap to close and would catch real classes of bugs and supply-chain risk before they hit `main`. The most consequential gaps, in priority order:

1. **No CI-side enforcement of lint/format/build.** Pre-commit hooks can be bypassed with `--no-verify` and don't cover anything not staged. CI must duplicate every blocking check that lefthook runs locally.
2. **No frontend test harness at all.** Vitest is the obvious choice for a Vite + React 19 app and should be wired up before Phase 3 work hardens further.
3. **No supply-chain hygiene.** `cargo-deny`, Dependabot, SHA-pinning of third-party actions, and `dependency-review-action` are all free, near-zero friction, and individually high-value.
4. **No container build/publish workflow.** The Dockerfile exists but nothing builds or pushes images on merge. Once you're ready to deploy, you'll want this.

Everything below is structured as a numbered survey. Each subsection has a recommendation and a sign-off block. Section 12 is a phased adoption plan that rolls all the "Approved" items into a sensible sequence.

A note on this document's recommendations: the "must / nice / skip" buckets are tuned for this project's specific situation — solo dev, prelaunch, public repo, GitHub Actions free tier, self-hosted VPS deploy target. A team project, a paid-tier project, or a published library would weigh several of these differently.

---

## 1. Baseline — what already exists

- **`.github/workflows/coverage.yml`** — runs `cargo llvm-cov` on `pull_request` and `push` to `main`, uploads `lcov.info` to Codecov. Concurrency-cancels in-progress runs. Uses `Swatinem/rust-cache@v2` with `workspaces: backend`. Pinned by tag, not SHA.
- **`codecov.yml`** — project + patch coverage gates (auto target / 80% patch), ignores `entities/`, `migration/`, `main.rs`, `seed.rs`, and the entire `frontend/`.
- **`lefthook.yml`** — pre-commit: `cargo +nightly fmt --check`, `cargo clippy -- -D warnings`, `bunx eslint`, `bunx prettier --check`. Parallel.
- **`Cargo.toml` workspace lints** — already deny warnings, clippy `pedantic` + `nursery` + `cargo`. Some pedantic ones allowed pragmatically.
- **No frontend test runner**, no E2E, no security scans, no container builds, no Dependabot config.

This is a coherent foundation; the gaps below are additive.

---

## 2. Rust backend — build & test gates

### 2.1 Run `cargo nextest run --workspace` instead of `cargo test`

**What:** [`cargo-nextest`](https://github.com/nextest-rs/nextest) runs each test in its own process, parallelizing across all test binaries instead of within each one. ~3× faster on most workloads, plus auto-detects flaky tests, supports per-test retries and timeouts, and emits JUnit XML natively. Drop-in replacement: `cargo nextest run`.

**Caveat:** nextest doesn't run doctests yet — you still need a separate `cargo test --doc` step.

**Friction:** None. Install via `taiki-e/install-action` (prebuilt binary). `cargo-llvm-cov` supports nextest natively (`cargo llvm-cov nextest`), so you can keep coverage on the existing pipeline.

**Recommendation:** **Must.** Replace the implicit test step inside the coverage job with `cargo llvm-cov nextest --workspace --lcov --output-path lcov.info`, and add a separate `cargo test --doc --workspace` step.

### 2.2 Add a fast PR-feedback test job separate from coverage

Coverage runs are slow because instrumentation slows compilation and the Codecov upload adds overhead. A lightweight `cargo nextest run --workspace --all-features` job (no instrumentation) gives sub-2-minute PR feedback, while the heavier coverage job can still run.

Alternatively: keep one job and accept the slightly slower feedback. For a solo dev with a small backend, the difference is marginal — defer this until coverage runs feel slow.

**Recommendation:** **Skip until painful.** One job is fine for now. Revisit if cold-cache CI exceeds ~5 min.

### 2.3 Toolchain matrix (stable / beta / nightly)

Matrix-testing across stable + beta is best practice for libraries with external consumers; for an internal binary with no MSRV declaration it's overkill. Pin the dev toolchain via `rust-toolchain.toml` so CI and contributors agree on the compiler. Bump intentionally.

**Recommendation:** **Skip the matrix.** **Add a `rust-toolchain.toml`** pinning to a specific stable version (e.g., `1.84.0`), since both `dtolnay/rust-toolchain` and `actions-rust-lang/setup-rust-toolchain` will read it automatically. Reproducibility win for free.

### 2.4 Use `actions-rust-lang/setup-rust-toolchain` instead of `dtolnay/rust-toolchain`

**Background:** `actions-rs/*` is [archived and unmaintained](https://github.com/actions-rs/toolchain/issues/216) — don't use it. The two live alternatives are:

- [`dtolnay/rust-toolchain`](https://github.com/dtolnay/rust-toolchain) — minimal, what most ecosystem projects use. No built-in caching.
- [`actions-rust-lang/setup-rust-toolchain`](https://github.com/actions-rust-lang/setup-rust-toolchain) — registers problem matchers (clippy + rustc warnings appear inline on the PR diff), integrates `Swatinem/rust-cache` automatically, reads `rust-toolchain.toml`.

The current coverage workflow uses `dtolnay`. Both work, but the inline annotations from `actions-rust-lang` are nicer when clippy fires on a PR.

**Recommendation:** **Switch to `actions-rust-lang/setup-rust-toolchain`** for new workflows (and migrate `coverage.yml` opportunistically). Pin by SHA, not tag (see §6.5).

### 2.5 SeaORM migration apply check + entity drift check

Your consolidated migration must apply cleanly on every PR — otherwise a contributor's broken migration ships. And the entities under `backend/src/entities/` must stay in sync with the schema, since they're hand-checked in (per CLAUDE.md and `2026-05-02-entity-codegen-strategy.md`).

Two CI steps:

```bash
# 1. Migration applies cleanly to an empty DB
DATABASE_URL=sqlite::memory: cargo run --bin migration -- up

# 2. Entities match the schema
sea-orm-cli generate entity -o /tmp/regen-entities
diff -r backend/src/entities /tmp/regen-entities
```

Step 2 catches the failure mode where someone edits the migration but forgets to regenerate entities. This complements rather than replaces clippy's type checks.

**Recommendation:** **Must.** Add both steps to a `db-check` job. Cheap, catches real bugs.

---

## 3. Rust backend — lint & code quality

### 3.1 CI lint enforcement (rustfmt + clippy)

Pre-commit hooks can be bypassed (`git commit --no-verify`). CI must run the same checks as a blocking gate.

```yaml
- run: cargo +nightly fmt --all -- --check        # nightly per CLAUDE.md
- run: cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Workspace lints already deny warnings, but `-D warnings` on the clippy invocation is belt-and-suspenders for any per-file `#[allow]` overrides that creep in.

**Recommendation:** **Must.** Lefthook isn't a substitute for CI.

### 3.2 `cargo doc` with broken-link check

`cargo doc --no-deps --workspace` with `RUSTDOCFLAGS="-D warnings"` catches broken intra-doc links and missing `# Errors` / `# Panics` sections that pedantic clippy doesn't always flag. Cheap (~30s) and useful given your standards in `docs/coding-standards/rust.md`.

**Recommendation:** **Nice.**

### 3.3 `cargo-machete` (unused dependency detection)

[`cargo-machete`](https://github.com/bnjbvr/cargo-machete) is a fast (~1s) text-based check for unused deps in `Cargo.toml`. False-positive risk for macro-only deps, mitigated by allowlist in `[workspace.metadata.cargo-machete]`. The slower-but-stricter `cargo-udeps` requires nightly + a full compile and is overkill here.

**Recommendation:** **Nice.** Run via the [`bnjbvr/cargo-machete` Action](https://github.com/bnjbvr/cargo-machete) on PR.

### 3.4 `typos` (typo checker)

[`crate-ci/typos`](https://github.com/crate-ci/typos) is a fast typo finder with low false-positive rate. As a CI Action it adds inline PR annotations (file/line/suggestion) that pre-commit can't match. Configure via `_typos.toml` to allowlist project nouns ("Beerio", "SeaORM", "Axum"). Sub-second runtime.

**Recommendation:** **Nice.** Cheap insurance for user-facing strings.

### 3.5 Skip these (called out so they're explicitly considered)

- **`cargo-msrv`** — you don't publish, no MSRV declared.
- **`cargo-semver-checks`** — library tool; your `migration` crate is internal.
- **`cargo-minimal-versions`** — only matters for libraries; your `Cargo.lock` pins everything.
- **`cargo-outdated`** — Dependabot supersedes it.
- **`sccache` for CI** — wins are situational and small for a project this size; `Swatinem/rust-cache` is enough.

---

## 4. Frontend CI — build, lint, test

### 4.1 Bun in CI

**Tool:** [`oven-sh/setup-bun@v2`](https://github.com/oven-sh/setup-bun) is the official action. Pattern:

```yaml
- uses: oven-sh/setup-bun@v2
  with:
    bun-version: 1.2.x
- run: bun install --frozen-lockfile
```

Bun's text-based `bun.lock` (replaced binary `bun.lockb` in v1.1.39) plays well with CI. `setup-bun` caches the global install dir between runs. Bun is ~10–30× faster than npm cold and ~3× warm.

**Recommendation:** **Must keep Bun in CI** — falling back to npm reintroduces a parallel toolchain mismatch with local dev, which is exactly what CI is supposed to catch.

### 4.2 Build verification + typecheck

`bun run build` already runs `tsc -b && vite build`. Per [TypeScript #38440](https://github.com/microsoft/TypeScript/issues/38440), `--noEmit` is incompatible with `-b`, so `bun run typecheck` is a *separate* invocation, not redundant — it fails faster and produces clearer error output when the issue is purely typing.

**Recommendation:** **Must.** Two parallel jobs in CI: `bun run build` and `bun run typecheck`.

Vite 8 compatibility note: requires Node 20.19+ / 22.12+. Bun runtime handles this fine, but if any other CI step shells out to Node (Lighthouse CI, Playwright's bundled Node), pin `actions/setup-node@v4` accordingly.

### 4.3 ESLint + Prettier in CI (not just pre-commit)

```yaml
- run: bun run lint -- --max-warnings=0
- run: bunx prettier --check .
```

`--max-warnings=0` prevents warning creep. Same rationale as §3.1 — pre-commit can be bypassed.

**Recommendation:** **Must.**

### 4.4 Wire up Vitest + React Testing Library

The frontend has no test runner today. [Vitest](https://vitest.dev/guide/comparisons.html) is the natural fit for a Vite + React app — same config, same transform pipeline, same plugin ecosystem. Vitest 3.x supports React 19. Pair with `@testing-library/react` for component tests.

For component tests that need real DOM/layout, use [Vitest Browser Mode](https://vitest.dev/guide/browser/) (powered by Playwright under the hood). Skip Playwright Component Testing — Vitest Browser Mode does the same job with better DX.

**Recommendation:** **Must (wire it up now), even if test count starts small.** Milestone Star is going to start producing more frontend logic; deferring the harness gets harder, not easier.

### 4.5 Playwright E2E + axe accessibility

[Playwright](https://playwright.dev/docs/ci) with sharding (matrix `shardIndex: [1,2,3,4]`, then `playwright merge-reports` for unified output). For mobile reference (Pixel 9 Pro per CLAUDE.md):

```ts
use: { viewport: { width: 427, height: 952 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true }
```

Pin to Chromium-only initially — Firefox + WebKit triple your CI minutes. (You're targeting Firefox per `docs/design.md`, so Firefox should be added back once flows stabilize.)

Pair with [`@axe-core/playwright`](https://playwright.dev/docs/accessibility-testing) for accessibility assertions in E2E specs. `axe` finds more issues than `pa11y` and integrates in one import. Inclusivity is in your design principles — wire a11y checks in early.

**Recommendation:** **Nice now → Must when Milestone Star auth/session flows land.** Scaffold the directory + one smoke test now so it's not a sudden lift later.

### 4.6 Bundle size monitoring & Lighthouse CI

- [`size-limit`](https://github.com/ai/size-limit) + [`andresz1/size-limit-action`](https://github.com/andresz1/size-limit-action) — PR comment with size delta. `bundlesize` is in maintenance; `size-limit` is the active choice. Sub-100 KB app today, so not urgent.
- [`treosh/lighthouse-ci-action`](https://github.com/treosh/lighthouse-ci-action) — runs Lighthouse against a `vite preview` server in CI; `temporaryPublicStorage: true` uploads reports for 7 days at no cost. Mobile-first means a strict LCP / TBT / a11y budget is meaningful.

**Recommendation:** **Both nice, defer.** Add `size-limit` once the bundle starts growing (Phase 4-ish). Add Lighthouse CI once a deployable preview URL exists.

### 4.7 Skip visual regression for now

Chromatic / Percy have nontrivial setup and Storybook coupling. Playwright `toHaveScreenshot()` works but is flaky across font rendering and only earns its keep when there's a designer or multi-contributor review need. Defer until those exist.

**Recommendation:** **Skip.** Mention in case the question comes up.

---

## 5. Caching & action conventions

### 5.1 `Swatinem/rust-cache@v2` configuration

Already used in `coverage.yml` with `workspaces: backend`. Recommended additions for any new Rust job:

- `shared-key: "ci"` — so format/clippy/test/audit jobs hit the same cache entry instead of fragmenting.
- `save-if: ${{ github.ref == 'refs/heads/main' }}` — PR runs read but don't pollute the cache.

**Recommendation:** **Approve as a convention** for new Rust jobs.

### 5.2 `taiki-e/install-action` for installing cargo tools

[`taiki-e/install-action`](https://github.com/taiki-e/install-action) installs `cargo-deny`, `cargo-nextest`, `cargo-llvm-cov`, `cargo-machete`, `typos` etc. as prebuilt binaries (with SHA256 + attestation verification) in seconds, falling back to `cargo-binstall` then `cargo install`. The current coverage workflow already uses it for `cargo-llvm-cov`.

**Recommendation:** **Standard convention** — use it for any cargo-tool install in CI.

### 5.3 Concurrency cancellation on every workflow

Already in `coverage.yml`. Apply the same pattern to every new workflow:

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
```

Cancel on PR pushes (latest commit wins), don't cancel on `main` / release runs (never interrupt a deploy).

**Recommendation:** **Standard convention.**

---

## 6. Security & supply chain

### 6.1 `cargo-deny` (advisories + bans + licenses + sources)

[`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) is a superset of `cargo-audit` — same RustSec advisory DB plus license allowlist, dep bans (incl. duplicate-version detection), and source restrictions. Has an [official Action](https://github.com/EmbarkStudios/cargo-deny-action) with SARIF output for the GitHub code-scanning tab.

`cargo-audit`'s primary maintainer [stepped back in 2024](https://shnatsel.medium.com/i-am-stepping-back-from-maintaining-cargo-audit-35bb5f832d43); the WG continues maintenance, but for new pipelines `cargo-deny` is the better target.

**Recommendation:** **Must.** Use `cargo-deny` only — skip `cargo-audit` (redundant). Run `cargo deny check advisories bans sources licenses` on every PR.

License gate is genuinely useful even on a closed-source backend — catches a copyleft transitive dep before it ships.

### 6.2 CodeQL (now GA for Rust)

CodeQL's [Rust support went GA in October 2025](https://github.blog/changelog/2025-10-23-codeql-2-23-3-adds-a-new-rust-query-rust-support-and-easier-c-c-scanning/) (CodeQL 2.23.3). [JS/TS has been GA for years](https://codeql.github.com/docs/codeql-overview/supported-languages-and-frameworks/). Free for public repos.

**Recommended setup:** Use **default setup** (Settings → Code security → CodeQL → Set up → Default) for both languages. Switch to advanced (workflow file) only if you need custom query packs.

**Recommendation:** **Must.** Default setup is two clicks and runs on a schedule + on PRs. SARIF output appears in the Security tab.

### 6.3 Dependabot — version + security updates

Dependabot is free, native, and creates PRs that your existing CI gates. Configure for four ecosystems:

```yaml
version: 2
updates:
  - package-ecosystem: cargo
    directory: /backend
    schedule: { interval: weekly }
  - package-ecosystem: npm                # supports Bun's bun.lock — see flag below
    directory: /
    schedule: { interval: weekly }
  - package-ecosystem: docker
    directory: /
    schedule: { interval: weekly }
  - package-ecosystem: github-actions
    directory: /
    schedule: { interval: weekly }
```

**Bun-specific flag:** [Dependabot added Bun support in February 2025](https://github.blog/changelog/2025-02-13-dependabot-version-updates-now-support-the-bun-package-manager-ga/) — supports text `bun.lock` (not legacy `bun.lockb`), Bun ≥1.1.39. **However, [`dependabot-core#14223`](https://github.com/dependabot/dependabot-core/issues/14223) (Feb 2026, still open) reports Dependabot doesn't update `bun.lock` correctly when the project uses npm-style workspaces.** Your root `package.json` declares `"workspaces": ["frontend"]`, so this likely affects you. Practical workaround: re-run `bun install` after merging Dependabot PRs to keep the lockfile in sync, or watch the bug for a fix.

Also: **enable security updates separately** (Settings → Code security → Dependabot security updates). They fire only when an advisory matches a pinned dep, not on a schedule.

**Recommendation:** **Must.** Add the config + enable security updates. Watch the workspace bug.

### 6.4 `actions/dependency-review-action`

[`actions/dependency-review-action`](https://github.com/actions/dependency-review-action) diffs the dependency graph on PR and fails the build if a PR introduces a known-vulnerable dep or a disallowed license. Catches the case Dependabot doesn't: vulns in deps a PR *adds*. Free for public repos. Tiny config.

**Recommendation:** **Must.**

### 6.5 Pin third-party actions to commit SHA

In March 2025 the `tj-actions/changed-files` action was [compromised (CVE-2025-30066)](https://github.com/advisories/ghsa-mrrh-fwg8-r2c3) — the attacker pushed malicious code under existing tags, exfiltrating secrets from ~23,000 public repos. Tags are mutable; commit SHAs are not. The same incident also exposed [`reviewdog/action-setup`](https://www.cisa.gov/news-events/alerts/2025/03/18/supply-chain-compromise-third-party-github-action-cve-2025-30066) (CVE-2025-30154). Pinning by SHA is now mainstream advice.

Two approaches:

1. **One-time conversion + Dependabot maintenance.** Use [StepSecurity's Secure Workflow](https://app.stepsecurity.io/secure-workflow) (web tool) or [`sethvargo/ratchet`](https://github.com/sethvargo/ratchet) (CLI) to rewrite all `uses:` lines from tag form to `pin@<sha>  # vX.Y.Z` form. Dependabot's `package-ecosystem: github-actions` then keeps SHAs current; comments preserve human-readable version.
2. **Renovate with `helpers:pinGitHubActionDigests`** — auto-pins on PR and rebases SHA bumps. More flexible, slightly more setup than Dependabot.

**Pragmatic carve-out:** GitHub-owned actions (`actions/*`, `github/*`) are lower-risk; some teams skip pinning those. Recommend pinning everything for consistency.

The current `coverage.yml` uses tag-pinned actions (`actions/checkout@v4`, etc.). Should be migrated.

**Recommendation:** **Must.** Run a one-time SHA-pin over `coverage.yml` and any new workflow at write time. Dependabot will keep them current.

### 6.6 GitHub native secret scanning + push protection

Free for public repos, on by default. Includes push protection (Settings → Code security). Covers ~200 partner-validated patterns.

Third-party scanners (`gitleaks`, `trufflehog`) add custom-regex coverage but the marginal value is low for a project that hasn't defined custom secret formats.

**Recommendation:** **Must verify** push protection is enabled. **Skip** the action-based scanners.

### 6.7 OSSF Scorecard

[`ossf/scorecard-action`](https://github.com/ossf/scorecard-action) grades a public repo on ~18 supply-chain hygiene checks (branch protection, pinned actions, signed releases, token permissions, etc.). Useful as a one-time audit; running it weekly produces noise.

**Recommendation:** **Nice.** Run once, fix the easy wins, then disable or schedule monthly.

### 6.8 `zizmor` (workflow security analyzer)

[`zizmorcore/zizmor`](https://github.com/woodruffw/zizmor) is a static analyzer specifically for GitHub Actions security issues — template injection (`${{ github.event.pull_request.title }}` in shell), over-permissive `GITHUB_TOKEN`, untrusted-checkout patterns, mutable-tag pinning. Used by PyPI Warehouse, pip-audit, astral-sh. Sponsored by Grafana Labs. Complements `actionlint` (which checks correctness, not security).

**Recommendation:** **Nice.** ~5-line workflow that runs on every workflow change. Free for public repos. Outputs SARIF for the Security tab.

### 6.9 Skip these (called out)

- **StepSecurity Harden-Runner** — egress filtering. Audit mode is cheap visibility but block mode requires non-trivial allowlist tuning. Skip until you handle real secrets in CI.
- **`cargo-geiger`** (unsafe-block counter) — informational, not actionable on a stack like Axum/SeaORM where unsafe lives in deep deps. Skip.
- **Frontend license tools** — `dependency-review-action` (§6.4) covers it. Skip a separate npm license tool.

---

## 7. Container build & publish

### 7.1 Build + push to GHCR

Standard pipeline:

```yaml
- uses: docker/setup-buildx-action@<sha>
- uses: docker/metadata-action@<sha>      # derives tags from refs (sha, branch, semver)
- uses: docker/login-action@<sha>
  with:
    registry: ghcr.io
    username: ${{ github.actor }}
    password: ${{ secrets.GITHUB_TOKEN }}
- uses: docker/build-push-action@<sha>
  with:
    push: true
    cache-from: type=gha
    cache-to: type=gha,mode=max
    tags: ${{ steps.meta.outputs.tags }}
```

Permissions: `packages: write` on the job. GHCR is free for public repos (unlimited).

**Multi-arch:** Skip arm64 cross-build via QEMU — it's "several times slower" because every ARM instruction is translated. amd64-only fits a typical VPS. Revisit if you ever deploy to a Pi or Graviton host (use two native runners + manifest merge, not QEMU).

**Cache budget caveat:** GitHub Actions cache is capped at 10 GB and Rust release builds with cargo-chef can fill it fast. If you hit the cap, switch to `type=registry,ref=ghcr.io/<you>/beerio-kart:buildcache,mode=max` — registry-backed cache lives in GHCR (no 10 GB limit for public repos) and works just as well. Try `type=gha` first.

**Recommendation:** **Must (when you're ready to deploy).**

### 7.2 Hadolint (Dockerfile linter)

[`hadolint/hadolint-action`](https://github.com/hadolint/hadolint-action) catches the usual footguns: unpinned `apt-get`, missing `--no-install-recommends`, root user, etc. Five-line workflow step, ~5s runtime. Your current `Dockerfile` would benefit — quick scan suggests no major findings, but worth running.

**Recommendation:** **Nice.** Cheap, runs once on Dockerfile changes.

### 7.3 Trivy container scan (with caveat)

[`aquasecurity/trivy-action`](https://github.com/aquasecurity/trivy-action) scans built images for CVEs. Apache 2.0, no usage limits.

**Caveat:** [Trivy's release infra was reportedly compromised in March 2026](https://lucaberton.com/blog/trivy-vs-grype-2026/) — hijacked tags, malicious images on Docker Hub; DB updates suspended late March. Pin to a commit SHA, not a floating tag.

**Recommended config:** `--severity HIGH,CRITICAL --exit-code 1` so noisy LOW/MEDIUM doesn't block PRs. Run after build, before push.

**Alternative:** [`anchore/scan-action`](https://github.com/anchore/scan-action) (Grype) is vulnerability-only but faster and includes EPSS/KEV prioritization. Either is fine. Trivy has broader coverage (also IaC, secrets, licenses).

**Recommendation:** **Must (with deploys).** Pin by SHA after the supply-chain incident.

### 7.4 SBOM generation + image signing

- **SBOM** ([`anchore/sbom-action`](https://github.com/anchore/sbom-action)) — only useful when downstream consumers verify it. You're the only consumer of your own image. **Skip.**
- **Cosign keyless signing** ([`sigstore/cosign-installer`](https://github.com/sigstore/cosign-installer)) — same logic. Nobody is verifying your signatures. **Skip.**

Revisit both if you ever publish images for others.

**Recommendation:** **Skip.**

### 7.5 GHCR retention policy

Untagged + old tagged images accumulate. [`snok/container-retention-policy`](https://github.com/snok/container-retention-policy) on a weekly cron: keep last 10 tagged + delete untagged older than 14 days. Caveat: naive untagged deletion can break multi-arch manifests; the 14-day buffer protects against this.

**Recommendation:** **Nice.** Set up once you've pushed enough images to care.

---

## 8. Deployment to self-hosted VPS

The four realistic patterns, ranked for your situation:

### 8.1 Recommended path: SSH-from-CI now, Tailscale later

| Pattern | How it works | Verdict for you |
|---|---|---|
| **(a) `appleboy/ssh-action`** | CI SSHes to VPS, runs `docker pull && docker compose up -d`. SSH key in `secrets.SSH_KEY`. | **Recommended start.** Use a restricted deploy user (forced command, no shell), ed25519 key, non-22 port, fail2ban. Rotate key annually. |
| **(b) Watchtower / Diun (pull-based)** | Daemon on VPS polls GHCR for new images. **Note: containrrr/Watchtower [is archived](https://github.com/containrrr/watchtower) as of 2024.** [Diun](https://crazymax.dev/diun/) (notify-only) is the active equivalent. | **Use Diun as a watchdog**, not as your deploy mechanism. Watchtower-style auto-update bites you when an upstream image breaks. |
| **(c) Tailscale + SSH** | [`tailscale/github-action`](https://tailscale.com/kb/1276/tailscale-github-action) joins the runner to your tailnet (ephemeral OAuth client + ephemeral auth keys). SSH over tailnet, no public SSH port. | **Best security/effort once Tailscale is set up.** Migrate from (a) when you tire of exposing SSH publicly. Free tier covers 100 devices easily. |
| **(d) Self-hosted runner on the VPS** | GitHub dispatches jobs directly to the VPS. | **Don't.** GitHub explicitly warns against self-hosted runners on public repos — any forked PR's workflow code can persistently compromise the box. |

**Recommendation:** **Start with (a) `appleboy/ssh-action`.** Run [Diun](https://crazymax.dev/diun/) on the VPS as a base-image-CVE watchdog. Plan to migrate to (c) Tailscale within ~6 months of go-live.

### 8.2 Health check + rollback after deploy

After the deploy step, hit `/healthz` with retries via [`Jtalk/url-health-check-action`](https://github.com/Jtalk/url-health-check-action). On failure, `docker compose` rollback to the previous image tag (keep `:previous` as a moving tag updated post-success).

**Recommendation:** **Nice.** ~10 lines of YAML, pays for itself the second time a deploy goes wrong.

### 8.3 OIDC for deploys (informational)

OIDC (`permissions: id-token: write`) lets workflows mint short-lived cloud credentials per run, eliminating long-lived secrets. **GHCR is already OIDC-handled** via `GITHUB_TOKEN` + `packages: write` — non-issue. Self-hosted VPS isn't OIDC-aware, so SSH key remains the auth path.

**Recommendation:** **Note in the design doc**, no action needed.

---

## 9. Repo & workflow hygiene

### 9.1 `actionlint` (workflow YAML linter)

[`rhysd/actionlint`](https://github.com/rhysd/actionlint) type-checks `${{ }}` expressions, validates action inputs against a metadata DB, catches deprecated commands like `::set-output`, validates runner labels. Generic YAML linters miss all of this. ~10s on `.github/workflows/**` changes.

**Recommendation:** **Must.** Cheap, catches real bugs.

### 9.2 Switch to Repository Rulesets, not legacy branch protection

[Repository Rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets) are GitHub's modern (2023+) replacement for branch protection. Free on public repos. Wins:

- Multiple rulesets stack (vs. one branch-protection rule winning).
- Status-check matching by *integration source*, not exact name string — fixes the "rename a job and forget to update protection" footgun.
- "Bypass list" — emergency-merge without disabling the rule.

**Solo-dev recommended ruleset on `main`:**

- Require PR before merge (review count = 0; you can't review your own PR).
- Require status checks: list every CI gate (cargo nextest, cargo clippy, cargo deny, db-check, frontend build, frontend lint, frontend typecheck, actionlint, container build).
- Require branches up-to-date.
- Block force-push, block deletion.
- Add yourself to bypass list for break-glass.
- **Skip CODEOWNERS** — solo dev.

**Recommendation:** **Must.**

### 9.3 Documentation checks

| Tool | What | Verdict |
|---|---|---|
| [`DavidAnson/markdownlint-cli2-action`](https://github.com/DavidAnson/markdownlint-cli2-action) | Markdown style/structure | **Nice with tuned config** — disable MD013 (line length) and MD041 (first-line H1) or it'll fight `docs/design.md` hard. |
| [`lycheeverse/lychee-action`](https://github.com/lycheeverse/lychee-action) | HTTP link checker (Rust-based, fast) | **Nice on schedule, not blocking.** Args: `--cache --max-cache-age 1d --retry-wait-time 10 --max-retries 3 --accept 200,206,429`. Schedule weekly with `fail: false` + issue-creation on errors so transient 5xx/429s don't break PRs. |
| Mermaid validation | Diagram syntax | **Skip** — no mermaid blocks in docs today. |

### 9.4 Skip these (called out)

- **`commitlint` / conventional-commits enforcement** — solo dev controls 100% of commits. Cheap to add later if you adopt `release-plz`.
- **`amannn/action-semantic-pull-request`** (PR title linting) — only matters if you adopt squash-merge + auto-changelog. Skip until §10 changes.
- **`actions/labeler`, `release-drafter`** — solo workflow doesn't benefit.
- **`shellcheck`** — useful only if/when you add shell scripts. Skip until then.
- **`yamllint`** — actionlint covers workflow YAML; nothing else justifies a second linter.

---

## 11. Open questions

These are decisions Brendan should make before §12 kicks off:

1. **Repo visibility.** This research assumes the repo is **public** (free CI minutes, free CodeQL, free Scorecard, free GHCR). If it's private, several recommendations change (free-tier minute budget tightens to 2,000/mo; CodeQL needs Advanced Security; Scorecard is irrelevant). Confirm.
2. **Conventional Commits stance.** Adopting `release-plz` later assumes commits follow conventional format. Decide whether to enforce that now (cheap with `wagoid/commitlint-github-action`) or accept retroactive cleanup later.
3. **Codecov coverage flag.** Current `codecov.yml` ignores `frontend/` entirely. When Vitest lands (§4.4), do we want frontend coverage tracked too, or keep it backend-only? (Recommend: track both with separate flags so backend regressions don't get hidden by frontend coverage growth.)
4. **VPS readiness.** Sections 7 and 8 assume you'll have a VPS reachable from GHCR-pulls within ~3 months. If that's later, defer those sections.

---

## Sources

### Rust CI
- [`actions-rust-lang/setup-rust-toolchain`](https://github.com/actions-rust-lang/setup-rust-toolchain)
- [`dtolnay/rust-toolchain`](https://github.com/dtolnay/rust-toolchain)
- [`actions-rs/toolchain` archived](https://github.com/actions-rs/toolchain/issues/216)
- [`Swatinem/rust-cache`](https://github.com/Swatinem/rust-cache)
- [`taiki-e/install-action`](https://github.com/taiki-e/install-action)
- [`cargo-nextest`](https://nexte.st/) and [benchmarks](https://nexte.st/docs/benchmarks/)
- [`taiki-e/cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov)
- [`EmbarkStudios/cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) and [Action](https://github.com/EmbarkStudios/cargo-deny-action)
- [shnatsel — stepping back from cargo-audit](https://shnatsel.medium.com/i-am-stepping-back-from-maintaining-cargo-audit-35bb5f832d43)
- [`bnjbvr/cargo-machete`](https://github.com/bnjbvr/cargo-machete)
- [`crate-ci/typos`](https://github.com/crate-ci/typos)
- [Rust Project Primer — GitHub Actions](https://rustprojectprimer.com/ci/github.html)

### Frontend CI
- [`oven-sh/setup-bun`](https://github.com/oven-sh/setup-bun)
- [Bun CI/CD guide](https://bun.com/docs/guides/runtime/cicd)
- [Bun text-based lockfile](https://bun.com/blog/bun-lock-text-lockfile)
- [Vite 8 announcement](https://vite.dev/blog/announcing-vite8)
- [TypeScript `--noEmit` + `-b` incompatibility](https://github.com/microsoft/TypeScript/issues/38440)
- [Vitest comparisons](https://vitest.dev/guide/comparisons.html)
- [Vitest Browser Mode](https://vitest.dev/guide/browser/)
- [Playwright CI docs](https://playwright.dev/docs/ci) and [sharding](https://playwright.dev/docs/test-sharding)
- [Playwright accessibility testing](https://playwright.dev/docs/accessibility-testing)
- [`treosh/lighthouse-ci-action`](https://github.com/treosh/lighthouse-ci-action)
- [`size-limit`](https://github.com/ai/size-limit) + [Action](https://github.com/andresz1/size-limit-action)

### Security & supply chain
- [CodeQL Rust GA — Oct 2025 changelog](https://github.blog/changelog/2025-10-23-codeql-2-23-3-adds-a-new-rust-query-rust-support-and-easier-c-c-scanning/)
- [CodeQL supported languages](https://codeql.github.com/docs/codeql-overview/supported-languages-and-frameworks/)
- [Dependabot Bun GA — Feb 2025](https://github.blog/changelog/2025-02-13-dependabot-version-updates-now-support-the-bun-package-manager-ga/)
- [Dependabot Bun + npm-workspace bug (#14223)](https://github.com/dependabot/dependabot-core/issues/14223)
- [Dependabot supported ecosystems](https://docs.github.com/en/code-security/reference/supply-chain-security/supported-ecosystems-and-repositories)
- [`actions/dependency-review-action`](https://github.com/actions/dependency-review-action)
- [tj-actions/changed-files compromise — Wiz](https://www.wiz.io/blog/github-action-tj-actions-changed-files-supply-chain-attack-cve-2025-30066)
- [CISA alert — tj-actions + reviewdog](https://www.cisa.gov/news-events/alerts/2025/03/18/supply-chain-compromise-third-party-github-action-cve-2025-30066)
- [CVE-2025-30066 advisory](https://github.com/advisories/ghsa-mrrh-fwg8-r2c3)
- [`sethvargo/ratchet`](https://github.com/sethvargo/ratchet)
- [StepSecurity Secure Workflow](https://app.stepsecurity.io/secure-workflow)
- [`ossf/scorecard-action`](https://github.com/ossf/scorecard-action)
- [`zizmorcore/zizmor`](https://github.com/woodruffw/zizmor) and [Grafana Labs writeup](https://grafana.com/blog/how-to-detect-vulnerable-github-actions-at-scale-with-zizmor/)

### Container & deploy
- [Docker GitHub Actions multi-platform docs](https://docs.docker.com/build/ci/github-actions/multi-platform/)
- [GitHub Actions cache backend](https://docs.docker.com/build/cache/backends/gha/)
- [Registry cache backend](https://docs.docker.com/build/cache/backends/registry/)
- [Working with the GHCR](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry)
- [Trivy vs Grype 2026 (incl. Trivy supply-chain incident)](https://lucaberton.com/blog/trivy-vs-grype-2026/)
- [`hadolint/hadolint-action`](https://github.com/hadolint/hadolint-action)
- [`appleboy/ssh-action`](https://github.com/appleboy/ssh-action)
- [Watchtower archived; alternatives](https://linuxhandbook.com/blog/watchtower-like-docker-tools/)
- [Diun](https://crazymax.dev/diun/)
- [Tailscale GitHub Action](https://tailscale.com/kb/1276/tailscale-github-action) and [secure GH runners guide](https://tailscale.com/kb/1586/secure-github-runners)
- [GitHub: secure use reference (self-hosted runners)](https://docs.github.com/en/actions/reference/security/secure-use)
- [`snok/container-retention-policy`](https://github.com/snok/container-retention-policy)
- [`Jtalk/url-health-check-action`](https://github.com/Jtalk/url-health-check-action)

### Repo & workflow hygiene
- [`rhysd/actionlint`](https://github.com/rhysd/actionlint)
- [GitHub Repository Rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets)
- [GitHub workflow concurrency docs](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency)
- [`DavidAnson/markdownlint-cli2-action`](https://github.com/DavidAnson/markdownlint-cli2-action)
- [`lycheeverse/lychee-action`](https://github.com/lycheeverse/lychee-action) and [GitHub Actions setup guide](https://lychee.cli.rs/continuous-integration/github/)
- [`release-plz`](https://github.com/release-plz/release-plz) and [Orhun's release automation post](https://blog.orhun.dev/automated-rust-releases/)

---

## Document history

- 2026-05-05 — Created in `docs/research/` by splitting from the 2026-05-04 `reviews/design/2026-05-04-ci-research.md` per the triage decision in `docs/designs/2026-05-04-design-doc-restructure.md` §6.5. PR #41.
- 2026-05-08 — Repaired broken lychee-action recipes URL in the "Repo & workflow hygiene" bullet: `https://lychee.cli.rs/github_action_recipes/check-repository/` → `https://lychee.cli.rs/continuous-integration/github/`. Old path was removed in lychee's site redesign; new path is the closest equivalent (CI integration guide). Updated link text to match. PR 5 of the docs restructure (plan deviation — surfaced when lychee `fail: true` flipped on).
