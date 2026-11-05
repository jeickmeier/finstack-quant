# scripts

Repository maintenance and verification tooling. Almost everything here is a
gate: a checker that fails a `mise` task, a pre-commit hook, or a CI job when
the workspace drifts from a contract that the Rust compiler cannot enforce on
its own — documentation completeness, generated-artifact freshness, wire-format
stability, benchmark regressions, publish ordering.

These are Python because they inspect *all three* language surfaces (Rust,
PyO3, wasm-bindgen/TypeScript) and their generated artifacts, which no single
toolchain sees at once.

This file is the map of every gate in the repository: the convention it
enforces, the task that runs it, and its tier.

| Tier | Runs in | Locally |
| --- | --- | --- |
| PR | `.github/workflows/checks.yml` on every pull request | `mise run all` |
| Nightly | `.github/workflows/full-checks.yml` | `mise run all --full` |
| Release | `.github/workflows/release.yml` | `mise run all --release` |
| Local | by hand, when the area changes | the task named in the row |

`mise run all` runs one stage per `checks.yml` job (`pre-commit-run`,
`gen-check`, `rust-test`, `python-test`, `wasm-test`, `ui-check`,
`all-audit`); `scripts/tests/test_gate_parity.py` keeps the two equal.
Generated files are refreshed by one command, `mise run gen-write`.

## Running them

`scripts/` is an importable package (`scripts/__init__.py`), so the multi-module
tools run as modules from the repository root:

```bash
uv run python -m scripts.serde_audit --check
uv run python -m scripts.golden.quantlib.generate --family pricing --product all
```

Single-file scripts run by path:

```bash
uv run --no-sync python scripts/check_loc.py --full
uv run --no-sync python scripts/generation_digest.py --verbose
```

Prefer the `mise` task that owns a script (listed below) — the tasks pass the
exact arguments CI uses. Checkers that touch no compiled extension are invoked
with `uv run --no-sync` so a plain `uv run` does not trigger a ~20 minute PyO3
rebuild; keep that flag when adding static checkers to a task.

## Documentation contract checkers

`AGENTS.md` requires every public callable with caller-supplied inputs to
document each input. These enforce that on each host surface.

| Script | Enforces | Task (tier) |
| --- | --- | --- |
| `check_methodology_coverage.py` | Exact source ownership and record linkage for the [audited crates](../docs/decisions/financial/README.md). `--verify-source` checks audited hashes; `--write` regenerates the readable source map. This is documentation coverage, not numerical certification. | manual: `uv run --no-sync python scripts/check_methodology_coverage.py --verify-source` |
| `check_public_api_input_docs.py` | Rust `# Arguments` coverage for every publicly reachable function, associated function, trait method, and constructor. Builds rustdoc JSON via `cargo public-api`, which needs the nightly toolchain `mise.toml` provisions. | `rust-doc` (nightly) |
| `check_python_api_input_docs.py` | Substantive summaries plus argument/return/raises/example sections on public classes and callables in `.pyi` stubs and pure-Python modules. NumPy- and Google-style sections both parse. | `python-lint` (PR, through pre-commit) |
| `check_pyo3_doc_placeholders.py` | Rejects placeholder prose left in public PyO3 runtime doc comments under `finstack-quant-py/src/bindings/`. | `python-lint` (PR, through pre-commit) |
| `check_deprecated_annotations.py` | Legacy annotation-format checks: every `#[deprecated]` carries `since = "X.Y.Z"`, a replacement or retention rationale, and a planned removal release. Bare annotations fail. These checks do not implement ADR 0004's development replacement policy; [reconciliation is pending](../docs/decisions/FOLLOW_UPS.md#reconciliation-items). | `rust-doc` (nightly) |
| `run_python_stub_doctests.py` | Extracts `>>>` examples from `.pyi` and module docstrings and executes them against the compiled `finstack_quant` package, since stubs are not importable. | `python-test --doctest` (nightly), via `finstack-quant-py/tests/test_stub_doctests.py` |

