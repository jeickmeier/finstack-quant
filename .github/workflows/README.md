# GitHub Actions

CI workflows and issue templates for finstack-quant.

`checks.yml` is the standard check set: lint, the standard fast tests, and
audits, with no doctests or slow tests. The other workflows call it:

- `pr.yml` runs it on every pull request into `master` or a `release/X.Y`
  branch; its `PR gate` check must pass before the pull request can merge.
- `nightly.yml` runs it every night and, when `master` moved, builds the
  release artifacts and publishes them as the rolling `nightly` pre-release.
- `release.yml` runs when a `vX.Y.Z` or `vX.Y.Z-rc.N` tag is pushed. It adds the
  release-only gates (upstream check, docs, the `--doctest` and `--slow` test
  variants, publish checks, semver), then builds and publishes the release or
  release candidate.

`build-artifacts.yml` builds the wheels, sdist, and WASM packages for both
`nightly.yml` and `release.yml`. `backport.yml` cherry-picks labeled pull
requests from `master` onto release branches. Branching, pull request, and
release practice is in [`../../CONTRIBUTING.md`](../../CONTRIBUTING.md).

CI does not reimplement the build. Almost every job provisions the pinned
toolchain with `jdx/mise-action@v4` and then runs a canonical `mise` task, so
those jobs run exactly the commands you can run locally. Toolchain versions live
in [`../../mise.toml`](../../mise.toml); nothing here pins a compiler.

## Workflows

| File | Triggers | What it runs |
| --- | --- | --- |
| `checks.yml` | manual; called by `pr.yml`, `nightly.yml` and `release.yml` | Lint, the fast tests on all three surfaces, supply-chain audit, and the SonarQube scan. |
| `pr.yml` | pull request into `master` or `release/**` | The check set (SonarQube skipped), the upstream check for release branches, then the `PR gate` check the rulesets require. |
| `nightly.yml` | daily 06:00 UTC; manual | The check set, then, if `master` moved, `build-artifacts.yml` at `X.Y.Z-dev.YYYYMMDDHHMM` and the rolling `nightly` pre-release. |
| `release.yml` | push of a `vX.Y.Z` or `vX.Y.Z-rc.N` tag | Version metadata check, the check set, the upstream check, docs and doctests, the slow Rust and Python suites, publish checks, and semver. If all pass, `build-artifacts.yml`, then the GitHub Release (a pre-release for `-rc.N`). |
| `build-artifacts.yml` | called by `nightly.yml` and `release.yml` | Wheels, sdist and WASM packages, stamped with the caller's version. |
| `backport.yml` | pull request into `master` merged or labeled | A cherry-pick pull request into each `release/X.Y` named by a `backport release/X.Y` label. |

### checks.yml

`prime-cache` runs first as the single writer of the mise cache; every other
job reads it with `cache_save: false` to avoid concurrent `actions/cache/save`
races on the shared `mise.toml`-derived key. Supply-chain Security runs next
and gates the rest. Once it passes, Lint and the three test jobs fan out in
parallel, and SonarQube runs after all three test jobs:

| Job | Command | Notes |
| --- | --- | --- |
| Supply-chain Security | `mise run all-audit` | `cargo deny check` (advisories, licenses, bans), then OSV-Scanner over `Cargo.lock`, `uv.lock` and `finstack-quant-wasm/package-lock.json`, filtered by `osv-scanner.toml`. |
| Lint | `mise run pre-commit-run`, then `mise run gen-check` | `SKIP: cargo-deny,osv-scanner`, which the supply-chain job owns. Clippy covers lib/bins/tests/examples (`--all-features`), not Criterion benches. |
| Test Rust | `mise run rust-test --coverage` | The fast nextest lib and integration targets under `cargo llvm-cov` (no doctests, no `#[ignore]`d tests); writes a Job Summary from nextest's `junit.xml` and `coverage/rust/lcov.info`, and uploads `coverage/rust/lcov.info`. |
| Test Python | `mise run python-test --coverage` | maturin dev build, then the fast pytest suite (`-m 'not slow and not doctest'`) with coverage; writes a Job Summary from `coverage/python/{junit.xml,coverage.xml}` and uploads `coverage/python/coverage.xml`. |
| Test WASM | `mise run wasm-test` | wasm-bindgen tests plus the Node facade suite and JSDoc sync check (the documented code samples run under `--doctest`); writes a Job Summary of the test results. |
| SonarQube | `SonarSource/sonarqube-scan-action` | `master` only, so it is skipped inside a release run. Scan only: it imports the two coverage artifacts, runs no tests and no clippy (`sonar.rust.clippy.enabled=false`; Lint owns clippy), and writes the Quality Gate and project measures to the Job Summary. |

