# Python binding tree

The PyO3 layer, ~65k lines across 13 domain directories plus a handful of shared
helpers. Every file here converts arguments, calls a workspace crate, and converts
the result back. Nothing computes: no pricing, no aggregation, no currency
arithmetic, no defaulting of financial parameters. When a Python caller gets a
wrong number the bug is in `../../../finstack-quant/`, not here.

The module tree mirrors the Rust umbrella crate one-for-one, and Python names match
Rust names exactly. That is the whole naming rule — no host-specific aliases, no
convenience re-exports, no legacy compatibility paths. See
[`../../../.agents/rules/python/code-standards.md`](../../../.agents/rules/python/code-standards.md).

Crate-level context (namespaces, result-return contract, conventions that bite
Python callers) is in [`../../README.md`](../../README.md). Per-item detail comes
from `help()` at the REPL and from the `.pyi` stubs; there is no published rustdoc
for this crate.

## Layout

`../lib.rs` is a thin `#[pymodule]` that delegates to `mod.rs::register_root`.
`../errors.rs` owns every Rust→Python error conversion. Everything else lives here.

| Path | Visibility | Role |
|------|-----------|------|
| `mod.rs` | private (`mod bindings;` in `../lib.rs`) | Declares every submodule; `register_root` sets `__package__`, `__version__` (from `CARGO_PKG_VERSION`), calls the 13 domain `register` functions plus `schema::register` in a fixed order, and sets the root `__all__` |
| `analytics/` | `pub` | `Performance` and `constrained_least_squares` — `performance.rs`, `regression.rs`, `types.rs` |
| `attribution/` | `pub` | `entry.rs` (spec pipeline), `pnl_attribution.rs`, `return_contribution.rs`, `schema.rs` |
| `cashflows/` | `pub` | `builder/` (`orchestrator.rs`, `schedule.rs`, `specs.rs`), `accrual.rs`, `aggregation.rs`, `primitives.rs`, `schema.rs` |
| `core/` | `pub` | Most files (45), though `valuations/` is larger by line count: `dates/`, `market_data/` (with `curves/`), `math/`, `credit/`, plus `config.rs`, `currency.rs`, `money.rs`, `types.rs`, `rating_scales.rs`, `table.rs`, `schema.rs` |
| `calibration/` | `pub` | Calibration envelopes, diagnostics, schema registry, and explicit model-calibration helpers |
| `covenants/` | `pub` | `report.rs` (`PyCovenantReport`) plus JSON spec/report validators in `mod.rs` |
| `features/` | `pub` | Single file; all transforms are free functions on `mod.rs` |
| `margin/` | `pub` | `calculators.rs`, `im.rs`, `regulatory.rs`, `xva.rs`, `metrics.rs`, `types.rs`, `sensitivity_frame.rs`, `schema.rs` |
| `models/` | `pub` | Analytical, Fourier, SABR, credit, correlation, and factor bindings plus `monte_carlo/` (`engine.rs`, `pricers.rs`, `greeks.rs`, `analytical.rs`, `results.rs`, `time_grid.rs`) |
| `portfolio/` | `pub` | 34 files: `factor_model/` and `optimization_spec/` subtrees plus per-method attribution (`brinson.rs`, `factor_brinson.rs`, `fi_attribution.rs`, `grid_attribution.rs`, `excess_return.rs`), `materialization.rs`, `pipeline.rs`, `sensitivity.rs`, `liquidity.rs`, `replay.rs`, `types.rs` |
| `scenarios/` | `pub` | `engine.rs`, `horizon.rs`, `operation_spec/` (typed authoring path), `schema.rs` |
| `statements/` | `pub` | `builder.rs`, `evaluator.rs`, `types.rs`, `capital_structure.rs`, `checks.rs`, `adjustments.rs`, `monte_carlo.rs`, `dsl.rs`, `schema.rs` |
| `statements_analytics/` | `pub` | `analysis.rs`, `typed.rs`, `corkscrew.rs`, `ecl.rs`, `scorecards.rs`, `comps.rs`, and the `templates_*.rs` family |
| `valuations/` | `pub` | Instrument, market, and product-pricing bindings: `typed_rates.rs`, `typed_fx.rs`, `typed_equity.rs`, `typed_legs.rs`, `typed_credit/`, `typed_structured_credit/`, plus `pricing.rs`, `instruments.rs`, `exotic_rates.rs`, and `schema.rs` |
| `schema.rs` | `pub` | The workspace-wide `finstack_quant.schema` namespace: merges all ten per-domain registries and labels each row with its owning domain |
| `schema_registry.rs` | `pub(crate)` | `registry_index` / `find_artifact` / `render_profile` / `validate_against`, and the `schema_registry_functions!` macro that generates the identical `index` / `get` / `validate` trio for each `finstack_quant.<domain>.schema` |
| `module_utils.rs` | `pub(crate)` | Submodule registration; also `py_to_json_value`, `py_to_json_string`, `py_to_serde`, `parse_currency`, `parse_date` |
| `extract.rs` | `pub(crate)` | Polymorphic "typed object **or** canonical JSON string" extraction — the `*Access` enums (`ModelAccess`, `ResultAccess`, `MarketAccess`, `PortfolioAccess`, `ValuationAccess`, `PortfolioResultAccess`) deref to the borrowed Rust type so no clone or re-parse happens on the typed path |
| `pandas_utils.rs` | `pub(crate)` | DataFrame/Series construction (`dict_to_dataframe`, `serde_rows_to_dataframe*`, `table_to_dataframe`, `labeled_values_to_series`, `dates_to_datetime_index`, …). Caches the `pandas.DataFrame` / `pandas.Series` constructors in `PyOnceLock` |
| `date_utils.rs` | `pub(crate)` | `parse_iso_date_py`, `extract_date`, `extract_date_iso` — accept either an ISO string or any date-like Python object |
| `pickle_support.rs` | `pub(crate)` | `reduce_via_json`: the shared `__reduce__` over `to_json`/`from_json`, with a guard that refuses payloads whose JSON does not round-trip |
| `repr_support.rs` | `pub(crate)` | `repr_from_serde`: renders `Name(field=value, …)` from a value's serde form, eliding past six fields |

