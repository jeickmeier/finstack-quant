# Contributing to Finstack Quant

Thanks for your interest in contributing.

## Good First Contributions

- Improve examples and notebooks.
- Add docstrings to Python stubs.
- Add missing test cases for financial conventions.
- Improve error messages at Rust, Python, or WASM boundaries.
- Add small valuation examples.
- Expand glossary and concepts documentation.

## Development Setup

```bash
git clone https://github.com/jeickmeier/finstack-quant.git
cd finstack-quant
mise install
mise run python-sync
```

Run the full local quality gate before opening larger changes:

```bash
mise run all-fmt
mise run all-lint
mise run all-test
```

For narrower changes, use the smallest relevant task first:

| Area | Commands |
|---|---|
| Rust libraries | `mise run rust-build`, `mise run rust-test` |
| Python bindings | `mise run python-build`, `mise run python-test` |
| WASM bindings | `mise run wasm-pkg`, `mise run wasm-test` |
| UI registry | `mise run ui-fmt`, `mise run ui-typecheck`, `mise run ui-test` |
| Examples | `mise run python-examples` |
| Schemas | `mise run rust-check-schemas` |

Run `mise tasks` to see the full task list.

## Project Principles

- Financial logic lives in Rust.
- Python and WASM bindings stay thin: type conversion, wrapper construction,
  error mapping, and module registration only.
- Public APIs require documentation and tests.
- Rust names are canonical. Python names match Rust `snake_case`; WASM names use
  `camelCase`.
- Determinism, convention clarity, and explicit error behavior matter more than
  clever abstractions.

## Binding Changes

When adding or renaming binding surface, update the whole public slice in one
change:

- Rust source, Rust tests, and re-exports.
- PyO3 registration, `__all__`, `.pyi` stubs, and package `__init__.py` exports.
- WASM `#[wasm_bindgen(js_name = ...)]`, TypeScript declarations, facade exports,
  and namespace shims when the surface is part of the WASM subset.
- `finstack-quant-py/parity_contract.toml`.
- Examples, notebooks, and benchmarks that reference the renamed API.

## Documentation

Documentation should describe current behavior, not implementation history.
Prefer concrete inputs, outputs, units, market conventions, errors, and examples.
For Python users, `.pyi` docstrings are the primary IDE-facing API docs.

Useful entry points:

- [`README.md`](README.md)
- [`docs/index.md`](docs/index.md)
- [`docs/REFERENCES.md`](docs/REFERENCES.md)
- [`finstack-quant-py/DOCS_STYLE.md`](finstack-quant-py/DOCS_STYLE.md)
- [`finstack-quant-py/parity_contract.toml`](finstack-quant-py/parity_contract.toml)

## Branches and Pull Requests

`master` is the only long-lived branch and must always be releasable. Every
change reaches it through a pull request, the maintainer's included: a ruleset
blocks direct pushes, force-pushes, and deletion of `master`.

1. Branch from `master`. Prefix the branch name with the change type, then a
   short slug: `feat/`, `fix/`, `perf/`, `refactor/`, `docs/`, `test/`, `ci/`,
   or `chore/` (for example `fix/cds-accrual-on-default`). Automated agents may
   use their own prefix, such as `claude/`.
2. Keep the branch short-lived: days, not weeks. Split large work into a series
   of pull requests that each leave `master` working.
3. Open a pull request into `master`. The PR workflow
   ([`.github/workflows/pr.yml`](.github/workflows/pr.yml)) runs the fast
   check tier: lint, generated-artifact drift, the fast Rust, Python, and WASM
   tests, and the supply-chain audit, in under 15 minutes. It reports one
   status check, `PR gate`, which must pass before the pull request can merge.
   `mise run all-ci` runs the same checks locally. The full tier (coverage,
   docs and doctests, the slow suites) runs nightly on `master`; for a change
   that needs it before merging, run
   `gh workflow run full-checks.yml --ref <branch>`, or
   `mise run all-ci --full` locally (without coverage). A test that needs more
   than 60 s belongs in the slow suite: mark it `#[ignore = "slow: <reason>"]`.
4. Title the pull request as a conventional commit, such as
   `fix(credit): accrue premium on default`. Pull requests are squash-merged,
   so the title becomes the commit subject on `master`.
5. To merge as soon as the checks pass, enable auto-merge with
   `gh pr merge --auto --squash`. The branch is deleted after it merges.

A branch does not need to be up to date with `master` to merge. The nightly run
on `master` catches the rare pair of changes that merge cleanly but fail
together.

Branches worked on in parallel often conflict in generated files: JSON schemas,
WASM contract types, the UI registry, and the pinned UI fixtures. Resolve such a
conflict by taking `master`'s version and regenerating, not by editing the
file. `mise run gen-write` regenerates the schemas, contract types, and UI
registry; [`AGENTS.md`](AGENTS.md) lists the commands for the UI fixtures. The
Generated artifacts job (`mise run gen-check`) fails on any generated file left
stale.

### Pull Request Descriptions

Keep pull requests focused. Include:

- What changed.
- Why it changed.
- Which checks were run.
- Any remaining verification gaps.

For financial models, pricing, risk, or accounting behavior, include convention
notes and tests that cover edge cases, invalid inputs, and units.

## Releases

There are three channels, all GitHub Releases built by the same workflow
([`build-artifacts.yml`](.github/workflows/build-artifacts.yml)); see
[`.github/workflows/README.md`](.github/workflows/README.md):

