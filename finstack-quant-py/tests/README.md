# finstack-quant-py tests

The pytest suite for the Python bindings. Everything here runs against the
**compiled extension**, not against source — build it first with
`mise run python-build` (the `python-test*` tasks do this for you).

pytest is configured from the repository-root `pyproject.toml`:
`testpaths = ["finstack-quant-py/tests"]`, collection patterns `test_*.py` /
`*_test.py`, and the markers `slow`, `perf`, `security`, `integration`.

## Layout

```
tests/
  conftest.py                 auto-marks expensive groups as `slow`
  tests_typed_helpers.py      shared canonical typed-instrument factories (not collected)
  test_*.py                   runtime/behavioural tests, one module per area
  parity/                     structural + return-shape parity against parity_contract.toml
  golden/                     external-benchmark pricing comparisons (marked `slow`)
  data/                       rendered tear-sheet goldens and JSON payloads
  fixtures/                   JSON baselines for attribution DataFrame tests
```

`tests_typed_helpers.py` is deliberately named so pytest does *not* collect it
(`tests_` ≠ `test_`). It defines each instrument family's canonical typed
instance exactly once, reused by the per-family modules and by
`test_typed_instruments_roundtrip.py`.

## Fast vs slow

`conftest.py` applies `pytest.mark.slow` to everything under `golden/` and to
`models/test_monte_carlo.py`. That drives the four task variants:

| Task | Runs |
|------|------|
| `mise run python-test` | `pytest -m 'not slow'` — the default dev loop |
| `mise run python-test-slow` | `pytest -m slow` — goldens and Monte Carlo only |
| `mise run python-test-all` | the whole suite (what CI runs) |
| `mise run python-test-cov` | full suite plus HTML coverage in `target/python-cov` |

Each of these rebuilds the extension via `mise run python-build` first.

`pytest-randomly` is installed, so **collection order is shuffled every run**
with a printed seed. If a failure looks order-dependent, reproduce it with
`-p no:randomly` or `--randomly-seed=<seed>` before assuming it is real.

## Behavioural tests (top level)

Roughly one module per binding area. The recurring groups:

| Prefix | What it covers |
|--------|----------------|
| `test_core_*`, `test_dates_*`, `test_money_decimal` | primitives, calendars/schedules, Decimal money, config validation, and the `ArrowTable` envelope |
| `test_models_credit_*` | credit master-scale, migration, and scoring engines owned by models |
| `test_analytics*`, `models/test_correlation` | the `Performance` panel facade and the `models.correlation` namespace |
| `test_cashflows*` | the cashflow JSON bridge and the typed `finstack_quant.cashflows` surface |
| `test_typed_*` | typed instrument constructors, keyword fidelity, JSON round-trips |
| `test_valuations_*`, `test_fx_delta_vol_surface`, `test_vol_cube_normal`, `test_arbitrage_bindings` | pricing entry points, their result wrappers, and the vol-surface inputs they consume |
| `test_attribution_entry`, `test_portfolio_*` | attribution entry points, positions, materialization, GIL release, liquidity |
| `test_factor_model_*`, `test_credit_factor_model_bindings`, `test_merton_model`, `test_credit_validation` | factor primitives and risk, the credit factor hierarchy, Merton structural credit, non-finite input rejection |
| `test_statements*`, `test_statements_analytics_*` | model graph, DCF/ECL, capital structure |
| `test_margin_*`, `models/test_monte_carlo`, `test_features_*`, `test_scenario_resolution_mode` | margin/IM/MVA, Monte Carlo, panel feature transforms, scenario resolution mode |
| `test_structured_credit_bindings`, `test_recovery_waterfall`, `test_envelope_diagnostics` | tranche analytics, recovery waterfalls, calibration-envelope diagnostics |
| `test_reporting_*` | the pure-Python `finstack_quant.reporting` presentation layer |
| `test_*_dataframes`, `test_leaf_dataframes`, `test_to_arrow_producers`, `test_arrow_interchange` | pandas/Arrow exits from result wrappers |
| `test_namespace`, `test_schema_access`, `test_schema_registry` | package topology, `__all__`, and the per-domain `finstack_quant.<domain>.schema` and workspace-wide `finstack_quant.schema` registries |
| `test_error_handling`, `test_error_hierarchy` | Rust error → Python exception mapping |
| `test_pickle_roundtrip`, `test_empty_frame_dtypes` | wrapper pickling and empty-frame dtype stability |
| `test_binding_ergonomics`, `test_binding_audit_fixes` | cross-cutting quant-facing contracts, and regression pins from the binding audit |

