# scripts

Repository maintenance and verification tooling. Almost everything here is a
gate: a checker that fails a `mise` task, a pre-commit hook, or a CI job when
the workspace drifts from a contract that the Rust compiler cannot enforce on
its own — documentation completeness, generated-artifact freshness, wire-format
stability, benchmark regressions, publish ordering.

These are Python because they inspect *all three* language surfaces (Rust,
PyO3, wasm-bindgen/TypeScript) and their generated artifacts, which no single
toolchain sees at once.

## Running them

`scripts/` is an importable package (`scripts/__init__.py`), so the multi-module
tools run as modules from the repository root:

```bash
uv run python -m scripts.serde_audit --check
uv run python -m scripts.golden.quantlib.generate --family pricing --product all
```

Single-file scripts run by path:

```bash
uv run python scripts/check_loc.py 800
uv run python scripts/generation_digest.py --verbose
```

Prefer the `mise` task that owns a script (listed below) — the tasks pass the
exact arguments CI uses. Checkers that touch no compiled extension are invoked
with `uv run --no-sync` so a plain `uv run` does not trigger a ~20 minute PyO3
rebuild; keep that flag when adding static checkers to a task.

The unit tests for these scripts live in [`tests/`](tests/) and run under
pytest; several `mise` tasks run them immediately before the script itself.
`tests/test_generate_core_reference_data.py` covers the generator at
`finstack-quant/core/build/generate_reference_data.py` and is driven by
`mise run core-check-data`.

## Documentation contract checkers

`AGENTS.md` requires every public callable with caller-supplied inputs to
document each input. These enforce that on each host surface.

| Script | Enforces | Driven by |
| --- | --- | --- |
| `check_public_api_input_docs.py` | Rust `# Arguments` coverage for every publicly reachable function, associated function, trait method, and constructor. Builds rustdoc JSON via `cargo public-api`, which needs the nightly toolchain `mise.toml` provisions. | `mise run rust-doc` |
| `check_python_api_input_docs.py` | Substantive summaries plus argument/return/raises/example sections on public classes and callables in `.pyi` stubs and pure-Python modules. NumPy- and Google-style sections both parse. | `mise run python-lint`, `mise run python-doc` |
| `check_pyo3_doc_placeholders.py` | Rejects placeholder prose left in public PyO3 runtime doc comments under `finstack-quant-py/src/bindings/`. | `mise run python-lint`, `mise run python-doc` |
| `check_wasm_api_input_docs.py` | `@param` coverage in the Rust doc comments under `finstack-quant-wasm/src/api/` attached to `#[wasm_bindgen]` exports — the text wasm-bindgen copies into the generated TypeScript declarations. The comment must precede the attribute or it is dropped. The hand-written facade declaration `finstack-quant-wasm/index.d.ts` is checked separately by `npm run docs:check`. | `mise run wasm-doc` |
| `check_deprecated_annotations.py` | The INVARIANTS.md §7.2 contract: every `#[deprecated]` carries `since = "X.Y.Z"`, a replacement or retention rationale, and a planned removal release. Bare `#[deprecated]` always fails. | `mise run rust-doc` |
| `run_python_stub_doctests.py` | Extracts `>>>` examples from `.pyi` and module docstrings and executes them against the compiled `finstack_quant` package, since stubs are not importable. | `mise run python-doctest` (via `finstack-quant-py/tests/test_stub_doctests.py`) |

## Generated artifacts and wire contracts

Rust serde types are the source of truth; JSON Schemas and TypeScript
declarations are deterministic publication artifacts. These prove the artifacts
match the types and that regeneration is idempotent.

