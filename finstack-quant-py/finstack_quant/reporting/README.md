# Reporting

Publication-quality tear sheets rendered as self-contained HTML with inline SVG.
This is the only public `finstack_quant.*` namespace with no Rust crate behind it:
it is pure Python, has no rustdoc, and has no WASM twin.

It is a **presentation layer only**. Every module reads values that an engine has
already computed and lays them out. Nothing here prices, aggregates, converts
currency, or derives a statistic. If a number is missing from a result, the fix is
to expose it from the Rust crate and its binding — not to compute it in a
`_section_*` helper. The exemption from crate-mirroring that lets this directory
exist is recorded in
[`../../../.agents/rules/python/code-standards.md`](../../../.agents/rules/python/code-standards.md)
and in `parity_contract.toml` under `[crates.reporting]` (`rust_crate = ""`).

## Layout

| File | Role |
|------|------|
| `__init__.py` | Public surface: re-exports the ten `*_tearsheet` functions plus `Theme` and `INSTITUTIONAL`. Its `__all__` is contract-pinned |
| `theme.py` | `Theme` (frozen dataclass of design tokens) and `Theme.to_css(scope)`; the `INSTITUTIONAL` house style instance |
| `document.py` | Composition layer: `TearSheet`, `Section`, `KPI`, the scope constant `_SCOPE = "fq-ts"`, the tooltip `<script>`, and `_resolve_sections` |
| `format.py` | Display formatters: `pct`, `ratio`, `money`, `sign_class`, `fmt_date`; the internal `_escape_html`, `_missing`, `_dates_of` |
| `charts.py` | Hand-built inline-SVG primitives: `line_chart`, `bar_chart`, `waterfall_chart`, `tornado_chart`, `fan_chart`, `cashflow_ladder`, plus `rgba`, `nice_ticks`, `color_scale` |
| `tables.py` | HTML table primitives: `kv_table`, `data_table`, `heatmap`, `scroll` |
| `statements_common.py` | Shared statement-result access: `StatementView`, `parse_statement`, `json_or_dict`, `pl_matrix_table`, `variance_table`, and the reusable `_section_variance` |
| `performance.py` | `performance_tearsheet` — cumulative, stats, drawdown, rolling, monthly heatmap, drawdown episodes |
| `benchmark.py` | `benchmark_tearsheet` — alpha/beta, capture, rolling greeks, relative series, optional multi-factor block |
| `attribution.py` | `attribution_tearsheet` — single-instrument T0→T1 P&L waterfall plus factor / carry / credit detail |
| `instrument.py` | `instrument_tearsheet` — a priced `ValuationResult`: definition terms, analytics, key-rate buckets, cashflows, schedule, payoff, survival, covenants |
| `portfolio.py` | `portfolio_tearsheet` — holdings, exposure, sensitivities, tenor buckets, cashflow ladder |
| `portfolio_risk.py` | `portfolio_risk_tearsheet` — Euler VaR/ES contributions and risk budget |
| `statement.py` | `statement_tearsheet` — P&L summary, trend, margins, variance |
| `credit.py` | `credit_tearsheet` — leverage/coverage trend, per-instrument coverage, covenant compliance, EBITDA build |
| `dcf.py` | `dcf_tearsheet` — EV→equity bridge, UFCF projection, sensitivity tornado, forecast summary |
| `scenario.py` | `scenario_tearsheet` — driver tornado, scenario comparison, Monte-Carlo percentile fan, variance vs baseline |

There are no `.pyi` stubs here. The `.py` files *are* the IntelliSense surface, so
they are held to the same docstring bar as the stubs elsewhere in the package (see
[`../../DOCS_STYLE.md`](../../DOCS_STYLE.md)).

## Public API vs internal

Public — the twelve names in `__init__.__all__`, and only those:

```
INSTITUTIONAL  Theme
attribution_tearsheet  benchmark_tearsheet  credit_tearsheet  dcf_tearsheet
instrument_tearsheet   performance_tearsheet  portfolio_risk_tearsheet
portfolio_tearsheet    scenario_tearsheet     statement_tearsheet
```