| Channel | Tag | Version | Built from |
| --- | --- | --- | --- |
| Nightly | `nightly` (moves) | `X.Y.Z-dev.YYYYMMDDHHMM` | `master`, every night it moved |
| Release candidate | `vX.Y.Z-rc.N` | `X.Y.Z-rc.N` | `release/X.Y`, for QA/UAT |
| Release | `vX.Y.Z` | `X.Y.Z` | the candidate QA signed off on |

Python spells the first two `X.Y.Z.devYYYYMMDDHHMM` and `X.Y.ZrcN`. Both
orderings put dev builds below candidates below the release.

### Upstream first

Every change lands on `master` first, through a normal pull request, including
the version bump and release notes. A release branch only receives backports
of changes already on `master`, so it is always a subset of `master`: whatever
is not backported ships in the next release line, and nothing ships that
`master` lacks. The `PR gate` on release branches and the release workflow
both enforce this (`mise run release-check-upstream` runs the check locally).

### Cutting a release

1. **Cut the line.** Create `release/X.Y` from a recent `master` commit, or from
   the previous release tag for a release that carries only a few changes, push
   it, and create the label `backport release/X.Y`:

   ```bash
   git switch -c release/X.Y <master commit or vX.Y'.Z'>
   git push -u origin release/X.Y
   gh label create "backport release/X.Y"
   ```

   Then bump `master` to the next minor version `X.(Y+1).0` (the workspace
   `Cargo.toml` version and every internal `path` dependency requirement, and
   `finstack-quant-wasm/package.json`; `mise run stamp-version -- X.(Y+1).0`
   writes them all, then `cargo metadata --format-version 1 >/dev/null`
   refreshes `Cargo.lock`) in a pull request, so nightly builds order above
   everything on this line.
2. **Write the release notes on `master`.** In a pull request into `master`,
   labeled `backport release/X.Y`, add the `## [X.Y.Z]` section to
   `CHANGELOG.md` and `release_notes/RELEASE_NOTES_X.Y.Z.md`.

   The tag sets the version: the release workflow stamps `X.Y.Z` or
   `X.Y.Z-rc.N` into the manifests before building, so a patch release or
   candidate needs no version commit. Its Version metadata gate checks that the
   committed version is on the tag's `X.Y` line, and, for the final tag, the
   changelog section and notes file. `mise run all-ci --release` runs both
   check tiers (without coverage) and the release gates locally.
3. **Choose the pull requests.** Label each pull request into `master` that
   should ship on this line `backport release/X.Y`, before or after it merges.
   Once it is merged, the backport bot opens a pull request into
   `release/X.Y` that merges itself when `PR gate` passes. On a conflict it
   opens a draft pull request with the conflict committed and comments the
   steps on the original pull request: resolve on that branch, mark it ready,
   and merge.
4. **Tag a candidate** on the release branch:

   ```bash
   git switch release/X.Y
   git pull
   git tag -a vX.Y.Z-rc.N -m "vX.Y.Z-rc.N"
   git push origin vX.Y.Z-rc.N
   ```

   [`release.yml`](.github/workflows/release.yml) runs every release gate and
   publishes a GitHub pre-release. Its notes list the pull requests since the
   previous tag and the `master` commits the candidate leaves out. QA/UAT
   installs from it. For a fix, backport it and tag `rc.N+1`.
5. **Promote.** When UAT signs off, tag the same commit as the release:

   ```bash
   git tag -a vX.Y.Z -m "vX.Y.Z" vX.Y.Z-rc.N^{}
   git push origin vX.Y.Z
   ```

   The release is built from exactly the commit QA tested and is marked
   latest.
6. A failed gate leaves the tag without a release. Fix forward through a pull
   request into `master` and a backport, then delete and re-push the tag, or
   tag the next candidate.

Keep release branches short-lived. After `vX.Y.0` ships, `release/X.Y` takes
only patch fixes for that line, through the same label; the next minor release
cuts a new branch from `master`.

A release can still be tagged straight on `master` when it should carry
everything there: merge the release notes pull request, tag the merge commit
`vX.Y.0` (`master` carries `X.Y.0`), then bump `master` to `X.(Y+1).0`.

### Nightly builds

Every night that `master` moved and its checks pass,
[`nightly.yml`](.github/workflows/nightly.yml) publishes the same artifacts as
a release under the rolling `nightly` pre-release. Install the latest master
build with `gh release download nightly`. It is replaced on the next build and
is not for production use.

## Maintainers and Review

The project has one maintainer. The `master` ruleset requires a pull request
and a passing `PR gate`, but no approving review, because GitHub does not let
authors approve their own pull requests.
[`.github/CODEOWNERS`](.github/CODEOWNERS) assigns every path to the
maintainer.

When a second maintainer joins:

1. Optionally, first transfer the repository to a GitHub organization. Links
   and clones redirect, and an organization adds teams and the merge queue.
2. Add the new maintainer with write access.
3. In the `master` ruleset (Settings → Rules → Rulesets), raise the required
   approvals from 0 to 1. Consider also dismissing stale approvals when new
   commits are pushed and requiring conversation resolution.
4. Split `.github/CODEOWNERS` by area so the right maintainer is requested
   automatically, and optionally require code-owner review in the ruleset.
5. Agree who owns each release.

Branch names, CI, the merge method, and the release process stay the same.