Three details worth knowing before editing:

- The cargo-heavy jobs add 10 GB of swap (`pierotofy/set-swap-space`); the
  workspace has OOM'd linking without it.
- Every `Swatinem/rust-cache` step follows `rustup toolchain uninstall
  stable`. rust-cache keys on the version of every installed toolchain, and the
  runner image's preinstalled `stable` changes between images; without the step
  the key flips and the job builds cold. The builds use the toolchain mise pins.
- `checks.yml` has no workflow-level `concurrency`. As a called workflow it
  would share the caller's group and cancel or deadlock the caller's run.

### pr.yml

`pr.yml` calls `checks.yml` unchanged, coverage included, so every job builds
exactly what the `master` run built and restores the cache that run saved. Pull
request runs never save a cache of their own. Do not add a pull-request-only
build variant: `cargo llvm-cov` builds into `target/llvm-cov-target` with
`-C instrument-coverage`, so an uninstrumented Test Rust on pull requests would
find nothing usable in `master`'s cache and compile every dependency cold. The
SonarQube job is `master`-only and skips.

A new push to a pull request cancels that pull request's run in progress;
`pr.yml` owns the `concurrency` group because `checks.yml` cannot.

Pull requests into `release/**` also run **Upstream check**
(`scripts/check_release_upstream.py check`): every commit the pull request adds
must have a counterpart on `master` — the same patch, a
`(cherry picked from commit <sha>)` trailer naming a `master` commit, or a
`(#N)` reference to a pull request squash-merged on `master`. A change made only
on a release branch fails. Locally: `mise run release-check-upstream`.

The `PR gate` job is the only status check the rulesets require, so jobs in
`checks.yml` can be added or renamed without editing the rulesets. It runs
unless the run was cancelled and fails unless the called checks passed and the
upstream check passed or was skipped (pull requests into `master`). A required
check that is skipped counts as passing, so do not give it a condition that a
failed check can skip.

The `master` ruleset also requires a pull request with linear history (squash
merges only) and blocks force-pushes and deletion. A `release/*` ruleset
mirrors it. Neither requires an approving review while the project has one
maintainer; `CONTRIBUTING.md` lists the steps for adding reviewers.

### nightly.yml

Runs at 06:00 UTC and on manual dispatch, on `master` only:

1. **Checks**: `checks.yml`, every night, including when `master` did not move
   (the supply-chain audit can fail on new advisories alone).
2. **Plan nightly build**: compares `master` with the commit the `nightly` tag
   points at. A scheduled run skips the build when they match; a manual
   dispatch always builds. The version is the `Cargo.toml` workspace version
   plus `-dev.` and the commit time in UTC as `YYYYMMDDHHMM`, e.g.
   `0.12.0-dev.202610210600` (Python `0.12.0.dev202610210600`). `master`
   carries the next unreleased version, so a nightly orders below that
   version's release candidates and release.
3. **Build**: `build-artifacts.yml` at that version.
4. **Publish nightly**: deletes the `nightly` release and tag, then recreates
   them at the built commit: a pre-release, never marked latest, whose notes
   list the commits since the previous nightly. It is recreated rather than
   edited because asset names carry the version.

A failed check or build leaves the previous nightly in place. If publishing
fails after the delete, the next run rebuilds it, since `master` no longer
matches a `nightly` tag. Download the latest with
`gh release download nightly`.

### build-artifacts.yml

Called with a `version` input by `nightly.yml` and `release.yml`. Each job
first runs `scripts/stamp_version.py <version>` in its own checkout: it writes
the version into the workspace `Cargo.toml`, every intra-workspace path
dependency requirement, and `finstack-quant-wasm/package.json` and its
lockfile. Nothing is committed; when the version already matches, nothing
changes. maturin turns `-rc.N` into `rcN` and `-dev.N` into
`.devN` for the wheels. `mise run stamp-version -- <version>` does the same
locally; it is also how `master` is bumped after a release branch is cut.

- **Wheels**: `PyO3/maturin-action@v1` across linux-x64, linux-arm64,
  macos-arm64, and windows-x64 for CPython 3.12, 3.13, and 3.14. Every wheel
  except linux-arm64 (build-only until a native arm64 runner is selected) is
  installed and smoke-tested with `scripts/smoke_python_wheel.py`.
- **Source dist**: `maturin sdist`.
- **WASM / npm**: `mise run wasm-pkg` (the size-optimized web package and the
  Node package), then the npm tarball plus one archive per target.

The `dist-*` workflow artifacts are kept 7 days; the GitHub Release is the
durable copy.

### backport.yml