## Generated artifacts and wire contracts

Rust serde types are the source of truth; JSON Schemas and TypeScript
declarations are deterministic publication artifacts. These prove the artifacts
match the types and that regeneration is idempotent. `mise run gen-write`
regenerates every one of them; `mise run gen-check` fails when one is stale.

| Script | Purpose | Task (tier) |
| --- | --- | --- |
| `serde_audit/` | Package: audits public Rust contract types for effective `Serialize`/`Deserialize`/`JsonSchema` coverage. A conservative module-aware Rust lexer (`lexer.py`, `resolution.py`, `scanner.py`) resolves modules, re-exports, aliases and `cfg_attr` derives; `registries.py` holds the maintained contract set and reviewed exceptions; `report.py` is the CLI (`--check` fails on missing capabilities or stale exceptions, `--report` lists everything). | `gen-check` (PR) |
| `check_schema_residue.py` | Rejects obsolete or hand-maintained JSON/serde machinery: hand-written schema files, asymmetric optional wire adapters, non-canonical serde enum naming. | `gen-check` (PR) |
| `check_schema_generation.py` | Runs the built `gen_*` binaries directly (no `cargo run`). By default it checks that the registries own exactly the generated files on disk (missing artifacts, orphaned schemas or instrument fixtures), validates the `$ref` graph across registries, and regenerates the tree twice into temporary directories, comparing the two and the checked-in files byte-for-byte. `--write` regenerates every registry in place. `--bin-dir` overrides the default `target/debug`. | `gen-check` (PR); `--write` in `gen-write` |
| `check_generated_instrument_fixtures.py` | Validates canonical generated instrument fixtures against `finstack-quant/valuations/tests/instruments/coverage_manifest.toml`. | `gen-check` (PR) |
| `sync_generated_ts_index.py` | Writes (or with `--check`, verifies) the barrels for the schema-generated TypeScript types in `finstack-quant-wasm/types/generated/`: one `<crate>/index.ts` per crate and a root `index.ts` that re-exports each crate as a namespace. The per-crate modules themselves come from `finstack-quant-wasm/scripts/generate-contract-types.mjs`. | `gen-check` (PR); write in `gen-write` |
| `generation_digest.py` | Computes a stable SHA-256 over the generated artifacts named in the manifest. `--check-manifest` verifies that the file list still matches what is on disk; `gen-check` takes the digest before and after the verification pass and fails if it moved, so a "check" that quietly rewrote the workspace is caught. | `gen-check` (PR); `--write-manifest` in `gen-write` |

`generated-artifacts.txt` is the manifest of tracked generated files.
`gen-write` rewrites it last (`generation_digest.py --write-manifest`), so a
new generated file is picked up without a separate step. The per-crate JSON
schema registries in `finstack-quant/*/schemas/` are not in it — their
reproducibility is proved by `check_schema_generation.py` instead.

These tests also pin generated files; `gen-write` refreshes each one:

| Test | Pins | Task (tier) |
| --- | --- | --- |
| `finstack-quant/valuations/tests/registry_snapshot.rs` | `tests/data/registry_snapshot.json`: the models and metrics every instrument type registers | `rust-test` (PR) |
| `finstack-quant-wasm/tests/facade/surface.test.mjs` | `finstack-quant-wasm/facade-surface.json`: the published facade's runtime surface, checked against the declaration | `wasm-test` (PR) |
| `finstack-quant-ui/scripts/run-fixtures.mjs --check` | the UI native fixtures (`tests/**/cases.json` and friends) | `ui-check` (PR) |
| `finstack-quant-ui/scripts/run-generators.mjs --check` | `finstack-quant-ui/src/generated/`, the registry manifests, the theme and provenance | `ui-check` (PR) |

## WASM and UI checkers

