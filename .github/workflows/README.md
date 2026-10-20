# GitHub Actions

CI workflows and issue templates for finstack-quant.

There are two workflows. `nightly.yml` is the minimal check set and runs every
night: lint, the standard fast tests, and audits, with no doctests or slow
tests. `release.yml` runs when a `vX.Y.Z` tag is pushed. It runs the nightly
set plus the release-only gates (docs, the `--doctest` and `--slow` test
variants, publish checks, semver), then builds and publishes the release. Pull requests
and pushes to `master` trigger nothing; run `mise run all-ci` before merging.

CI does not reimplement the build. Almost every job provisions the pinned
toolchain with `jdx/mise-action@v4` and then runs a canonical `mise` task, so
those jobs run exactly the commands you can run locally. Toolchain versions live
in [`../../mise.toml`](../../mise.toml); nothing here pins a compiler.

## Workflows

| File | Triggers | What it runs |
| --- | --- | --- |
| `nightly.yml` | daily 06:00 UTC; manual; called by `release.yml` | Lint, the fast tests on all three surfaces, supply-chain audit, and the SonarQube scan. |
| `release.yml` | push of a `vX.Y.Z` tag | Version metadata check, the nightly set, docs and doctests, the slow Rust and Python suites, publish checks, and semver. If all pass, it builds wheels, sdist and WASM packages, then creates the GitHub Release. |

### nightly.yml

`prime-cache` runs first as the single writer of the mise cache; every other
job reads it with `cache_save: false` to avoid concurrent `actions/cache/save`
races on the shared `mise.toml`-derived key. Supply-chain Security runs next
and gates the rest. Once it passes, Lint and the three test jobs fan out in
parallel, and SonarQube runs after all three test jobs:

| Job | Command | Notes |
| --- | --- | --- |
| Supply-chain Security | `mise run all-audit` | `cargo deny check` (advisories, licenses, bans), then OSV-Scanner over `Cargo.lock`, `uv.lock` and `finstack-quant-wasm/package-lock.json`, filtered by `osv-scanner.toml`. |
| Lint | `mise run pre-commit-run`, then `mise run gen-check` | `SKIP: cargo-deny`, which the supply-chain job owns. Clippy covers lib/bins/tests/examples (`--all-features`), not Criterion benches. |
| Test Rust | `mise run rust-test --coverage` | The fast nextest lib and integration targets under `cargo llvm-cov` (no doctests, no `#[ignore]`d tests); uploads `coverage/rust/lcov.info`. |
| Test Python | `mise run python-test --coverage` | maturin dev build, then the fast pytest suite (`-m 'not slow and not doctest'`) with coverage; uploads `coverage/python/coverage.xml`. |
| Test WASM | `python3 scripts/check_wasm_boundary.py`, then `mise run wasm-test` | Serializer and error-path check, then wasm-bindgen tests plus the Node facade suite (the documented code samples run under `--doctest`). |
| SonarQube | `SonarSource/sonarqube-scan-action` | `master` only, so it is skipped inside a release run. Scan only: it imports the two coverage artifacts and runs no tests and no clippy (`sonar.rust.clippy.enabled=false`; Lint owns clippy). |

Three details worth knowing before editing:

- The cargo-heavy jobs add 10 GB of swap (`pierotofy/set-swap-space`); the
  workspace has OOM'd linking without it.
- Every `Swatinem/rust-cache` step follows `rustup toolchain uninstall
  stable`. rust-cache keys on the version of every installed toolchain, and the
  runner image's preinstalled `stable` changes between images; without the step
  the key flips and the job builds cold. The builds use the toolchain mise pins.
- `nightly.yml` has no workflow-level `concurrency`. As a called workflow it
  would share the release run's group and cancel or deadlock it.

### release.yml

Gates. Everything except `version` starts after `version` passes, in parallel:

| Job | Command | Notes |
| --- | --- | --- |
| Version metadata | inline check | The tag equals the `Cargo.toml` workspace version and the `finstack-quant-wasm/package.json` version; `CHANGELOG.md` has a `## [X.Y.Z]` section; `release_notes/RELEASE_NOTES_X.Y.Z.md` exists. |
| Nightly checks | `uses: ./.github/workflows/nightly.yml` | The whole nightly set (SonarQube skipped). |
| Documentation | `mise run all-doc`, then `rust-test`, `python-test` and `wasm-test` with `--doctest` | Strict rustdoc, the public-API input-doc and deprecation contracts, the Python and WASM documentation contracts, and the benchmark documents; then the Rust doctests, the Python stub and module doctests, and the WASM documented code samples. Installs nightly for `cargo public-api`. |
| Test ignored Rust suite | `mise run rust-test --slow` | The `#[ignore]`d suite. |
| Test slow Python suite | `mise run python-test --slow` | Golden pricing comparisons and Monte Carlo. |
| Rust Publish Checks | `mise run rust-publish-checks` | Publish order and first-crate dry run. |
| Semver Checks | `cargo semver-checks check-release` | `finstack-quant-core`, `finstack-quant-valuations`, `finstack-quant-portfolio` against the previous `vX.Y.Z` tag reachable from the release commit. |

The release-only jobs build cold. Nothing on `master` saves a Rust cache for
them, and a cache saved on a tag is visible only to that tag.

Artifacts, after every gate passes:

- **Wheels**: `PyO3/maturin-action@v1` across linux-x64, linux-arm64,
  macos-arm64, and windows-x64 for CPython 3.12, 3.13, and 3.14. Every wheel
  except linux-arm64 (build-only until a native arm64 runner is selected) is
  installed and smoke-tested with `scripts/smoke_python_wheel.py`.
- **Source dist**: `maturin sdist`.
- **WASM / npm**: `mise run wasm-pkg` (the size-optimized web package and the
  Node package), then the npm tarball plus one archive per target.

**Publish** runs after every artifact builds. It creates the GitHub Release for
the tag, marked latest, with the release notes file as its body. Artifacts are
attached to the GitHub Release only; nothing is pushed to crates.io, PyPI, or
npm.

A failed gate leaves the tag without a release. Fix forward, then delete and
re-push the tag, or tag the next patch version.

## Issue templates

[`../ISSUE_TEMPLATE/`](../ISSUE_TEMPLATE/) holds the stock GitHub
`bug_report.md` and `feature_request.md`. They are not tailored to this
project; the bug template still asks for browser and smartphone details.

## Changing CI

Add the command to `mise.toml` first and make it pass locally, then call the
task from a workflow. Jobs that invoke raw `cargo`/`npm` instead of a `mise`
task cannot be reproduced locally at all.

Local equivalents:

- `mise run all-ci` regenerates derived artifacts first, then runs the nightly
  Lint, Test, and Supply-chain checks (without coverage). GitHub CI uses
  `gen-check` as a non-mutating drift gate so uncommitted generated files fail
  the Lint job.
- The release gates are `mise run all-doc`; `rust-test`, `python-test` and
  `wasm-test` with `--doctest`; `rust-test --slow` and `python-test --slow`;
  `mise run rust-publish-checks`; and `cargo semver-checks check-release
  --baseline-rev <previous tag>`.
- Benchmarks are not run in CI. Use `mise run rust-bench` or
  `mise run rust-bench-crate`, and `scripts/materialization_bench.sh` for the
  materialization record.