Adding the label `backport release/X.Y` to a pull request into `master`, before
or after it merges, makes `korthout/backport-action` open a pull request into
`release/X.Y` once it is merged. The pull request cherry-picks the squash commit
with `git cherry-pick -x` and has squash auto-merge enabled, so it merges itself
when `PR gate` passes. On a conflict it opens a draft pull request with the
conflict committed and comments the resolution steps on the original pull
request.

The token comes from a GitHub App, because pull requests opened with the
default `GITHUB_TOKEN` do not trigger workflows and `PR gate` would never run.
The App needs Contents, Pull requests and Workflows read/write on this
repository; its client ID is the `BACKPORT_APP_CLIENT_ID` Actions variable and
its private key the `BACKPORT_APP_PRIVATE_KEY` secret. The workflow runs on
`pull_request_target` from `master` and never checks out pull-request code.

### release.yml

Two tag kinds: `vX.Y.Z` publishes a release; `vX.Y.Z-rc.N` publishes a release
candidate for QA/UAT, usually from a `release/X.Y` branch.

Gates. Everything except `version` starts after `version` passes, in parallel:

| Job | Command | Notes |
| --- | --- | --- |
| Version metadata | inline check | The `Cargo.toml` workspace version and the `finstack-quant-wasm/package.json` version are on the tag's `X.Y` line; the tag itself sets the built version. For `vX.Y.Z` only: `CHANGELOG.md` has a `## [X.Y.Z]` section and `release_notes/RELEASE_NOTES_X.Y.Z.md` exists. |
| Checks | `uses: ./.github/workflows/checks.yml` | The whole check set (SonarQube skipped). |
| Upstream check | `scripts/check_release_upstream.py check origin/master <tag commit>` | The tagged commit carries nothing `master` lacks. A tag on `master` passes trivially. |
| Documentation | `mise run all-doc`, then `rust-test`, `python-test` and `wasm-test` with `--doctest` | Strict rustdoc, the public-API input-doc and deprecation contracts, the Python and WASM documentation contracts, and the benchmark documents; then the Rust doctests, the Python stub and module doctests, and the WASM documented code samples. Installs nightly for `cargo public-api`. |
| Test ignored Rust suite | `mise run rust-test --slow` | The `#[ignore]`d suite. |
| Test slow Python suite | `mise run python-test --slow` | Golden pricing comparisons and Monte Carlo. |
| Rust Publish Checks | `mise run rust-publish-checks` | Publish order and first-crate dry run. |
| Semver Checks | `cargo semver-checks check-release` | `finstack-quant-core`, `finstack-quant-valuations`, `finstack-quant-portfolio`, with the tag's `X.Y.Z` stamped in, against the previous `vX.Y.Z` tag reachable from the release commit (release candidates excluded). |

The release-only jobs build cold. Nothing on `master` saves a Rust cache for
them, and a cache saved on a tag is visible only to that tag.

**Build**, after every gate passes: `build-artifacts.yml` at the tag's version,
so a candidate's wheels are `X.Y.ZrcN` and its npm package `X.Y.Z-rc.N`.

**Publish** runs after every artifact builds. For `vX.Y.Z` it creates the
GitHub Release marked latest, with the release notes file as its body. For
`vX.Y.Z-rc.N` it creates a pre-release, never marked latest, whose body lists
the `master` commits the candidate leaves out
(`scripts/check_release_upstream.py missing`), followed by GitHub's generated
list of pull requests since the previous tag. Artifacts are attached to the
GitHub Release only; nothing is pushed to crates.io, PyPI, or npm.

A failed gate leaves the tag without a release. Fix forward, then delete and
re-push the tag, or tag the next patch version or candidate.

## Issue templates

[`../ISSUE_TEMPLATE/`](../ISSUE_TEMPLATE/) holds the stock GitHub
`bug_report.md` and `feature_request.md`. They are not tailored to this
project; the bug template still asks for browser and smartphone details.

## Changing CI

Add the command to `mise.toml` first and make it pass locally, then call the
task from a workflow. Jobs that invoke raw `cargo`/`npm` instead of a `mise`
task cannot be reproduced locally at all.

Local equivalents:

- `mise run all-ci` regenerates derived artifacts first, then runs the
  `checks.yml` Lint, Test, and Supply-chain checks (without coverage). GitHub CI uses
  `gen-check` as a non-mutating drift gate so uncommitted generated files fail
  the Lint job.
- `mise run all-ci --release` adds the release gates: `all-doc`; `rust-test`,
  `python-test` and `wasm-test` with `--doctest`; `rust-test --slow` and
  `python-test --slow`; `rust-publish-checks`; and `cargo semver-checks
  check-release` against the previous release tag.
- Benchmarks are not run in CI. Use `mise run rust-bench` or
  `mise run rust-bench-crate`, and `scripts/materialization_bench.sh` for the
  materialization record.