The `pub` on the domain directories is documentation, not a boundary: the crate is
`crate-type = ["cdylib"]`, so nothing outside it can `use` anything. The distinction
that matters is `pub(crate)` on the seven shared helper modules — those are
plumbing, and a domain module should reach for them rather than re-implementing the
same conversion.

## Registration

One `register(py, parent) -> PyResult<()>` per domain, called from `register_root`.
The pattern, from `analytics/mod.rs`:

```rust
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "analytics")?;
    m.setattr("__doc__", "Performance analytics centred on the Performance class.")?;
    types::register(py, &m)?;
    performance::register(py, &m)?;
    regression::register(py, &m)?;
    let all = PyList::new(py, ["Performance", "PeriodStats", /* … */])?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )
}
```

Rules that are load-bearing:

- Set `__all__` with `PyList` inside `register`; never return an export list for the
  parent to assemble. Keep it exhaustive.
- Every module sets `__doc__`.
- Create every compiled module with `module_utils::new_submodule(parent, name)`,
  never `PyModule::new`. It names the module by its **public** import path
  (`finstack_quant.models.credit.lgd`, not the extension-internal
  `finstack_quant.finstack_quant.…`), derived from `parent.__package__`, and sets
  `__name__` and `__package__` before anything is added — so child modules, and the
  `__module__` of every function added afterwards, inherit the public path. Do not
  patch `__name__`, `__package__` or `__module__` afterwards.
- Finish with `module_utils::attach_submodule(parent, &m, exposure)`. Pass
  `Exposure::Compiled` when `import finstack_quant.x.y` should return the compiled
  module itself: it gets a `__spec__` and is registered in `sys.modules` under its
  public path. Pass `Exposure::Python` when a pure-Python package or module at that path
  (`../../finstack_quant/x/y/__init__.py` or `…/y.py`) re-exports it: the Python
  file owns the `sys.modules` key, and registering the compiled module there would
  replace it or keep it from ever running.
  `tests/test_module_names.py` fails if either choice is wrong.
- Each root namespace also has a pure-Python shim under `../../finstack_quant/`
  (a package per domain, `schema.py` for the workspace registry) that re-exports
  the compiled module and owns the docstring and stubs. A new namespace needs both
  halves; a root namespace without a Python file is not importable until
  something else has loaded the extension.

## Wrapper conventions

```rust
#[pyclass(name = "Currency", module = "finstack_quant.core.currency", frozen)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyCurrency {
    pub(crate) inner: Currency,
}
```

- Named struct, field `inner`, `pub(crate)` so sibling modules can borrow it. Prefix
  the Rust type `Py*`; the Python-visible name comes from `name = "..."` and must
  equal the Rust name.
- Set `module = "..."` to the namespace the class is actually registered under —
  it is what makes `repr()` and pickling resolve.
- `#[new]` for the primary constructor, `#[classmethod]` for alternates,
  `#[staticmethod]` for `from_json`. Prefer `frozen` where the Rust type is a value.
- Every public function and constructor carries `#[pyo3(text_signature = "...")]`
  and a `///` doc comment documenting each caller-supplied input. Both are gated:
  `scripts/check_pyo3_doc_placeholders.py` and `scripts/check_python_api_input_docs.py`,
  wired into `mise run python-lint` and `mise run python-doc`.
