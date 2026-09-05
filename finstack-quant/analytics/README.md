# finstack-quant-analytics

Portfolio performance and risk analytics over numeric return series and
`finstack_quant_core::dates::Date`. No DataFrame or Polars dependency: inputs
are `Vec<Vec<f64>>` panels and `Vec<Date>` grids, outputs are `Vec<f64>` or
small serde-derived result structs.

The crate also owns the shared correlation-matrix validation and PSD-repair
helpers used by `finstack-quant-models` and `finstack-quant-valuations`.

## Position in the stack

`finstack-quant-core` is the only workspace crate it depends on; the sole
non-trivial third-party dependency is `nalgebra`, used by the multi-factor
regression and the constrained least-squares solver. Consumed by:

| Consumer | What it uses |
|----------|--------------|
| `finstack-quant-models` | `beta` (OLS slope in the credit peel), `correlation::{nearest_correlation_matrix, validate_correlation_matrix, NearestCorrelationOpts}` |
| `finstack-quant-valuations` | `correlation::*`, re-exported verbatim through `finstack_quant_models::correlation` |
| `finstack-quant` (umbrella) | re-exported as `finstack_quant::analytics` |
| `finstack-quant-py`, `finstack-quant-wasm` | `Performance`, `regression::constrained_least_squares` |

## Entry point

[`Performance`](src/performance/mod.rs) is the entry point. Construct it from a
price panel (`Performance::new`) or a return panel
(`Performance::from_returns`); every analytic is then a method on that
instance.

```rust
use finstack_quant_analytics::Performance;
use finstack_quant_core::dates::{Date, Month, PeriodKind};

let dates: Vec<Date> = (1..=10)
    .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
    .collect();
let prices = vec![(0..10).map(|i| 100.0 + i as f64).collect::<Vec<_>>()];
let perf = Performance::new(
    dates,
    prices,
    vec!["SPY".into()],
    None,                 // benchmark ticker; None selects column 0
    PeriodKind::Daily,
)
.unwrap();

assert_eq!(perf.ticker_names(), &["SPY"]);
let sharpe = perf.sharpe(0.0); // Vec<f64>, one entry per ticker
assert_eq!(sharpe.len(), 1);
```

Method families on `Performance`:

| Family | Source | Examples |
|--------|--------|----------|
| Scalars | [`performance/scalar.rs`](src/performance/scalar.rs) | `cagr`, `mean_return`, `volatility`, `sharpe`, `sortino`, `calmar`, `omega_ratio`, `gain_to_pain`, `modified_sharpe`, `geometric_mean`, `downside_deviation` |
| Tail risk | [`performance/scalar.rs`](src/performance/scalar.rs) | `value_at_risk`, `expected_shortfall`, `parametric_var`, `cornish_fisher_var`, `tail_ratio`, `skewness`, `kurtosis`, `cdar` |
| Drawdown | [`performance/scalar.rs`](src/performance/scalar.rs), [`performance/aggregation.rs`](src/performance/aggregation.rs) | `max_drawdown`, `mean_drawdown`, `max_drawdown_duration`, `drawdown_series`, `drawdown_details`, `ulcer_index`, `martin_ratio`, `sterling_ratio`, `burke_ratio`, `pain_index`, `pain_ratio`, `recovery_factor` |
| Benchmark-relative | [`performance/benchmark.rs`](src/performance/benchmark.rs) | `beta`, `greeks`, `rolling_greeks`, `multi_factor_greeks`, `tracking_error`, `information_ratio`, `r_squared`, `treynor`, `m_squared`, `up_capture`, `down_capture`, `capture_ratio`, `batting_average` |
| Rolling series | [`performance/rolling.rs`](src/performance/rolling.rs) | `rolling_returns`, `rolling_volatility`, `rolling_sharpe`, `rolling_sortino` |
| Panels and periods | [`performance/aggregation.rs`](src/performance/aggregation.rs) | `returns`, `cumulative_returns`, `excess_returns`, `correlation_matrix`, `periodic_returns`, `period_stats`, `lookback_returns`, `cumulative_returns_outperformance`, `drawdown_difference` |
| Window and benchmark reset | [`performance/mod.rs`](src/performance/mod.rs) | `reset_date_range`, `reset_bench_ticker`, `active_dates`, `active_dates_for_ticker`, `returns_for_ticker` |

The per-domain modules `returns`, `risk_metrics`, `drawdown`, `benchmark`,
`aggregation`, and `lookback` are `pub(crate)`; only the result and config
types they define are re-exported at the crate root, because `Performance`
returns them.

## Public surface

