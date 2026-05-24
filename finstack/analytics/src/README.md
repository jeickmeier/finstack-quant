# Analytics crate layout

User-facing usage lives in [`../README.md`](../README.md). This file maps
source modules and documents how to extend the crate.

## Module map

| Path | Role | Public surface |
|------|------|----------------|
| `lib.rs` | Crate root; re-exports | `Performance`, `LookbackReturns`, `PeriodStats`, `DrawdownEpisode`, `BetaResult`, `GreeksResult`, `RollingGreeks`, `MultiFactorResult`, `CagrBasis`, `AnnualizationConvention`, `DatedSeries`, `DatedSeries`, `DatedSeries`, `DatedSeries`, `beta`, `correlation` |
| `performance/` | Stateful orchestrator | `Performance`, `LookbackReturns` |
| `risk_metrics/` | Return-based, tail, rolling metrics | Config and rolling result types only |
| `benchmark.rs` | Benchmark-relative metrics | `beta`, result types |
| `correlation/` | Row-major correlation validation and repair | Shared Rust infrastructure for valuations and factor-model crates; Python/WASM bindings live under valuations |
| `drawdown.rs` | Drawdown paths and ratios | `DrawdownEpisode` |
| `returns.rs` | Simple/excess/compounded returns | crate-internal |
| `aggregation.rs` | Group-by-period stats | `PeriodStats` |
| `lookback.rs` | MTD/QTD/YTD/FYTD index ranges | crate-internal |

Performance building-block functions are `pub(crate)`. New analytics callers
should use `Performance`, not the free functions directly. The public
`correlation` module is the exception: it is shared infrastructure for crates
that need row-major correlation validation or repair without depending on
`finstack-valuations`.

## `Performance` shape

- Scalar methods return one value per ticker in `ticker_names` order.
- Per-ticker methods take `ticker_idx: usize` and return `Result` on invalid indices.
- `reset_date_range` narrows the active window for all subsequent metrics.
- `lookback_returns(ref_date, fiscal_config, calendar)` compounds returns over MTD/QTD/YTD/FYTD ranges; FYTD needs a holiday calendar for fiscal-year adjustment.
- `from_returns` accepts a pre-built simple-return matrix when prices are unavailable.

## Adding a scalar metric

1. Add a pure function in the closest module (`returns`, `risk_metrics/return_based`, `risk_metrics/tail_risk`, `drawdown`, `benchmark`, or `aggregation`).
2. Keep it `pub(crate)` unless it must be shared cross-crate (today only `beta`
   and `correlation` are public outside the `Performance` facade).
3. Add a thin `impl Performance` method in `performance/scalar.rs`, `performance/benchmark.rs`, or the relevant `performance/` submodule.
4. Add unit tests in the building-block module.
5. Wire Python/WASM bindings and `parity_contract.toml` when exposing the metric externally.

## Adding a rolling series

Rolling outputs use `DatedSeries` (or a type alias such as `DatedSeries`):

- Produce `n - window + 1` points when `n >= window > 0`; otherwise return empty vectors.
- Label each point with the last date in its window (right-labeled).
- Add a `Performance::rolling_*` wrapper that calls `ensure_ticker_idx` and passes `active_returns`, `active_dates`, and `self.ann()`.

## Adding a `PeriodStats` field

1. Add the field to `PeriodStats` in `aggregation.rs`.
2. Compute it inside `period_stats_from_grouped`.
3. Set a zero/default in the empty-input early return.
4. Extend the unit test.

## Adding a lookback horizon

1. Add a selector in `lookback.rs` returning `Range<usize>`.
2. Add a field to `LookbackReturns` and compute it in `Performance::lookback_returns`.

## Tests

Each new building block needs:

- A happy-path case with a known analytic value
- Empty-input behavior (`0.0` or empty output, no panic)
- At least one edge case (zero volatility, wipeout, etc.)

Doctests on `pub(crate)` helpers use `# Examples` blocks marked `ignore`.

## Numerical notes

- `comp_sum` / `comp_total` accumulate in log space with Neumaier compensation; growth factors below `1e-18` clamp so returns ≤ −100% stay finite.
- Sample statistics (`n - 1`) match Bloomberg/QuantLib-style conventions; population variance lives in `finstack_core::math::stats` when needed.
- Annualization factors come from `PeriodKind::annualization_factor()` (252 daily, 52 weekly, 12 monthly, 4 quarterly, 2 semi-annual, 1 annual).

## When to use other crates

| Need | Crate |
|------|-------|
| Analytics on `Vec<f64>` return panels | `finstack-analytics` |
| Python analytics on Polars frames | `finstack-py` (`Performance`) |
| Realized volatility from OHLC | `finstack_core::math::stats` |
| Instrument pricing or model greeks | `finstack-valuations` |
| Cashflow NPV or IRR | `finstack_core::cashflow` |
| Curves and discount factors | `finstack_core::market_data` |

Keep this crate instrument-agnostic: no Polars, curves, or instrument types.