Five modules police the example tree rather than the bindings:

- `test_notebook_hygiene.py` — AST scan of `../examples/notebooks/` for
  failure-hiding constructs (bare `except` over finstack calls and friends).
  Cells that demonstrate a failure on purpose carry the `intentional-negative`
  tag.
- `test_run_all_notebooks.py` / `test_run_all_scripts.py` — unit-test the two
  example runners, plus targeted end-to-end executions of a few notebooks.
- `test_notebook_instrument_fixtures.py` — every instrument factory used by the
  maintained notebooks must still deserialize.
- `test_statements_test_a_example.py` — drives
  `../examples/scripts/statements_test_a.py` as a subprocess and asserts its
  report-writing is atomic and does not mutate the committed report.

`test_stub_doctests.py` executes the doctest examples embedded in the `.pyi`
stubs and pure-Python modules against the live extension, via
`scripts/run_python_stub_doctests.py`. `mise run python-doctest` runs just that
module.

## `parity/` — structural parity

These are the tests that keep Rust, Python and WASM from drifting. The
source-of-truth is [`../parity_contract.toml`](../parity_contract.toml).

| Module | Asserts |
|--------|---------|
| `test_contract_topology.py` | the contract matches reality on all three sides: Rust umbrella re-exports parsed out of `finstack-quant/src/lib.rs`, importability of every declared Python package/module (and non-importability of every `missing` one), `.pyi` top-level names, live `__all__` surfaces, and the WASM namespaces parsed out of `finstack-quant-wasm/index.js` + `exports/*.js` |
| `test_return_shapes.py` | what public entry points *return* — `wrapper` / `frame` / `series` / `json` / `scalar` / `list` / `dict`. Only `*_json`- and `*_from_spec`-named wire surfaces may return a JSON string; every result wrapper must carry `to_json` plus a `from_json` **staticmethod** and typed getters |
| `test_covenants_bindings.py` | focused runtime checks for the covenants slice (round-trip, DataFrame exit, unknown-field rejection, error mapping) |

`test_return_shapes.py` is the deliberate mirror of
[`../../finstack-quant-wasm/tests/return_shapes.rs`](../../finstack-quant-wasm/tests/return_shapes.rs):
same entries, same order, so a cross-language divergence reads as a one-screen
diff. Keep them edited together.

## `golden/` — external-benchmark pricing

Fixture-driven comparisons of Python pricing output against QuantLib,
Bloomberg, published formulas and textbook values. **The fixtures do not live
here** — they are shared with the Rust golden runner and live under
[`../../finstack-quant/valuations/tests/golden/data/`](../../finstack-quant/valuations/tests/golden/data),
whose [README](../../finstack-quant/valuations/tests/golden/README.md) carries
the provenance rules. `conftest.py` resolves them through its `DATA_ROOTS` map.

| File | Role |
|------|------|
| `schema.py` | dataclasses mirroring the `finstack_quant.golden/1` fixture schema; unknown keys are rejected |
| `tolerance.py` | the abs/rel comparator, mirroring the Rust one |
| `conftest.py` | fixture discovery, metadata validation, runner dispatch, comparison reporting |
| `pricing_validation.py` | validates a fixture's instrument envelope and metric names against the published schema before it is priced |
| `runners/pricing_common.py` | prices a fixture through `finstack_quant.valuations` and returns the requested metrics |
| `runners/sabr_smile.py` | the volatility-smile runner |
| `test_pricing_*.py` | one thin module per instrument family, each parametrized over `discover_fixtures(...)` |
| `test_volatility_sabr.py` | the same, for the `market_data/sabr` smile fixtures — closed-form vol generation, not instrument pricing |
| `test_walk.py` | every committed fixture must be reachable from some Python test |
| `test_conftest.py`, `test_schema.py`, `test_tolerance.py`, `test_pricing_runners.py` | unit tests for the harness itself, including the runner's snapshot/envelope market resolution |