| Item | Module | Notes |
|------|--------|-------|
| `Performance` | `performance` | Entry point |
| `LookbackReturns` | `performance` | `mtd` / `qtd` / `ytd` / `fytd`, one entry per ticker |
| `PeriodStats` | `aggregation` | Best/worst, win rate, streaks, payoff and profit factors, CPC index |
| `DrawdownEpisode` | `drawdown` | Returned by `Performance::drawdown_details` |
| `BetaResult`, `GreeksResult`, `RollingGreeks`, `MultiFactorResult`, `ReturnKind` | `benchmark` | Returned / consumed by benchmark methods |
| `CagrDayCount` | `risk_metrics` | Act/365.25 default or a wrapped core `DayCount` |
| `DatedSeries` | `risk_metrics` | Returned by `Performance::rolling_*` |
| `beta` | `benchmark` | Freestanding OLS slope; consumed by `finstack-quant-models::factor` |
| `correlation` | `correlation` | Public module: shared row-major correlation validation and repair |
| `regression` | `regression` | Public module: `constrained_least_squares` |

All other analytics building blocks are `pub(crate)`.

### `correlation`

Canonical home for the shared correlation-matrix helpers. Exports:

- `validate_correlation_matrix(matrix, n)` — square-shape, unit-diagonal,
  symmetry, `[-1, 1]` bounds, and PSD (Cholesky) checks, classifying failures
  as the canonical core `CorrelationError`, re-exported here as `Error`.
- `nearest_correlation_matrix(matrix, n, opts)` with `NearestCorrelationOpts` —
  Higham (2002) alternating-projection PSD repair.
- `Error` — re-export of
  `finstack_quant_core::math::linalg::CorrelationError`.
- `Result<T>` — analytics-local convenience alias for
  `std::result::Result<T, Error>`. The analytics crate does not define a
  separate correlation error or depend on `thiserror`.

`finstack_quant_models::correlation` re-exports the matrix helpers and opts
unchanged, and owns a wider `Error` that wraps this crate's matrix failures
plus credit-domain variants. That merged namespace is a documented deviation
from strict crate-mirroring, recorded in
[`finstack-quant-py/parity_contract.toml`](../../finstack-quant-py/parity_contract.toml).

### `regression`

`constrained_least_squares` is an equality-constrained least-squares solver.
`finstack-quant-portfolio`'s factor-Brinson documentation and error messages
point callers at it for pre-solving factor return vectors; portfolio does not
link against this crate.

## Conventions

- Returns are simple decimal returns (`0.01` is 1%), not percentages.
- Annualization is derived from `finstack_quant_core::dates::PeriodKind`.
- Risk-free rates that enter Sharpe, Treynor, M², modified Sharpe, Jensen,
  and `ReturnKind::Total` are geometrically decompounded:
  `rf_period = (1 + rf_annual)^{1/N} − 1`. Sterling, Calmar, and pain keep
  `CAGR − rf_annual` (both already annual).
- `excess_returns` subtracts a panel-aligned `rf` series (`rf.len()` must
  equal the active date grid). `nperiods: None` decompounds at `self.ann()`.
- CAGR defaults to Act/365.25 via `CagrDayCount`. Wrap a core `DayCount`
  for Act/365F, Act/Act, Bus/252, and so on; Bus/252 requires a calendar.
- Historical VaR, ES, tail ratio, parametric VaR, and Cornish–Fisher VaR
  return `NaN` on empty or invalid series. Parametric and CF horizon
  `None` is one period, not `ann()`.
- Drawdown and compounding share one log-space Neumaier wealth engine.
- FYTD is the first observation on or after the fiscal calendar start;
  holidays are not skipped with `Following`.
- Calmar is CAGR / |max DD| over the **active window**, not Young's
  36-month CTA definition.
- `correlation_matrix` uses complete-case when every ticker has ≥2 points
  on the common span, otherwise pairwise, then Higham. Degenerate pairs
  or repair failure return `Err`.
- `multi_factor_greeks` takes `ReturnKind::Excess` or
  `ReturnKind::Total { risk_free_rate }`. Factors are already-excess;
  Total subtracts decompounded rf from the dependent series only.
- Drawdown depths are non-positive fractions: `-0.25` is a 25% loss.
- Rolling series are right-labeled — each output value carries the date of the
  last observation in its window.
- Benchmark inputs are assumed pre-aligned to the panel's date grid.
- Volatility and covariance use sample statistics (`n - 1` denominator).
  `skewness` and `kurtosis` are the bias-corrected G₁ / G₂ estimators
  (Joanes & Gill 1998), matching Excel `SKEW()` / `KURT()`; both are built on
  the same `n - 1` sample standard deviation and return `0.0` on
  zero-variance or too-short series.
- Compounding accumulates in log space with a Neumaier compensated
  accumulator for long-series stability.
- Degenerate cases return `0.0`, `NaN`, or `±∞` rather than panicking; the
  crate denies `unwrap`/`expect`/`panic`/`unreachable` at the lint level.
- This crate is `f64` throughout. It holds no `Money` and performs no FX; see
  [`INVARIANTS.md`](../../INVARIANTS.md) §1 for the workspace Decimal/f64 split.

