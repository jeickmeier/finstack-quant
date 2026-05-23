# finstack-analytics

Portfolio performance and risk analytics on numeric return series and
`finstack_core::dates::Date`, with no DataFrame or Polars dependency.

[`Performance`](src/performance/mod.rs) is the entry point. Construct it from
a price or return panel; scalars, drawdown statistics, rolling windows,
periodic returns, and benchmark-relative metrics are methods on that instance.

Per-domain modules (`returns`, `risk_metrics`, `drawdown`, `benchmark`,
`aggregation`, `lookback`) hold crate-internal building blocks that
`Performance` composes. Result and config types those modules define are
re-exported at the crate root because `Performance` returns them.

## Coverage

- **Returns**: simple returns, excess returns, compounded accumulation, geometric mean
- **Risk metrics**: CAGR, mean return, volatility, Sharpe, Sortino, downside deviation, Omega, gain-to-pain, modified Sharpe
- **Tail risk**: historical VaR, Expected Shortfall, parametric VaR, Cornish-Fisher VaR, skewness, kurtosis, tail ratios
- **Drawdown**: drawdown paths, episodes, max/mean drawdown, Ulcer Index, CDaR, Calmar, Martin, Sterling, Burke, Pain, recovery factor
- **Benchmark-relative**: tracking error, information ratio, beta (with SE and CI), alpha/beta/R² greeks, rolling greeks, up/down capture, batting average, Treynor, M-squared, multi-factor regression
- **Rolling series**: rolling Sharpe, Sortino, volatility, alpha/beta
- **Aggregation and lookbacks**: period compounding, win/loss streaks, Kelly criterion, MTD/QTD/YTD/FYTD range selection

## Dependencies

```toml
[dependencies]
finstack-analytics = { path = "../finstack/analytics" }
finstack-core = { path = "../finstack/core" }
```

Import path uses underscores even though the package name uses hyphens:

```rust
use finstack_analytics::Performance;
use finstack_core::dates::{Date, Month, PeriodKind};
```

## Quick start

```rust
use finstack_analytics::Performance;
use finstack_core::dates::{Date, Month, PeriodKind};

let dates: Vec<Date> = (1..=6)
    .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
    .collect();

let benchmark = vec![100.0, 101.0, 99.0, 102.0, 101.0, 103.0];
let portfolio = vec![100.0, 103.0, 100.0, 104.0, 102.0, 106.0];

let mut perf = Performance::new(
    dates,
    vec![benchmark, portfolio],
    vec!["SPY".to_string(), "ALPHA".to_string()],
    Some("SPY"),
    PeriodKind::Daily,
)
.expect("price matrix should be aligned and valid");

let sharpe = perf.sharpe(0.02);
let max_drawdown = perf.max_drawdown();
let beta = perf.beta();
let info_ratio = perf.information_ratio();
let rolling = perf.rolling_sharpe(1, 3, 0.02).expect("valid ticker index");

assert_eq!(sharpe.len(), 2);
assert_eq!(beta.len(), 2);
assert_eq!(rolling.values.len(), 3);

perf.reset_date_range(
    Date::from_calendar_date(2025, Month::January, 3).unwrap(),
    Date::from_calendar_date(2025, Month::January, 6).unwrap(),
);

let windowed_cagr = perf.cagr().expect("valid active window");
assert_eq!(windowed_cagr.len(), 2);
```

Use [`Performance::from_returns`](src/performance/mod.rs) when the panel is
already simple returns instead of prices.

## Public API

| Item | Module | Notes |
|------|--------|-------|
| `Performance`, `LookbackReturns` | `performance` | Entry point |
| `PeriodStats` | `aggregation` | Returned by `Performance::period_stats` |
| `DrawdownEpisode` | `drawdown` | Returned by `Performance::drawdown_details` |
| `BetaResult`, `GreeksResult`, `RollingGreeks`, `MultiFactorResult` | `benchmark` | Returned by benchmark methods on `Performance` |
| `CagrBasis`, `AnnualizationConvention` | `risk_metrics` | Configuration types |
| `DatedSeries`, `RollingSharpe`, `RollingSortino`, `RollingVolatility` | `risk_metrics` | Returned by `Performance::rolling_*` |
| `beta` | `benchmark` | Freestanding OLS beta; also used by `finstack-valuations` |
| `correlation` | `correlation` | Shared row-major correlation validation / repair infrastructure used by valuations and factor-model crates |

All other analytics building-block functions are crate-internal (`pub(crate)`).

## Conventions

- **Returns are simple decimal returns** unless a function explicitly says otherwise. `0.01` means `+1%`.
- **Drawdown depths are non-positive fractions**. A 25% drawdown is `-0.25`.
- **CDaR is non-negative** (absolute tail drawdown depth).
- **Benchmark alignment is the caller's responsibility**. `Performance` assumes the benchmark column aligns with the panel date grid.
- **Annualization comes from `PeriodKind`** when called through `Performance`.
- **Rolling series are right-labeled**: each output date is the last date in its window.
- **`Performance::new` expects price paths** and derives simple returns internally.
- **Per-ticker methods** (`rolling_*`, `drawdown_details`, `period_stats`, `multi_factor_greeks`) take a zero-based ticker index and return `Result` when the index is invalid.

## Numerical behavior

- Compounding uses compensated summation in log space for long-series stability.
- `Performance::new` and `Performance::from_returns` reject empty inputs, ragged matrices, unknown benchmark names, duplicate or non-monotonic dates, non-finite values, and interior invalid returns.
- Multi-factor regression rejects mismatched factor lengths, non-finite factors, non-positive annualization factors, and singular or near-singular factor matrices.
- Volatility, covariance, skewness, and kurtosis use sample statistics (`n - 1` denominator).
- Degenerate cases return `0.0`, `NaN`, or `±∞` rather than panicking.

## Serialization

`Performance`, `LookbackReturns`, `PeriodStats`, `DrawdownEpisode`, `BetaResult`, `GreeksResult`, `MultiFactorResult`, `RollingGreeks`, `RollingSharpe`, `RollingSortino`, and `RollingVolatility` derive `Serialize`/`Deserialize`.

## Bindings

- Python: flat performance surface under `finstack.analytics`; shared correlation utilities are bound under `finstack.valuations.correlation` for historical namespace compatibility. See `finstack-py/parity_contract.toml`.
- WASM: mirrors `Performance`; result types serialize to JS objects via `serde-wasm-bindgen`.

## References

Quantitative references: [`docs/REFERENCES.md`](../docs/REFERENCES.md).

## Verification

```bash
cargo fmt -p finstack-analytics
cargo clippy -p finstack-analytics --all-features -- -D warnings
cargo test -p finstack-analytics
cargo test -p finstack-analytics --doc
RUSTDOCFLAGS='-D warnings' cargo doc -p finstack-analytics --no-deps --all-features
```
