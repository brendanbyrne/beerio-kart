# CI adoption — design record (2026-05-04)

Date: 2026-05-04
Author: Cowork
Source survey: `docs/research/ci-options.md`

## Overview

This design record holds the adoption decisions for GitHub Actions surveyed in `docs/research/ci-options.md`. Each subsection numbered §N.M corresponds to the same-numbered subsection in the research file. Sign off section by section.

---

## 2. Rust backend — build & test gates

### 2.1 Adopt: Run `cargo nextest run --workspace` instead of `cargo test`

See `docs/research/ci-options.md` § 2.1 for context. Replace the implicit test step inside the coverage job with `cargo llvm-cov nextest --workspace --lcov --output-path lcov.info`, and add a separate `cargo test --doc --workspace` step.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 2.2 Skip: Add a fast PR-feedback test job separate from coverage

See `docs/research/ci-options.md` § 2.2 for context. One job is fine for now; revisit if cold-cache CI exceeds ~5 min.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 2.3 Skip: Toolchain matrix (stable / beta / nightly)

See `docs/research/ci-options.md` § 2.3 for context. Skip the matrix, but add a `rust-toolchain.toml` pinning to a specific stable version for reproducibility.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 2.4 Adopt: Use `actions-rust-lang/setup-rust-toolchain` instead of `dtolnay/rust-toolchain`

See `docs/research/ci-options.md` § 2.4 for context. Switch to `actions-rust-lang/setup-rust-toolchain` for new workflows and migrate `coverage.yml` opportunistically. Pin by SHA, not tag.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 2.5 Adopt: SeaORM migration apply check + entity drift check

See `docs/research/ci-options.md` § 2.5 for context. Add both steps to a `db-check` job to catch broken migrations and entity drift at PR time.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

---

## 3. Rust backend — lint & code quality

### 3.1 Adopt: CI lint enforcement (rustfmt + clippy)

See `docs/research/ci-options.md` § 3.1 for context. Run the same lint checks in CI as a blocking gate that lefthook runs locally.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 3.2 Adopt: `cargo doc` with broken-link check

See `docs/research/ci-options.md` § 3.2 for context. Catch broken intra-doc links and missing documentation sections.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 3.3 Adopt: `cargo-machete` (unused dependency detection)

See `docs/research/ci-options.md` § 3.3 for context. Run via the `bnjbvr/cargo-machete` Action on PR for fast unused-dep detection.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 3.4 Adopt: `typos` (typo checker)

See `docs/research/ci-options.md` § 3.4 for context. Run `crate-ci/typos` with a `_typos.toml` to allowlist project nouns for cheap insurance on user-facing strings.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 3.5 Skip: `cargo-msrv`, `cargo-semver-checks`, `cargo-minimal-versions`, `cargo-outdated`, `sccache`

See `docs/research/ci-options.md` § 3.5 for context. These are not applicable for this project.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

---

## 4. Frontend CI — build, lint, test

### 4.1 Adopt: Bun in CI

See `docs/research/ci-options.md` § 4.1 for context. Keep Bun in CI using `oven-sh/setup-bun@v2` to avoid toolchain mismatch with local dev.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 4.2 Adopt: Build verification + typecheck

See `docs/research/ci-options.md` § 4.2 for context. Run `bun run build` and `bun run typecheck` as parallel jobs in CI.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 4.3 Adopt: ESLint + Prettier in CI (not just pre-commit)

See `docs/research/ci-options.md` § 4.3 for context. Run linters in CI with `--max-warnings=0` to prevent warning creep and bypass via `--no-verify`.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 4.4 Adopt: Wire up Vitest + React Testing Library

See `docs/research/ci-options.md` § 4.4 for context. Install Vitest and `@testing-library/react` now to provide a test harness before Milestone Star hardens.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 4.5 Adopt later: Playwright E2E + axe accessibility

See `docs/research/ci-options.md` § 4.5 for context. Scaffold the directory and a smoke test now; wire into required checks later when Milestone Star auth flows land.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 4.6 Defer: Bundle size monitoring & Lighthouse CI

See `docs/research/ci-options.md` § 4.6 for context. Add `size-limit` once the bundle grows and Lighthouse CI once a deployable preview URL exists.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 4.7 Skip: Visual regression

See `docs/research/ci-options.md` § 4.7 for context. Defer until there's a designer or multi-contributor review need.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

---

## 5. Caching & action conventions

### 5.1 Adopt: `Swatinem/rust-cache@v2` configuration

See `docs/research/ci-options.md` § 5.1 for context. Use `shared-key: "ci"` and `save-if: ${{ github.ref == 'refs/heads/main' }}` as a convention for all new Rust jobs.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 5.2 Adopt: `taiki-e/install-action` for cargo tool installs