| Script | Enforces | Task (tier) |
| --- | --- | --- |
| `check_wasm_boundary.py` | WASM bindings serialize only through `crate::utils::to_js_value` and build errors only through `to_js_err` (no raw `serde_wasm_bindgen::to_value`, no string errors). | `wasm-lint` (PR, through pre-commit) |
| `finstack-quant-wasm/scripts/check-exports-boundary.mjs` | The facade (`exports/**`) only re-exports: no arithmetic, and every function carries a comment saying what wasm-bindgen cannot express. | `wasm-lint` (PR, through pre-commit) |
| `finstack-quant-wasm/scripts/check-typescript-docs.mjs` | JSDoc completeness of the published declarations (`pkg/finstack_quant_wasm.d.ts`, from the Rustdoc of `src/api/**`, and the generated `index.d.ts`): summaries, every camelCase `@param`, `@returns`, examples on classes and namespaces. | `wasm-doc` (nightly) |
| `finstack-quant-wasm/scripts/gen-facade-dts.mjs --check` | The committed `index.d.ts` equals the facade declarations generated from `exports/**` over the built package declarations. | `wasm-test` (PR) |
| `finstack-quant-wasm/tests/facade/surface.test.mjs` | The runtime facade equals `facade-surface.json`, and `index.d.ts` declares exactly that surface. | `wasm-test` (PR) |
| `finstack-quant-wasm/tests/facade/typed_declarations.test.mjs` | No `any` in the published declarations: every `JsValue` parameter and return carries an `unchecked_*_type` attribute. | `wasm-test` (PR) |
| `finstack-quant-wasm/tests/facade/return_shapes.test.mjs` | Declared return shapes: Json/Text suffixes, structured results, `Float64Array` vectors; mirror of `finstack-quant-py/tests/parity/test_return_shapes.py`. | `wasm-test` (PR) |
| `finstack-quant-wasm/tests/facade/benchmark_names.test.mjs` | `benchmarks/bench.mjs` calls only names the package exports. | `wasm-test` (PR) |
| `finstack-quant-wasm` `npm run test:dts` | Consumer programs compile against the declarations under ES2020, ESNext disposable and NodeNext, and with `skipLibCheck: false`. | `wasm-test` (PR) |
| `finstack-quant-wasm/scripts/check-typescript-examples.mjs` | Every documented TypeScript/JavaScript sample type-checks. | `wasm-doc` (nightly) |
| `finstack-quant-ui/scripts/check-tokens.mjs` | Colours, fonts and spacing come only from the theme tokens. | `ui-check` (PR) |
| `finstack-quant-ui/scripts/check-registry.mjs` | Registry closures resolve, and domain components never import `@base-ui/*`. | `ui-check` (PR) |
| `finstack-quant-ui/scripts/check-no-arithmetic.mjs` | No arithmetic in `registry/**` and `src/**` without a `// finstack-arith:` reason. | `ui-check` (PR) |
| `finstack-quant-ui/scripts/check-worker-compose.mjs` | Worker services make one native call; no decision on a native result in JavaScript. | `ui-check` (PR) |
| `finstack-quant-ui/scripts/coverage.mjs --check` | `finstack-quant-ui/facade-coverage.json` records every facade member the worker uses. | `ui-check` (PR) |
| `docs-site/gen/check_{coverage,curriculum,links}.py` | Lesson coverage, curriculum dependencies and evidence, and links in the static export. | `docs-site-build` (local) |

The WASM-side Rust tests `tests/boundary_signatures.rs` (exports take `JsValue`
arguments) and `tests/return_shapes.rs` (no serializer bypass, no JSON parsing
in the facade) run under `rust-test` (PR).

## Benchmarks and regression gates

These implement the portfolio-materialization acceptance record described in
[`../benchmarks/MATERIALIZATION_BENCHMARKS.md`](../benchmarks/MATERIALIZATION_BENCHMARKS.md).
The checked-in baselines live in `../benchmarks/materialization/`. They run by
hand (local tier); `all-doc` runs `scripts/materialization_bench.sh doc-check`
in the nightly tier.