What `conftest.py` enforces before a fixture is allowed to run:

- `metadata.source` is one of `quantlib`, `bloomberg-api`, `bloomberg-screen`,
  `intex`, `formula`, `textbook`, and pricing fixtures live in the directory
  that source implies (`pricing/quantlib/`, `pricing/bloomberg/`, or
  `pricing/regression_goldens/`).
- Capture and review provenance (`captured_by`/`captured_on`,
  `last_reviewed_by`/`last_reviewed_on`) is present and non-empty.
- Screenshot-sourced fixtures (`bloomberg-screen`, `intex`) carry at least one
  screenshot, under `screenshots/`, that exists on disk **and is git-tracked**.
- `expected` and `tolerances` have identical key sets, and every tolerance
  specifies `abs` or `rel`.
- A risk metric asserted as zero must explain itself via
  `tolerances[metric].tolerance_reason`.
- Rates and fixed-income fixtures must assert `dv01`; credit fixtures must
  assert both `dv01` and `cs01`.
- The instrument and market payloads must validate — `MarketContext.from_json`
  for snapshots, `validate_calibration_json` for envelopes.

Known unresolved external gaps are allowlisted per metric in
`../../finstack-quant/valuations/tests/golden/known_non_executable.json`. A
listed metric that starts passing fails the test as a stale entry. Set
`GOLDEN_IGNORE_NON_EXECUTABLE=1` to ignore the allowlist and surface every
failure — that is what `mise run goldens-test-strict` does.

Each run writes `target/golden-reports/golden-comparisons.csv` (actual,
expected, abs/rel diff, tolerance, pass flag) for analyst review.

Run both the Rust and Python golden layers together:

```bash
mise run goldens-test          # Rust golden test + python-build + pytest golden/
mise run goldens-test-strict   # same, ignoring known_non_executable.json
```

Regenerate the QuantLib-sourced fixtures with `mise run goldens-quantlib-generate`
(`...-check` verifies the committed files against the generator).

## `data/` and `fixtures/`

Small, hand-checked payloads for the reporting and attribution tests. None of
it is vendor data.

| File | Used by |
|------|---------|
| `data/attribution_bond.json` | `test_reporting_attribution.py` — a serialized `PnlAttribution` |
| `data/instrument_bond_result.json`, `data/instrument_bond_cashflows.json` | `test_reporting_instrument.py`, `test_valuation_metric_series.py` — a serialized `ValuationResult` plus its cashflow records |
| `data/*_tearsheet_golden.html` | byte-exact rendered tear sheets for `test_reporting_{performance,attribution,instrument}.py` |
| `fixtures/attribution_baseline.json` | `test_attribution_dataframes.py`, `test_empty_frame_dtypes.py` |

The HTML goldens are compared byte-for-byte against a tear sheet rendered from
a literal, RNG-free, clock-free input series with a pinned `generated` date.
Any deliberate change to the reporting layer means re-rendering them; there is
no `--update-goldens` flag, so regenerate by capturing the value the test
computes and writing it back.

## Conventions

- Determinism first: no wall clock, no network, no unseeded randomness. Tests
  that need a date pass one explicitly.
- Money is Decimal-backed; assert on `Money.amount_decimal()` (a
  `decimal.Decimal`) when exactness matters, and reserve `pytest.approx` for the
  f64 analytics paths.
- Inbound types deny unknown fields — several tests assert that a typo in a
  JSON payload raises rather than being silently dropped.

## See also

- [`../README.md`](../README.md) — the Python package overview
- [`../parity_contract.toml`](../parity_contract.toml) — the parity source-of-truth
- [`../examples/README.md`](../examples/README.md) — notebooks, scripts and reports
- [`../DOCS_STYLE.md`](../DOCS_STYLE.md) — docstring conventions for the bindings
- [`../../.agents/rules/python/code-standards.md`](../../.agents/rules/python/code-standards.md)
- [`../../.agents/rules/rust/testing-standards.md`](../../.agents/rules/rust/testing-standards.md)
- [`../../INVARIANTS.md`](../../INVARIANTS.md)