See `docs/research/ci-options.md` § 5.2 for context. Use this action as a standard convention for any cargo-tool install in CI.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 5.3 Adopt: Concurrency cancellation on every workflow

See `docs/research/ci-options.md` § 5.3 for context. Apply concurrency groups that cancel on PR pushes but not on `main` / release runs.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

---

## 6. Security & supply chain

### 6.1 Adopt: `cargo-deny` (advisories + bans + licenses + sources)

See `docs/research/ci-options.md` § 6.1 for context. Use `cargo-deny` as the primary security gate; run `cargo deny check advisories bans sources licenses` on every PR.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 6.2 Adopt: CodeQL (now GA for Rust)

See `docs/research/ci-options.md` § 6.2 for context. Enable CodeQL default setup for both Rust and JS/TS via the GitHub UI.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 6.3 Adopt: Dependabot — version + security updates

See `docs/research/ci-options.md` § 6.3 for context. Configure Dependabot for cargo, npm, docker, and github-actions with weekly checks. Enable security updates separately. Watch the npm-workspace bug.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 6.4 Adopt: `actions/dependency-review-action`

See `docs/research/ci-options.md` § 6.4 for context. Add this action to fail builds when PRs introduce known-vulnerable or disallowed-license deps.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 6.5 Adopt: Pin third-party actions to commit SHA

See `docs/research/ci-options.md` § 6.5 for context. Run a one-time SHA-pin pass over existing workflows and pin all new actions by SHA. Dependabot will keep them current.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 6.6 Adopt: Verify GitHub native secret scanning + push protection

See `docs/research/ci-options.md` § 6.6 for context. Ensure push protection is enabled in settings. Skip third-party scanners.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 6.7 Adopt: OSSF Scorecard (one-time audit)

See `docs/research/ci-options.md` § 6.7 for context. Run once, fix easy wins, then disable or schedule monthly.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 6.8 Adopt: `zizmor` (workflow security analyzer)

See `docs/research/ci-options.md` § 6.8 for context. Add a ~5-line workflow that runs on every workflow change to catch template injection and over-permissive token usage.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 6.9 Skip: StepSecurity Harden-Runner, `cargo-geiger`, frontend license tools

See `docs/research/ci-options.md` § 6.9 for context. These are not applicable for this project at this stage.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

---

## 7. Container build & publish

### 7.1 Adopt when deploying: Build + push to GHCR

See `docs/research/ci-options.md` § 7.1 for context. When ready to deploy, add a workflow with `docker/setup-buildx-action`, `docker/metadata-action`, `docker/login-action`, and `docker/build-push-action`.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 7.2 Adopt when deploying: Hadolint (Dockerfile linter)

See `docs/research/ci-options.md` § 7.2 for context. Add cheap Dockerfile linting to catch common footguns once container builds are set up.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 7.3 Adopt when deploying: Trivy container scan

See `docs/research/ci-options.md` § 7.3 for context. Scan built images for CVEs with HIGH+CRITICAL severity gate. Pin to commit SHA after the supply-chain incident.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 7.4 Skip: SBOM generation + image signing

See `docs/research/ci-options.md` § 7.4 for context. Skip until you publish images for others to consume.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 7.5 Adopt later: GHCR retention policy

See `docs/research/ci-options.md` § 7.5 for context. Add `snok/container-retention-policy` once you've pushed enough images to care about cleanup.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

---

## 8. Deployment to self-hosted VPS

### 8.1 Adopt: SSH-from-CI now, Tailscale later

See `docs/research/ci-options.md` § 8.1 for context. Start with `appleboy/ssh-action` (restricted deploy user, ed25519 key, non-22 port, fail2ban). Plan to migrate to Tailscale within 6 months of go-live.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 8.2 Adopt: Health check + rollback after deploy

See `docs/research/ci-options.md` § 8.2 for context. Hit `/healthz` with retries and roll back on failure, keeping a `:previous` tag for quick recovery.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 8.3 Note: OIDC for deploys (informational)

See `docs/research/ci-options.md` § 8.3 for context. GHCR is already OIDC-handled. Self-hosted VPS SSH key remains the auth path — no action needed.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

---

## 9. Repo & workflow hygiene

### 9.1 Adopt: `actionlint` (workflow YAML linter)

See `docs/research/ci-options.md` § 9.1 for context. Add a workflow that type-checks `${{ }}` expressions and validates action inputs.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 9.2 Adopt: Switch to Repository Rulesets, not legacy branch protection

See `docs/research/ci-options.md` § 9.2 for context. Create a ruleset on `main` requiring PRs, all CI gates, up-to-date branches, and blocking force-push. Add yourself to bypass list.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 9.3 Adopt: Documentation checks