| Script | Purpose | Driven by |
| --- | --- | --- |
| `check_criterion_regressions.py` | Fails when Criterion median estimates regress beyond `--threshold` (0.10), comparing the exact expected materialization paths. | `scripts/materialization_bench.sh rust-compare` |
| `prepare_materialization_criterion_run.py` | Writes fresh run provenance (tree revision, combined fixture digest) before a comparison run, so a stale or wrong-revision measurement is rejected rather than compared. | `scripts/materialization_bench.sh rust-compare` / `python-compare` |
| `materialization_baseline_replace_flag.py` | Validates the `FQ_REPLACE_MATERIALIZATION_BASELINE` opt-in and emits the replace flag. Baselines are immutable without it. | `scripts/materialization_bench.sh rust-baseline` / `python-baseline` |
| `manage_materialization_rust_baseline.py` | `guard` / `establish` / `seal` / `verify` subcommands for the Rust baseline JSON. | `scripts/materialization_bench.sh rust-baseline` / `record` |
| `check_python_materialization_regressions.py` | The Python-side twin: `guard`, `prepare`, `establish`, `seal`, `compare`, `verify`. Rejects wrong revisions, fixture digests, case sets, and stale measurements before applying the median gate. | `scripts/materialization_bench.sh python-baseline` / `python-compare` |
| `collect_materialization_results.py` | Combines raw Rust/Python/WASM samples into the tracked `materialization-benchmark-results.json` record: percentiles, independent bootstrap intervals, phase counters, environment, and the exact commands used. | `scripts/materialization_bench.sh record` |
| `sync_materialization_benchmark_digest.py` | Writes (`--write`) or verifies (`--check`) the record SHA-256 quoted inside the Markdown document. | `scripts/materialization_bench.sh doc-check` / `record` |
| `check_materialization_benchmark_docs.py` | Verifies the Markdown toolchain lines, baseline artifact paths, identities, and hashes against the machine artifact — without running any measurement. | `scripts/materialization_bench.sh doc-check` |

## Golden fixtures

`golden/quantlib/` generates native QuantLib reference fixtures. Two families
share the package and are deliberately not merged — different schemas, different
scenarios:

- **pricing** — `finstack_quant.golden/1` fixtures under
  `finstack-quant/valuations/tests/golden/data/pricing/quantlib/`.
- **attribution** — T0/T1 plus expected-attribution JSON under
  `finstack-quant/valuations/tests/data/quantlib_parity/`.

Product builders are split by asset class: `bonds.py`, `credit.py`,
`deposits.py`, `fx.py`, `fx_exotics.py`, `options.py`, `rate_options.py`,
`rates.py`, `attribution.py`, with shared determinism helpers in `common.py`
(fixed valuation/capture dates, QuantLib >= 1.41 floor). `generate.py` is the
CLI; `--check` reports generator drift instead of writing.

```bash
uv run python -m scripts.golden.quantlib.generate --family pricing --product all           # rewrite pricing fixtures
uv run python -m scripts.golden.quantlib.generate --family pricing --product all --check   # drift check only
uv run python -m scripts.golden.quantlib.generate --family attribution --product all
uv run python -m scripts.golden.quantlib.generate --family attribution --product all --check
```

QuantLib (`quantlib>=1.41`, Python `dev` dependency group — run
`mise run python-sync` first) is needed only to *generate* fixtures. The
generated JSON is checked in, so the Rust and Python golden suites consume it
without a QuantLib install.

## Release and packaging

