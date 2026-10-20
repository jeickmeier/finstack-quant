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
   ([`.github/workflows/pr.yml`](.github/workflows/pr.yml)) runs the nightly
   check set: lint, generated-artifact drift, the fast Rust, Python, and WASM
   tests, and the supply-chain audit. It reports one status check, `PR gate`,
   which must pass before the pull request can merge. `mise run all-ci` runs
   the same checks locally.
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
Lint job's `gen-check` step fails on any generated file left stale.

### Pull Request Descriptions

Keep pull requests focused. Include:

- What changed.
- Why it changed.
- Which checks were run.
- Any remaining verification gaps.

For financial models, pricing, risk, or accounting behavior, include convention
notes and tests that cover edge cases, invalid inputs, and units.

## Releases

A release is cut from `master` by pushing a `vX.Y.Z` tag.
[`release.yml`](.github/workflows/release.yml) then runs the release gates and
publishes the GitHub Release; see
[`.github/workflows/README.md`](.github/workflows/README.md).

1. Open a release pull request that:
   - sets the version in the workspace `Cargo.toml` and in
     `finstack-quant-wasm/package.json`;
   - adds the `## [X.Y.Z]` section to `CHANGELOG.md`;
   - adds `release_notes/RELEASE_NOTES_X.Y.Z.md`.

   The release workflow's Version metadata gate checks all four.
   `mise run all-ci --release` runs the release gates locally.
2. Once it merges, tag the merge commit on `master` and push the tag:

   ```bash
   git switch master
   git pull
   git tag -a vX.Y.Z -m "vX.Y.Z"
   git push origin vX.Y.Z
   ```

3. A failed gate leaves the tag without a release. Fix forward through a pull
   request, then delete and re-push the tag, or tag the next patch version.

A fix for a released version lands on `master` and ships in the next patch
release. Only when an older release line needs a fix after `master` has moved
on, create `release/X.Y` from that line's latest tag, cherry-pick the fix from
`master`, and tag the patch release on that branch.

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