See `docs/research/ci-options.md` § 9.3 for context. Add markdownlint with tuned config; run lychee weekly on schedule (not blocking) for link checking.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

### 9.4 Skip: `commitlint`, PR title linting, release tools, `shellcheck`, `yamllint`

See `docs/research/ci-options.md` § 9.4 for context. These are not applicable for this project at this stage.

- [ ] Approved — must
- [ ] Approved — nice
- [ ] Approved — skip
- [ ] Needs discussion

---

## 10. Release automation (defer)

For a prelaunch project with no public releases, all release automation is deferred. Adopt `release-plz` when you ship.

- [ ] Approved — defer
- [ ] Needs discussion

---

## 12. Phased adoption plan

Bundling the "Approved" items into a sequence that prioritizes risk reduction and minimum disruption.

### Stream A — Close the "lefthook isn't CI" gap (highest leverage, lowest risk)

Goal: any check lefthook runs locally also runs in CI.

1. New workflow `.github/workflows/backend-ci.yml`:
   - `cargo +nightly fmt -- --check`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo nextest run --workspace --all-features` + `cargo test --doc --workspace`
   - DB check (migration apply + entity drift) — §2.5
2. New workflow `.github/workflows/frontend-ci.yml`:
   - `bun install --frozen-lockfile`
   - `bun run lint -- --max-warnings=0`
   - `bunx prettier --check .`
   - `bun run typecheck` (in parallel with build)
   - `bun run build`
3. Rewrite `coverage.yml` to use `cargo llvm-cov nextest`.
4. Add `actionlint` workflow.
5. Add `concurrency:` block to all three workflows.

**Outcome:** PR-bypass via `--no-verify` no longer hides anything. Migration drift caught at PR time.

### Stream B — Supply-chain hygiene

1. `dependabot.yml` for cargo / npm / docker / github-actions.
2. Enable Dependabot security updates + GitHub native secret scanning + push protection (UI).
3. SHA-pin all third-party actions in existing workflows (one-time pass via StepSecurity Secure Workflow).
4. Add `cargo-deny` to backend-ci.yml (`advisories bans sources licenses`). Configure `deny.toml`.
5. Add `actions/dependency-review-action` workflow.
6. Enable CodeQL default setup (UI) for JS/TS + Rust.
7. One-time OSSF Scorecard run; fix gaps; disable.
8. Repository Ruleset on `main` with required checks.

**Outcome:** Known supply-chain attack vectors covered. Future audits cleaner.

### Stream C — Wire up frontend testing

1. Install Vitest + `@testing-library/react` + `@testing-library/jest-dom`. Configure `vitest.config.ts` with the existing Vite plugin chain.
2. Write 1–2 component tests as proof of life (e.g., a presentational component already in the tree).
3. Add `bun run test` (Vitest) to `frontend-ci.yml`.
4. Update `codecov.yml` to instrument frontend with separate flag.
5. Scaffold Playwright dir + one smoke test against `vite preview`. Don't wire into required checks yet — let it stabilize.

**Outcome:** Frontend logic added in Milestone Star has somewhere to land tests.

### Stream D — Container build (when ready to deploy)

1. New workflow `.github/workflows/container.yml`:
   - hadolint
   - `docker/setup-buildx` + `metadata-action` + `login-action` + `build-push-action`
   - `aquasecurity/trivy-action` (HIGH+CRITICAL gate, SHA-pinned)
   - Push to GHCR on `main` + tags
2. Weekly cron for `snok/container-retention-policy`.

### Stream E — Deploy automation

1. SSH deploy via `appleboy/ssh-action` to VPS deploy user.
2. `Jtalk/url-health-check-action` post-deploy + rollback step.
3. Diun on the VPS as a base-image-CVE watchdog.

### Stream F — Polish (anytime, low priority)

- `cargo-machete`, `typos`, markdownlint-cli2, lychee, zizmor.
- `cargo doc -D warnings` job.
- E2E + axe accessibility wired into required checks.
- Bundle size monitoring + Lighthouse CI once preview URLs exist.
- `release-plz` once you actually start releasing.

---

## Document history

- 2026-05-05 — Created in `docs/designs/` by splitting from the 2026-05-04 `reviews/design/2026-05-04-ci-research.md` per the triage decision in `docs/designs/archive/2026-05-04-design-doc-restructure.md` §6.5 (archived 2026-05-15). PR #41.
- 2026-05-15 — Updated the path reference in the 2026-05-05 entry above for the design-doc-restructure record (now archived under `designs/archive/`). Companion to PR [#160](https://github.com/brendanbyrne/beerio-kart/pull/160) / Issue [#159](https://github.com/brendanbyrne/beerio-kart/issues/159).