| Script | Purpose | Task (tier) |
| --- | --- | --- |
| `cargo_publish_checks.py` | Derives the crate publish order from internal dependencies, validates internal dependency versions, and dry-runs the first crate. | `rust-publish-checks` (release) |
| `smoke_python_wheel.py` | Imports the installed `finstack_quant` wheel, walks every public subpackage, and exercises `Currency`/`Money`, `models.bs_price`, and `EuropeanPricer.price_gbm`. Runs against the built wheel on every release platform except linux-arm64. | `.github/workflows/build-artifacts.yml`; `finstack-quant-py/tests/test_smoke_python_wheel.py` (PR) |
| `stamp_version.py` | Writes a SemVer version into the workspace `Cargo.toml`, every intra-workspace path dependency requirement, and the WASM `package.json` and lockfile. CI stamps nightly (`X.Y.Z-dev.N`), candidate, and release versions before building; maintainers use it to bump `master`. | `mise run stamp-version`, `.github/workflows/build-artifacts.yml`, `.github/workflows/release.yml` |
| `check_release_upstream.py` | `check`: fails when a release-branch commit has no counterpart on `master` (same patch, `cherry-pick -x` trailer, or squash-merged `(#N)`). `missing`: lists the `master` commits a release candidate leaves out. | `release-check-upstream`; `.github/workflows/pr.yml` (pull requests into `release/**`), `.github/workflows/release.yml` |

## Notebook examples

| Script | Purpose | Driven by |
| --- | --- | --- |
| `install_notebook_path.py` | Writes `finstack_notebooks.pth` into the active environment so `import _shared` works from any notebook working directory. | `mise run python-sync`, `mise run python-build` |

## Code health and hygiene

| Script | Enforces | Task (tier) |
| --- | --- | --- |
| `check_loc.py` | The Rust source-file size rule: no hand-written Rust file under `src/` over 2000 lines, counted without tests (`#[cfg(test)]` items, `tests.rs`, `*_tests.rs`, `tests/`) and generated code; files over 1000 are summarised, `--full` lists them. | pre-commit `rust-loc` (PR) |
| `check_single_path.py` | The one-public-path rule over rustdoc JSON (nightly toolchain): duplicate public paths, cross-crate re-exports, renames and globs; `--compare <base>` fails when a public item became unnameable. | by hand after making a module private (local) |
| `audit_hardcoded_assumptions.py` | Flags source lines carrying market-convention, rating-agency, regulatory, accounting, or product assumptions that belong in a data registry. Reviewed matches live in `hardcoded_assumptions_allowlist.json`. `--format {text,markdown,json}`, `--include-allowed`, `--fail-on-candidates`. | `python3 scripts/audit_hardcoded_assumptions.py` (local) |
| `ci_report.py` | Formats Rust (`nextest` JUnit + LCOV), Python (`pytest` JUnit + Cobertura), WASM (`wasm-test` log), and SonarQube (`.scannerwork/report-task.txt` + SonarQube Cloud API) results into GitHub Actions `$GITHUB_STEP_SUMMARY` Markdown tables and workflow annotations. | `.github/workflows/checks.yml`, `.github/workflows/full-checks.yml` |
| `clean_workspace.py` | Removes build artifacts, virtualenvs, and caches. `--incremental` drops only Cargo incremental caches; `--wasm` drops only wasm-pack output and wasm32 target artifacts. | `mise run all-clean` |

## Tests

[`tests/`](tests/) holds pytest coverage for the checkers themselves — the gates
are load-bearing, so they are tested like production code. Fixtures under
`tests/fixtures/serde_audit/` are a synthetic Rust crate (`module_graph/`)
exercising module visibility, glob re-export bridges, duplicate public paths,
`cfg`-gated impls, and hidden items, plus `parser_cases.rs` for the lexer.

They run with the Python suite: `pyproject.toml` lists `scripts/tests` in
pytest's `testpaths`, so `mise run python-test` (PR) runs them.

```bash
uv run --no-sync pytest scripts/tests -q                         # just the checker tests
uv run pytest scripts/golden/quantlib/test_generate.py -q        # needs QuantLib
```