That list is checked in both directions by
`tests/parity/test_contract_topology.py::test_contract_symbols_match_live_surface`
against `[crates.reporting.symbols].public` in
[`../../parity_contract.toml`](../../parity_contract.toml). Adding a public name
without updating the contract fails the suite, and so does the reverse.

Everything else is a shared primitive for building tear sheets. `charts`, `tables`,
`format`, `document`, and `statements_common` each declare a module `__all__` and
are importable by their full path, but they are not re-exported from the package
and are not covered by the parity contract — treat them as internal to this
directory. Names prefixed with `_` (`_escape_html`, `_resolve_sections`,
`_section_variance`, every `_section_*` builder) are private even within it.

`ALL_SECTIONS` is defined per tear-sheet module and is deliberately *not* in the
package `__all__`; import it as
`from finstack_quant.reporting.performance import ALL_SECTIONS` when you need the
valid section names.

## The tear-sheet contract

Every `*_tearsheet` returns a `TearSheet` and takes the same keyword-only tail:

| Keyword | Meaning |
|---------|---------|
| `sections` | Subset of that module's `ALL_SECTIONS`, in the order given. `None` renders all. An unknown name raises `ValueError` via `_resolve_sections` |
| `title`, `subtitle` | Heading overrides; `None` derives from the payload |
| `theme` | A `Theme`; defaults to `INSTITUTIONAL`. Build variants with `dataclasses.replace` |
| `generated` | The stamped generation date. Pass a fixed `datetime.date` for reproducible output; omit it and no stamp is rendered |

Positional inputs differ by sheet, and `scenario_tearsheet` has none — it is
entirely keyword-driven. Several accept either a typed binding object or its
canonical JSON/dict payload (normalized through `statements_common.json_or_dict`,
`parse_statement`, or the wrapper's own `from_json`). Three do not:
`performance_tearsheet` and `benchmark_tearsheet` need a live
`analytics.Performance`, and `instrument_tearsheet` raises `TypeError` on a `str`
or `dict` rather than re-parsing a `ValuationResult`.

`TearSheet` renders three ways:

- `_repr_html_()` — scoped fragment for inline Jupyter display
- `to_html()` — standalone `<!DOCTYPE html>` document
- `save(path)` — writes `to_html()` as UTF-8

## Conventions a contributor must honor

- **No financial logic.** Sign flips for display direction (the DCF bridge's net-debt
  step), `x * 100.0` (or `x * 10000.0` for bp) to move a decimal the engine labelled
  as such into display units, magnitude sort keys for rows the engine returns
  unordered, and top-N selection are the whole permitted set. An engine-ordered
  result (the Rust tornado ranking) keeps its order. Each module's docstring states
  which of these it uses; keep that note accurate.
- **Units come from Rust.** A metric's unit family is read from the result
  (`ValuationResult.metric_units()`), never from a Python table; only display labels
  live here. Shares and rolling windows are likewise read from Rust
  (`PnlAttribution.pct_of_total`, `PeriodKind.periods_per_year`).
- **Provenance is read, never asserted.** A numeric-mode header line comes from the
  result's own stamp (`StatementResult` `meta.numeric_mode`, `ValuationResult`
  meta); a sheet whose input carries no stamp prints none.
- **Typed results are inputs.** Every result argument is normalised through
  `statements_common.json_or_dict` (typed → `to_json()`), so a typed Rust result
  renders exactly like its dict form; an unsupported type raises `TypeError`
  rather than silently dropping its section.
- **Units at the boundary.** `format.pct` takes a value *already in percentage
  points* — `13.2` renders `13.2%`. Engine metrics that arrive as decimals are scaled
  by the caller at the call site, not inside `pct`. `charts.line_chart(..., y_pct=True)`
  only appends `%` to tick labels; it does not scale.
