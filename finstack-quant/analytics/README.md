# finstack-quant-analytics

Portfolio performance and risk analytics over numeric return series and actual
`finstack_quant_core::dates::Date` valuation grids. The canonical Rust API owns
all calculations; Python and WASM convert types and expose the same contracts.
No DataFrame or Polars dependency is required by the Rust crate.

## Entry point

Construct [`Performance`](src/performance/mod.rs) from a
[`PerformancePanel`](src/performance/panel.rs). Both panel kinds use the same
actual, strictly increasing valuation grid. For `L` dates, prices have `L`
levels and returns have `L-1` values covering the adjacent valuation intervals.
No initial date is inferred from observation frequency.

```rust
use finstack_quant_analytics::{PanelSeriesKind, Performance, PerformancePanel};
use finstack_quant_core::dates::{Date, PeriodKind};
use time::Month;

let dates: Vec<Date> = (1..=10)
    .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
    .collect();
let perf = Performance::new(&PerformancePanel {
    dates,
    series: vec![(0..10).map(|i| 100.0 + i as f64).collect()],
    kind: PanelSeriesKind::Prices,
    ticker_names: vec!["SPY".into()],
    benchmark_ticker: None,
    frequency: Some(PeriodKind::Daily),
})
.unwrap();

assert_eq!(perf.get_ticker_names(), &["SPY"]);
let sharpe = perf.sharpe(0.0).unwrap();
assert_eq!(sharpe.len(), 1);
```

Ticker labels must be unique and nonblank. Each column has a contiguous
observed span with only leading/trailing missing observations. Native edge
NaNs travel as JSON null; infinities and invalid values are rejected rather
than reclassified as missing. Total investment returns must be finite and
at least `-1`; exact `-1` or a final zero wealth level is a terminal wipeout,
with no subsequent observed returns. Inputs already expressed as excess
returns or factor spreads belong in the raw statistical APIs.

Frequency and benchmark are optional panel context. A calculation that uses
one requires it explicitly: no date-derived frequency or first-ticker
benchmark is selected. Unannualized statistics need no unused frequency;
absolute statistics need no unused benchmark. Callers own input adjustments,
matching valuation intervals, cash flows, currency, fee basis and return
provenance.

## Method families

| Family | Methods and source |
| --- | --- |
| Return and reward/risk scalars | [`scalar.rs`](src/performance/scalar.rs): `cagr`, `mean_return`, `geometric_mean`, `volatility`, `sharpe`, `sortino`, `downside_deviation`, `omega_ratio`, `gain_to_pain`, `modified_sharpe`. |
| Tail estimates and moments | [`scalar.rs`](src/performance/scalar.rs): historical `value_at_risk`/`expected_shortfall`, `tail_ratio`, explicit `parametric_var`/`cornish_fisher_var`, `skewness`, `kurtosis`. |
| Drawdowns and rewards | [`drawdown.rs`](src/drawdown.rs), [`scalar.rs`](src/performance/scalar.rs), [`aggregation.rs`](src/performance/aggregation.rs): path/episode measures, Calmar, Martin, Sterling, Burke, Pain, recovery and CDaR. |
| Benchmark-relative metrics | [`benchmark.rs`](src/performance/benchmark.rs): beta, Jensen/fit statistics, TE/IR, geometric capture, batting average, Treynor and absolute M-squared. |
| Rolling estimates | [`rolling.rs`](src/performance/rolling.rs) and [`benchmark.rs`](src/performance/benchmark.rs): compact right-labelled return-risk and alpha/beta results. |
| Grouping and lookbacks | [`aggregation.rs`](src/performance/aggregation.rs): explicit calendar/fiscal grouping, occupied-period statistics and dated lookback coverage. |
| Correlation | [`correlation.rs`](src/correlation.rs): common-sample Pearson matrix with `CorrelationResult` sample provenance, no automatic repair. |
| Multifactor regression | [`multi_factor.rs`](src/multi_factor.rs): raw `multi_factor_greeks` and a Total-return panel adapter. |
| Constrained regression | [`regression.rs`](src/regression.rs): normalized full-rank SVD and single-equality `constrained_least_squares`. |
| Summary | [`scalar.rs`](src/performance/scalar.rs): fixed 22-metric `TableEnvelope`, explicit cash/MAR/Omega/confidence and per-row sample metadata. |

The crate root re-exports `Performance`, its panel/result types,
`validate_summary`, and the standalone
`sharpe`, `sortino`, `volatility`, `max_drawdown`, `beta` and raw
`multi_factor_greeks` functions. `regression` and the feature-gated `schema`
registry are public modules. Domain implementation modules remain private.

