# .github

GitHub Actions workflows and issue templates.

CI does not reimplement the build. Almost every job provisions the pinned
toolchain with `jdx/mise-action@v4` and then runs a canonical `mise` task, so
those jobs run exactly the commands you can run locally. Toolchain versions live
in [`../mise.toml`](../mise.toml); nothing here pins a compiler.

Three jobs are deliberately outside that pattern and cannot be reproduced by a
`mise` task: the two OSV-Scanner jobs (reusable upstream workflows) and Semver
Checks (raw `cargo semver-checks` against `origin/master`).

## Workflows

| File | Triggers | What it runs |
| --- | --- | --- |
| `workflows/build.yml` | push to `master`; PR opened/synchronize/reopened | The PR gate. Lint, tests on all three surfaces, MSRV, release build, supply chain, publish checks, semver. |
| `workflows/docs.yml` | push to `master`; Mondays 06:00 UTC; manual | Documentation gates, kept out of `build.yml` so a slow rustdoc build never blocks a PR. |
| `workflows/slow-rust-tests.yml` | Sundays 06:00 UTC; manual | `mise run rust-test-slow` — the `#[ignore]`d suite, 120-minute budget. |
| `workflows/release.yml` | successful `Build` run on `master`; manual with `publish: true` | Builds wheels, sdist, and WASM packages, then cuts a GitHub Release. |

### build.yml

`prime-cache` is a single-writer job that populates the mise cache; every other
job reads it with `cache_save: false` to avoid concurrent `actions/cache/save`
races on the shared `mise.toml`-derived key. Jobs then fan out in parallel:

| Job | Command | Notes |
| --- | --- | --- |
| Lint | `mise run pre-commit-run`, then `mise run gen-check` | `SKIP: cargo-deny` — the supply-chain job owns it. Clippy covers lib/bins/tests/examples (`--all-features`), not Criterion benches. |
| Test Rust | `mise run rust-test` | cargo-nextest, lib + integration targets. |
| Rust MSRV (1.90) | `mise run rust-msrv` | `cargo +1.90 check --locked --workspace --all-features --lib --bins --examples`. Production targets only — tests and benches are not MSRV-checked. |
| Rust release build | `mise run rust-build-prod` | Release compile without debug info. |
| Test Python | `mise run python-test-all` | maturin dev build, then the full pytest suite. |
| Test WASM | `mise run wasm-test` | wasm-bindgen tests plus the Node facade suite. |
| Supply-chain Security | `mise run rust-audit` | `cargo deny check` — advisories, licenses, bans. |
| Rust Publish Checks | `mise run rust-publish-checks` | Publish order and first-crate dry run. |
| OSV-Scanner | reusable workflow | PR-diff scan on pull requests, full scan on push. Covers `Cargo.lock`, `uv.lock`, and `finstack-quant-wasm/package-lock.json`. |
| Semver Checks | `cargo semver-checks check-release` | PRs only, gated behind lint and all three test jobs. Checks `finstack-quant-core`, `finstack-quant-valuations`, `finstack-quant-portfolio` against `origin/master`. |

Two details worth knowing before editing:

- The cargo-heavy jobs — Lint, Test Rust, Rust MSRV, Rust release build, Test
  Python, Test WASM, Semver Checks — add 10 GB of swap
  (`pierotofy/set-swap-space`); the workspace has OOM'd linking without it. The
  light jobs (`prime-cache`, Supply-chain Security, Rust Publish Checks) do not.
- The semver job compares against `origin/master`, not against the released
  tag. For a tag baseline locally, use the same command with
  `--baseline-rev <tag>` (current tags already use the `finstack-quant-*`
  crate layout, so no checkout rewrite is required).

### docs.yml

- **Rust Documentation** — `mise run rust-doc`: strict `RUSTDOCFLAGS='-D
  warnings'` build, the public-API input-documentation contract, the
  deprecation-annotation contract, and workspace doctests. Installs nightly
  because `cargo public-api` still needs nightly rustdoc JSON.
- **Python and WASM Documentation** — `mise run python-doc`,
  `mise run wasm-doc`, and `mise run materialization-benchmark-doc-check`.

### release.yml

Fires automatically after a successful `Build` on `master`, or manually with
`publish: true`. Jobs:

- **Wheels** — `PyO3/maturin-action@v1` across linux-x64, linux-arm64,
  macos-arm64, and windows-x64 for CPython 3.12, 3.13, and 3.14. Every wheel
  except linux-arm64 (build-only until a native arm64 runner is selected) is
  installed and smoke-tested with `scripts/smoke_python_wheel.py`.
- **Source dist** — `maturin sdist`.
- **WASM / npm** — syncs `package.json` to the workspace version from
  `Cargo.toml`, builds the `web` and `nodejs` targets, and packs an npm tarball.
- **Publish** — downloads every artifact and creates a GitHub Release. Automatic
  runs are timestamped prereleases (`master-YYYYmmdd-HHMMSS`); a manual run with
  an explicit `tag` input marks the release latest.

Artifacts are attached to the GitHub Release only. This workflow does not push
to PyPI or npm.

## Issue templates

`ISSUE_TEMPLATE/` holds the stock GitHub `bug_report.md`, `feature_request.md`,
and an unfilled `custom.md`. They are not tailored to this project — the bug
template still asks for browser and smartphone details.

## Changing CI

Add the command to `mise.toml` first and make it pass locally, then call the
task from a workflow. Jobs that invoke raw `cargo`/`npm` instead of a `mise`
task cannot be reproduced locally at all.

The local mirrors are partial, so know what they do and do not cover:

- `mise run all-ci` regenerates derived artifacts first, then covers the Lint,
  Test Rust, Test Python, Test WASM, Supply-chain Security, and Rust Publish
  Checks jobs of `build.yml`. It does **not** run Rust MSRV, Rust release
  build, OSV-Scanner, or Semver Checks. GitHub CI still uses `gen-check` as a
  non-mutating drift gate so uncommitted generated files fail the Lint job.
- `mise run all-doc` mirrors `docs.yml`.
- `slow-rust-tests.yml` is `mise run rust-test-slow`; `release.yml` has no local
  equivalent.