### Input validation

`Performance::new` and `Performance::from_returns` reject empty inputs, ragged
matrices, column counts that disagree with `ticker_names`, an unknown
`benchmark_ticker`, and non-ascending dates. Per ticker, leading and trailing
`NaN` padding is allowed but the finite span must be contiguous; an interior
`NaN` or an active return `< -1.0` is an error. `multi_factor_greeks` also
rejects mismatched factor lengths, non-finite factors, non-positive
annualization factors, an unknown or non-finite `ReturnKind::Total` rate,
and singular or near-singular factor matrices. `correlation_matrix` and
`excess_returns` return `Err` on length mismatch, degenerate pairs, or
Higham failure.

## Serialization

`Performance` serializes authoritative inputs: the initial valuation and return
dates, compact return columns and spans, unique ticker names, benchmark, frequency,
and active window. Deserialization validates all invariants and rebuilds drawdown
caches. Unknown fields, including serialized caches, are rejected. `LookbackReturns`, `PeriodStats`, `DrawdownEpisode`, `BetaResult`,
`GreeksResult`, `MultiFactorResult`, `RollingGreeks`, and `DatedSeries` derive
`Serialize`/`Deserialize`. The `PeriodStats` fields that can legitimately be
`±∞` (`payoff_ratio`, `profit_factor`, `cpc_ratio`, `kelly_criterion`) go through
`finstack_quant_core::wire::non_finite_f64` so JSON round-trips exactly. See
[`docs/SERDE_STABILITY.md`](../../docs/SERDE_STABILITY.md).

## Bindings

- **Python** — flat surface under `finstack_quant.analytics`: `Performance`
  plus the result wrappers (`BetaResult`, `DatedSeries`, `DrawdownEpisode`,
  `GreeksResult`, `LookbackReturns`, `MultiFactorResult`, `PeriodStats`,
  `RollingGreeks`), `constrained_least_squares`, and the `AnalyticsError`
  exception. `Performance` gains DataFrame-first constructors
  (`Performance(prices_df, ...)`, `Performance.from_returns(df, ...)`) with the
  Rust-shaped array constructors available as `Performance.from_arrays` and
  `Performance.from_returns_arrays`, plus `*_to_dataframe` exits.
- **WASM** — `analytics.Performance` and `analytics.constrainedLeastSquares`
  only (see [`exports/analytics.js`](../../finstack-quant-wasm/exports/analytics.js)).
  WASM `Performance` methods return plain JS values instead of typed wrappers.
- Shared correlation helpers are bound under
  `finstack_quant.models.correlation` in both hosts, with
  `nearest_correlation_matrix` exposed as `nearest_correlation`.

The authoritative contract, including every known gap, is
[`parity_contract.toml`](../../finstack-quant-py/parity_contract.toml)
(`[crates.analytics]`, `[wasm_analytics_subset]`).

## Tests and benchmarks

| Path | Contents |
|------|----------|
| [`tests/performance_smoke.rs`](tests/performance_smoke.rs) | End-to-end `Performance` construction and metric coverage |
| [`tests/correctness_regressions.rs`](tests/correctness_regressions.rs) | Hand-checked metric values pinned to `1e-12` |
| [`tests/correlation_validator_agreement.rs`](tests/correlation_validator_agreement.rs) | `correlation::validate_correlation_matrix` agrees with core's `math::linalg` validator |
| [`benches/analytics_hot_paths.rs`](benches/analytics_hot_paths.rs) | Criterion benches for the hot scalar and rolling paths |
| [`benches/analytics_scaling.rs`](benches/analytics_scaling.rs) | Criterion size-sweep benches (ns per element vs series / matrix size) |

## References

Entries live in [`docs/REFERENCES.md`](../../docs/REFERENCES.md):

- Sharpe ratio — [`#sharpe1966`](../../docs/REFERENCES.md#sharpe1966)
- Expected shortfall — [`#artzner1999CoherentRisk`](../../docs/REFERENCES.md#artzner1999CoherentRisk)
- Active-portfolio context — [`#grinoldKahn1999ActivePortfolio`](../../docs/REFERENCES.md#grinoldKahn1999ActivePortfolio)

## Verification

```bash
mise run rust-lint-crate -- finstack-quant-analytics
mise run rust-test-crate -- finstack-quant-analytics
mise run rust-bench-crate -- finstack-quant-analytics analytics_hot_paths
mise run rust-bench-crate -- finstack-quant-analytics analytics_scaling
```

Workspace gates (`mise run rust-lint`, `mise run rust-test`, `mise run rust-doc`
— the last one runs doctests) are what CI enforces. The scoped tasks above
route through the project's supported lint, nextest, and Criterion tooling;
see [`CONTRIBUTING.md`](../../CONTRIBUTING.md).