| Script | Purpose | Driven by |
| --- | --- | --- |
| `serde_audit/` | Package: audits public Rust contract types for effective `Serialize`/`Deserialize`/`JsonSchema` coverage. A conservative module-aware Rust lexer (`lexer.py`, `resolution.py`, `scanner.py`) resolves modules, re-exports, aliases and `cfg_attr` derives; `registries.py` holds the maintained contract set and reviewed exceptions; `report.py` is the CLI (`--check` fails on missing capabilities or stale exceptions, `--report` lists everything). | `mise run rust-serde-audit` |
| `check_schema_residue.py` | Rejects obsolete or hand-maintained JSON/serde machinery: hand-written schema files, asymmetric optional wire adapters, non-canonical serde enum naming. | `mise run rust-check-schemas` |
| `check_schema_generation.py` | Regenerates the full schema tree into a temporary directory and compares byte-for-byte, then validates the `$ref` graph across registries. Catches non-reproducible emitters and dangling references. | `mise run rust-check-schemas` |
| `check_generated_instrument_fixtures.py` | Validates canonical generated instrument fixtures against `finstack-quant/valuations/tests/instruments/coverage_manifest.toml`. | `mise run gen-check` |
| `sync_generated_ts_index.py` | Writes (or with `--check`, verifies) the barrel `index.ts` for the ts-rs artifacts in `finstack-quant-wasm/types/generated/`. | `mise run wasm-gen-bindings`, `mise run gen-check` |
| `check_generated_ts.py` | Re-runs the ts-rs exporters into a temp directory and diffs against the committed declarations, without mutating the workspace. Expected paths come from `git ls-files` so case-only filename drift is visible on case-insensitive filesystems. | `mise run wasm-check-bindings` |
| `generation_digest.py` | Computes a stable SHA-256 over the generated artifacts named in the manifest. `--check-manifest` verifies that the file list still matches what is on disk; `gen-check` takes the digest before and after the verification pass and fails if it moved, so a "check" that quietly rewrote the workspace is caught. | `mise run gen-check` (`mise run all-ci` writes first via `gen-write`) |

`generated-artifacts.txt` is the manifest of tracked generated files. It covers
two trees: the generated instrument JSON examples under
`finstack-quant/valuations/tests/instruments/json_examples/` and the ts-rs
declarations under `finstack-quant-wasm/types/generated/`. The per-crate JSON
schema registries in `finstack-quant/*/schemas/` are not in it — their
reproducibility is proved by `check_schema_generation.py` instead. Update the
manifest with `generation_digest.py --write-manifest` when a new generated
artifact is added.

## Benchmarks and regression gates

These implement the portfolio-materialization acceptance record described in
[`../benchmarks/MATERIALIZATION_BENCHMARKS.md`](../benchmarks/MATERIALIZATION_BENCHMARKS.md).
The checked-in baselines live in `../benchmarks/materialization/`.

