# finstack-quant-valuations-equity

The equity family of the valuations stack: spot positions, listed futures and
futures options, vanilla and alternative-model options, variance swaps, total
return swaps and futures, autocallables, cliquets, volatility-index futures,
DCF equity and private-markets funds. It implements the framework in
[`finstack-quant-valuations-core`](../core/README.md) and is re-exported by
the umbrella crate [`finstack-quant-valuations`](../README.md) as
`finstack_quant_valuations::instruments::equity`, so every public path there
is unchanged. Depends on `finstack-quant-core`, `finstack-quant-cashflows`,
`finstack-quant-models`, `finstack-quant-margin` (the `Marginable` impl of
`EquityTotalReturnSwap`) and valuations-core only; no other instrument family.

Real estate is not here. `RealEstateFinancing` holds `Bond`, `TermLoan`,
`RevolvingCredit` and `Repo` legs from other families, so `RealEstateAsset`
and `LeveredRealEstateEquity` stay in the umbrella at
`instruments::equity::real_estate`, registered with `STANDARD`.

## Leaves

| Directory | Prices | Market convention / model |
|-----------|--------|---------------------------|
| `spot/` | Equity positions | Quantity × spot in the quote currency |
| `equity_option/` | European and American options | Black-Scholes-Merton; Heston, rough Heston, rough Bergomi, local-vol PDE and Monte Carlo alternatives selected through `ModelKey`; implied vol and the full Greek set |
| `equity_future/`, `equity_future_option/` | Listed futures and options on them | Cost-of-carry fair value, daily variation margin, listed lifecycle terms |
| `equity_total_return_future/` | Total-return futures | Financing-spread fair value |
| `equity_trs/` | Equity total return swaps | Shared TRS engine (`instruments::pricing::TrsEngine`), dividend settlement conventions, `Marginable` |
| `variance_swap/` | Variance and volatility swaps | Realised-variance accrual with `EquityPriceSeriesPolicy`, Carr-Madan replication fair strike |
| `autocallable/`, `cliquet_option/` | Path-dependent structured notes | Monte Carlo over `finstack_quant_models::monte_carlo::payoff::{autocallable, cliquet}` |
| `vol_index_future/`, `vol_index_future_option/` | VIX-style futures and options | Forward-variance fair value, Black-76 options |
| `dcf_equity/` | Discounted-cash-flow equity | Explicit forecasts plus `TerminalValueSpec` |
| `pe_fund/` | Private-markets funds | Deterministic commitment / drawdown / distribution schedules and fund-level metrics |
| `dividend01.rs`, `piecewise_gbm.rs` | Not instruments | Shared dividend-yield sensitivity and the forward-bootstrapped GBM used by the path-dependent pricers |
| `family.rs` | Not an instrument | `EQUITY`, the family's pricer and metric registrations |

```rust
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_equity::equity_option::EquityOption;

let option = EquityOption::example().unwrap();
assert!(!option.id().is_empty());
```

## Public surface

Import path: `finstack_quant_valuations_equity::<leaf>::<Type>` (or
`finstack_quant_valuations::instruments::equity::<leaf>::<Type>` through the
umbrella); the crate root exports only the `EQUITY` family static and
`EquityPathModel`. The headline types (`Equity`, `EquityOption`, `EquityFuture`,
`EquityFutureOption`, `EquityTotalReturnFuture`, `EquityTotalReturnSwap`,
`VarianceSwap`, `EquityPriceSeriesPolicy`, `Autocallable`, `FinalPayoffType`,
`CliquetOption`, `VolatilityIndexFuture`, `VolatilityIndexFutureOption`,
`DiscountedCashFlow`, `TerminalValueSpec`, `PrivateMarketsFund`) are also flat at
`finstack_quant_valuations::instruments`.

## Tests and benches

Integration tests that need only this crate, valuations-core and the
test-support crate live in `tests/`, compiled into the single `equity` target
(`mise run rust-test-crate -- finstack-quant-valuations-equity`):
`variance_swap/`, `autocallable/`, `private_market_fund/` and the type and
bucketed-DV01 tests of `equity/`. Tests that price through the standard
registries, build options through `tests/support/equity_fx_options.rs`, or
reach fixed-income instruments (`trs/`, the real-estate suite) stay in the
umbrella's `../tests/instruments/`. Benches stay in the umbrella.