- Result wrappers carry typed getters, `to_json`, a `#[staticmethod] from_json`, and
  `to_dataframe()` (plus `to_series()` for 1-D labeled vectors). Computation entry
  points return the typed wrapper, not a JSON string; only `*_json`-suffixed
  functions return JSON, and each must have a typed twin. See
  [`../../../.agents/skills/finstack-consistency-reviewer/conventions.md`](../../../.agents/skills/finstack-consistency-reviewer/conventions.md).
- Reuse the shared helpers rather than hand-rolling: `reduce_via_json` for
  `__reduce__`, `repr_from_serde` for `__repr__`, `pandas_utils::*` for every
  DataFrame exit. All three are already used across a hundred-plus wrappers; a
  hand-written variant is drift.
- Accept `typed | str` through `extract.rs` where a pipeline function takes a model,
  market, portfolio, or result. The typed path borrows; the string path deserializes
  the same serde contract, so both observe identical payloads.

## Errors

Route everything through `../errors.rs`. Inline `PyValueError::new_err(e.to_string())`
bypasses the error-chain preservation the helpers provide and is a review reject.

| Helper | Use |
|--------|-----|
| `core_to_py` | `finstack_quant_core::Error` — by far the most common |
| `display_to_py` | Any `Display` error without a dedicated mapper |
| `portfolio_to_py`, `statements_to_py`, `analytics_to_py` | Crate-specific root errors |
| `pd_calibration_to_py`, `migration_to_py` | `models::credit` sub-errors |
| `contract_to_py`, `materialization_to_py`, `diagnostics_to_py` | Persisted-contract and materialization paths that attach structured diagnostics |
| `value_error`, `serde_json_to_py` | Constructing a new error in binding code |

Broad shape: missing id → `KeyError`; validation / bad argument → `ValueError`;
calibration or operational failure → `RuntimeError`. Named exceptions descend from
`FinstackError` (itself a `ValueError`, so pre-existing `except ValueError` handlers
keep working); the sole carve-out is `CalibrationEnvelopeError`, which derives from
`RuntimeError` because `pyo3::create_exception!` accepts only one base. The
hierarchy is drawn in the `../errors.rs` module docs.

The crate root denies `unwrap`, `expect`, and `panic` outside `#[cfg(test)]`
(`../lib.rs`), and there are currently zero `.unwrap()` calls in non-test code here.

## Other invariants

- **No cross-currency math.** Conversions go through the Rust `FxProvider`; a
  binding never adds two `Money` values of different currencies or picks an FX rate.
- **No binding-invented defaults.** If Rust requires a parameter, so does Python.
  A default that exists only in the binding silently diverges from the canonical API.
- **Release the GIL around real work.** Wrap the Rust call in `py.detach(|| …)` for
  anything that can run long — pricing, calibration, simulation, portfolio
  aggregation. Never hold Python objects across the detach.
- **Determinism.** Seeded APIs take an explicit seed; a binding never generates one.
- **Serde strictness.** Inbound envelopes deny unknown fields — do not pre-filter or
  "clean" a payload before handing it to serde.

## Verification

The crate sets `[lib] test = false, doctest = false` and is excluded from
`mise run rust-test`:
building its test target links against libpython, which a plain `cargo` environment
does not have. It is still type-checked, because `mise run rust-lint` runs clippy
with `--workspace` and clippy does not link. Binding behavior is tested from Python
against the built extension.

```bash
mise run rust-lint          # fmt --check + clippy --workspace --lib --bins --tests --examples -D warnings
mise run python-build       # maturin develop; required before any pytest run
mise run python-lint        # ruff + the two PyO3/Python doc checkers
mise run python-doc         # doc completeness gates on their own
uv run pytest finstack-quant-py/tests/parity -q      # contract topology + return shapes
uv run pytest finstack-quant-py/tests -q -m 'not slow'
```

Never run `cargo test` directly in this workspace.

Every rename or addition on the parity-tested surface must land in
[`../../parity_contract.toml`](../../parity_contract.toml) in the same change;
`tests/parity/test_contract_topology.py` fails in both directions.

## Related

- [`../../README.md`](../../README.md) — package overview and caller-facing conventions
- [`../../tests/README.md`](../../tests/README.md) — suite layout, goldens, fixtures
- [`../../DOCS_STYLE.md`](../../DOCS_STYLE.md) — stub and rustdoc documentation bar
- [`../../finstack_quant/reporting/README.md`](../../finstack_quant/reporting/README.md)
  — the pure-Python presentation layer that consumes these results
- [`../../../finstack-quant-wasm/src/api/README.md`](../../../finstack-quant-wasm/src/api/README.md)
  — the WASM mirror of this tree
- [`../../../.agents/rules/python/code-standards.md`](../../../.agents/rules/python/code-standards.md)