| Script | Purpose | Driven by |
| --- | --- | --- |
| `check_criterion_regressions.py` | Fails when Criterion median estimates regress beyond `--threshold` (0.10 in both compare tasks). Used both in generic discovery mode and in exact-path materialization mode. | `mise run rust-bench-compare`, `mise run materialization-rust-bench-compare` |
| `prepare_materialization_criterion_run.py` | Writes fresh run provenance (tree revision, combined fixture digest) before a comparison run, so a stale or wrong-revision measurement is rejected rather than compared. | both `*-bench-compare` tasks |
| `materialization_baseline_replace_flag.py` | Validates the `FQ_REPLACE_MATERIALIZATION_BASELINE` opt-in and emits the replace flag. Baselines are immutable without it. | both `*-bench-baseline` tasks |
| `manage_materialization_rust_baseline.py` | `guard` / `establish` / `seal` / `verify` subcommands for the Rust baseline JSON. | `mise run materialization-rust-bench-baseline`, `mise run materialization-benchmark-record` |
| `check_python_materialization_regressions.py` | The Python-side twin: `guard`, `prepare`, `establish`, `seal`, `compare`, `verify`. Rejects wrong revisions, fixture digests, case sets, and stale measurements before applying the median gate. | `mise run python-bench-portfolio-baseline`, `mise run python-bench-portfolio-compare` |
| `collect_materialization_results.py` | Combines raw Rust/Python/WASM samples into the tracked `materialization-benchmark-results.json` record: percentiles, independent bootstrap intervals, phase counters, environment, and the exact commands used. | `mise run materialization-benchmark-record` |
| `sync_materialization_benchmark_digest.py` | Writes (`--write`) or verifies (`--check`) the record SHA-256 quoted inside the Markdown document. | `mise run materialization-benchmark-doc-check`, `-record` |
| `check_materialization_benchmark_docs.py` | Verifies the Markdown toolchain lines, baseline artifact paths, identities, and hashes against the machine artifact — without running any measurement. | `mise run materialization-benchmark-doc-check` |

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
mise run goldens-quantlib-generate              # rewrite pricing fixtures
mise run goldens-quantlib-check                 # drift check only
mise run goldens-quantlib-attribution-generate
mise run goldens-quantlib-attribution-check
```

QuantLib (`quantlib>=1.41`, Python `dev` dependency group — run
`mise run python-sync` first) is needed only to *generate* fixtures. The
generated JSON is checked in, so the Rust and Python golden suites consume it
without a QuantLib install.

## Release and packaging

| Script | Purpose | Driven by |
| --- | --- | --- |
| `cargo_publish_checks.py` | Derives the crate publish order from internal dependencies, validates internal dependency versions, and dry-runs the first crate. | `mise run rust-publish-checks`, CI `Rust Publish Checks` |
| `smoke_python_wheel.py` | Imports the installed `finstack_quant` wheel, walks every public subpackage, and exercises `Currency`/`Money`, `models.bs_price`, and `EuropeanPricer.price_call`. Runs against the built wheel on every release platform except linux-arm64. | `.github/workflows/release.yml`, `finstack-quant-py/tests/test_smoke_python_wheel.py` |

## Code health and hygiene

| Script | Purpose | Driven by |
| --- | --- | --- |
| `check_loc.py` | Lists source files above a line limit (default 1000; positional override). Rust files are counted after stripping `#[test]` functions and `#[cfg(test)]` modules. `--ci` exits 1 on violations. | `mise run check-loc` |
| `audit_hardcoded_assumptions.py` | Flags source lines carrying market-convention, rating-agency, regulatory, accounting, or product assumptions that belong in a data registry. Reviewed matches live in `hardcoded_assumptions_allowlist.json`. `--format {text,markdown,json}`, `--include-allowed`, `--fail-on-candidates`. | `mise run assumptions-audit` |
| `clean_workspace.py` | Removes build artifacts, virtualenvs, and caches. `--incremental` drops only Cargo incremental caches; `--wasm` drops only wasm-pack output and wasm32 target artifacts. | `mise run all-clean`, `rust-clean-incremental`, `wasm-clean` |

## Tests

[`tests/`](tests/) holds pytest coverage for the checkers themselves — the gates
are load-bearing, so they are tested like production code. Fixtures under
`tests/fixtures/serde_audit/` are a synthetic Rust crate (`module_graph/`)
exercising module visibility, glob re-export bridges, duplicate public paths,
`cfg`-gated impls, and hidden items, plus `parser_cases.rs` for the lexer.

```bash
uv run pytest scripts/tests -q                  # all checker tests
uv run pytest scripts/golden/quantlib/test_generate.py -q   # needs QuantLib
```

Tasks that bundle the relevant tests with their checker:
`mise run check-loc`, `mise run rust-serde-audit`, `mise run rust-check-schemas`,
`mise run gen-check`, `mise run wasm-check-bindings`, `mise run python-doc`,
`mise run core-check-data`, `mise run rust-bench-compare`,
`mise run materialization-rust-bench-compare`,
`mise run python-bench-portfolio-baseline`,
`mise run python-bench-portfolio-compare`,
`mise run materialization-benchmark-doc-check`,
`mise run materialization-benchmark-record`.