- **Missing renders as `·`.** `format._missing` catches `None` and float `NaN`, and
  every formatter returns the `·` placeholder rather than `nan`. Charts skip missing
  points instead of plotting zero.
- **Escape everything.** Text into HTML goes through `format._escape_html`; text into
  SVG attributes or `<title>` goes through `charts._xml_attr` (which escapes `&`
  first, then `<`, `>`, `"`).
- **Determinism.** No wall clock, no RNG, no locale-dependent formatting anywhere in
  this directory — `generated` is the only date and the caller supplies it. The CSS
  scope class is the fixed constant `fq-ts`. Three tear sheets are pinned
  byte-for-byte against goldens, so any rendering change is a golden change.
- **No plotting dependency.** `charts.py` imports only `math`, `typing`, and its two
  siblings. Charts are strings built by hand, sized through `viewBox` (`_W = 620`)
  with `width:100%; height:auto`. Do not reach for matplotlib.
- **pandas is duck-typed.** `pandas` and `numpy` are package-level runtime
  dependencies, but nothing here imports them: modules consume whatever
  `.to_dataframe()` returned from the compiled bindings via `.index`, `.columns`,
  `.iloc`, `.tolist()`. Keep it that way — it is what lets `format._dates_of` accept
  both a `DatetimeIndex` and a plain list.
- **Cross-namespace imports are the exception.** Three modules import from other
  `finstack_quant` namespaces, and each does so to *read* a canonical helper
  rather than to recompute one: `instrument.py`
  (`cashflows.aggregation.calendar_year_ladder`, `valuations.vanilla_expiry_payoff`)
  and `portfolio.py` (`core.dates.Tenor`, `portfolio.PortfolioMetrics`,
  `portfolio.net_in_currency_by_date`) at module scope, and `attribution.py`
  (`attribution.PnlAttribution`) inside the function body, so the import is paid
  only on the `str`/`dict` re-parse path.

## Adding a tear sheet

1. New module `finstack_quant/reporting/<name>.py` with `ALL_SECTIONS`, private
   `_section_*` builders returning `Section | None`, and one public
   `<name>_tearsheet(...) -> TearSheet` carrying the keyword tail above.
2. Route section selection through `document._resolve_sections` so unknown names
   raise consistently.
3. Import and re-export it from `__init__.py`, and add the name to `__all__`
   (sorted; ruff `RUF022` enforces the ordering).
4. Add the same name to `[crates.reporting.symbols].public` in
   `parity_contract.toml`.
5. Write NumPy-style docstrings with `Parameters` / `Returns` / `Raises` /
   `Examples` on every public callable — `scripts/check_python_api_input_docs.py`
   scans this directory and `mise run python-doctest` *executes* the examples.
6. Add `tests/test_reporting_<name>.py`.

## Tests

Tests live beside the rest of the Python suite in
[`../../tests/`](../../tests/README.md), one `test_reporting_<module>.py` per
module here — `theme`, `format`, `document`, `tables`, `charts`,
`statements_common`, and the ten tear sheets.
(`test_reporting_periodic_returns.py` is misnamed: it covers
`analytics.Performance.to_periodic_returns_dataframe`, not this directory.)

`tests/data/{performance,attribution,instrument_bond}_tearsheet_golden.html` are
byte-exact renders from RNG-free, clock-free inputs with a pinned `generated` date.
There is no `--update-goldens` flag; regenerate by capturing what the test computes
and writing it back.

```bash
mise run python-build                                   # required: tests hit the extension
uv run pytest finstack-quant-py/tests -k reporting -q
mise run python-lint                                    # ruff + the API-doc checkers
mise run python-doctest                                 # executes the docstring examples
```

## Related

- [`../../README.md`](../../README.md) — the `finstack-quant-py` package
- [`../../tests/README.md`](../../tests/README.md) — suite layout and golden policy
- [`../../DOCS_STYLE.md`](../../DOCS_STYLE.md) — docstring requirements
- [`../../src/bindings/README.md`](../../src/bindings/README.md) — the PyO3 layer
  that produces every result this directory renders