## Numerical and financial conventions

- Returns and rates are decimal fractions: `0.01` means 1%.
- Annual-effective cash must be supplied wherever used, finite and greater
  than `-1`. Arithmetic cash excess uses `expm1(log1p(cash) / A)` per
  observation. Drawdown reward ratios subtract annual cash from dated CAGR.
- Frequency-based risk uses explicit positive finite periods per year or the
  panel's declared `PeriodKind`. It does not infer regular spacing or adjust
  for autocorrelation. MAR and Omega thresholds are explicit per-observation
  decimal hurdles, independent of annual cash.
- Dated CAGR uses each ticker's actual opening/closing valuation dates and
  defaults to Actual/365 Fixed. Explicit alternative clocks use the core
  `DayCountContext`; no observation-derived reference period is invented.
- Wealth compounds with compensated log accumulation. A terminal wipeout
  gives exact `-1` cumulative/geometric return. Empty reported histories are
  unavailable rather than observed zero performance.
- Volatility and tracking error use sample SD (`n-1`). Sharpe uses arithmetic
  excess return; downside deviation uses all observations in its denominator.
  Short history, measured zero risk, undefined `0/0` and signed infinity have
  distinct metric-specific meanings.
- Historical VaR is R-7 interpolation. ES averages exactly the fractional
  empirical lower-tail mass. Both are signed native-period return estimates,
  so positive lower-tail estimates are valid. Every tail request requires
  explicit finite `0.5 < confidence < 1`.
- Gaussian and CF use whole observation horizons in `1..=2^53-1`, omitted as
  one. Their additive model quantiles can fall below `-1`; they are not
  compounded investment observations. CF uses Type 2 shape coefficients and
  an analytic global ordering gate, with explicit errors and no fallback.
- Reported skewness/excess kurtosis use the Fisher-adjusted Type 2
  estimators, requiring three/four varying observations respectively.
  Modified Sharpe is an observation-period excess/CF-loss ratio at horizon
  one, with its own availability rule.
- Drawdown paths rebase at each selected actual opening. Recovery is exact
  return to the peak, durations count calendar days, and unavailable duration
  is nullable. Martin is cash-adjusted; modified Burke uses all episode
  depths and normalizes squared depths by return-observation count.
- Period grouping requires an explicit bucket frequency. Occupied partial
  periods are retained; absent buckets do not become synthetic observations.
  Fiscal context uses canonical core period boundaries.
- Raw regression inputs are prealigned finite statistical arrays. Standalone
  beta requires equal lengths. The raw multifactor API requires explicit
  total/excess interpretation; panel multifactor inputs are Total only and
  subtract cash from the response, with factors supplied already prepared.
- Correlation uses one all-ticker common sample. Constant/insufficient columns
  error the whole matrix. Singular PSD matrices are valid. The result records
  ticker order, shared observation count and actual return-ending bounds.
- Invalid requests and unresolved numerical failures return errors. Permitted
  unavailable estimates remain native NaN, and intentional zero-risk limits
  remain signed infinity. JSON results preserve them as `"nan"`, `"inf"`,
  `"-inf"`; malformed state and inappropriate nulls are rejected.

After restoring a generic `TableEnvelope`, call analytics
`validate_summary(&table)` before treating it as the canonical summary.
Generic table restoration validates structural fields; the analytics validator
also checks the required report metadata and metric/ticker contract. Summary
construction always runs this validation.

The [accepted methodology inventory](../../docs/decisions/financial/analytics/README.md)
contains the full formulas, numerical gates, availability tables and evidence
limits. [`INVARIANTS.md`](../../INVARIANTS.md) defines the workspace Decimal/f64
boundary. This crate holds no `Money` and performs no FX conversion.

## Position in the stack

`finstack-quant-core` is the only workspace dependency. `nalgebra` provides
normalized SVD for multifactor and constrained regression. The factor crate
uses raw beta for credit calibration; the umbrella re-exports analytics, and
Python/WASM expose its public surface.

## Verification

Use the repository tasks for scoped iteration:

```sh
mise run rust-lint-crate -- finstack-quant-analytics
mise run rust-test-crate -- finstack-quant-analytics
```

Run `mise run gen-write` after changing public wire types or bindings, then
complete the cross-language `mise run all` gate. Tests qualify the cases they
cover; they do not establish forecasting accuracy or universal numerical
certification.
