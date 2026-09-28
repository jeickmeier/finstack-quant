# Commodity

Commodity derivatives for energy, metals and agriculture: forwards and futures,
fixed-for-floating swaps, and four option families (European/American vanillas,
Asians, spread options, swaptions). Six leaves, about 8.6k lines — the smallest
asset-class directory in the crate.

This file is an index — which leaf owns which product and which convention it
follows — plus the conventions shared across the family. No leaf here carries
its own README; per-instrument formulas and references are in the leaf `mod.rs`
rustdoc.

## Leaves

| Directory | Prices | Market convention / model |
|-----------|--------|---------------------------|
| `commodity_forward/` | Physically or cash-settled forwards and futures | Cost of carry `F(T) = S · exp((r − y + u)·T)` with convenience yield `y` and storage cost `u`; `SettlementType` = `Physical` / `Cash`; optional `CommodityConvention` preset supplies settlement lag and calendar |
| `commodity_swap/` | Fixed-for-floating price swaps | Floating leg is the business-day average of daily prices over each settlement period; NPV (payer of fixed) = floating PV − fixed PV |
| `commodity_option/` | European and American options on a commodity | `CommodityPricingModel::Black76` for Europeans; a binomial tree on spot (or futures-implied spot) for Americans; `SchwartzSmith(SchwartzSmithParams)` two-factor Monte Carlo under `ModelKey::MonteCarloSchwartzSmith` |
| `commodity_asian_option/` | Average-price options on commodity forwards | `AveragingMethod::Geometric` → Kemna–Vorst (1990), exact closed form; `AveragingMethod::Arithmetic` → Turnbull–Wakeman (1991), roughly 1% against Monte Carlo. Forward prices are read per fixing date from the price curve; default `ModelKey::AsianTurnbullWakeman` |
| `commodity_spread_option/` | Options on the spread between two correlated commodity prices (crack, spark, location) | Kirk (1995) approximation; payoff `max(S1 − S2 − K, 0)` for calls |
| `commodity_swaption/` | Options to enter a commodity swap at a fixed price | Black-76 on the **annuity-weighted** forward swap rate: `call = DF · annuity · [F·N(d₁) − K·N(d₂)]` |
| `averaging.rs` | Not an instrument. `pub(crate)` business-day averaging shared by the swap floating leg and the swaption forward swap rate | Windows are half-open `[start, end)` so a payment date is never observed twice; the final period sets `include_end = true` so swap maturity is observed exactly once |

`commodity_swap` and `commodity_swaption` must stay consistent through
`averaging.rs`: the swaption's forward swap rate is the average the swap would
actually pay. Changing the averaging window rule in one place without the other
breaks the swaption's at-the-money strike.

## Public surface

Import path: `finstack_quant_valuations::instruments::commodity::<leaf>`. All
six leaf directories are `pub mod`; `averaging` is `pub(crate) mod`. Every
headline type is also re-exported flat at
`finstack_quant_valuations::instruments`:

`CommodityForward`, `CommoditySwap`, `CommodityOption`,
`CommodityAsianOption`, `CommoditySpreadOption`, `CommoditySwaption`.

Reachable only under the family path:
`commodity_option::{CommodityPricingModel, CommodityMcParams,
CommodityOptionMcPricer}`,
`commodity_asian_option::CommodityAsianOptionAnalyticalPricer`, and
`commodity_forward::SettlementType` (a re-export of the shared
`instruments::SettlementType`, not a commodity-specific type).

`CommodityUnderlyingParams` — `commodity_type`, `ticker`, `unit`, `currency` —
is the shared underlying type at `instruments::CommodityUnderlyingParams`, and
`AveragingMethod` comes from `instruments::AveragingMethod` (defined in
`../exotics/asian_option/`), not from this directory.

Inside a leaf, `metrics/`, `pricer.rs`, `traits.rs` and `types.rs` are
`pub(crate)` or private; supported items surface through each leaf's `pub use`.

## Family conventions

- **Price curves, not rate curves.** Floating exposure comes from a commodity
  forward/price curve named by `forward_curve_id`; `discount_curve_id` is a
  normal rates discount curve in the settlement currency. Both must appear in
  `market_dependencies()` — `forward_dependency_completeness` enforces it.
- **Quantity × multiplier.** Notional is expressed as physical quantity times a
  contract multiplier, not as a `Money` face. PV is `Money` in the contract
  currency; unit prices are `f64`.
- **Averaging is shared and fail-loud.** Use
  `averaging::business_day_average_price`. Missing fixings or curve gaps
  propagate as errors and are never silently substituted with a neighbouring
  price.
- **Black-76 is the family default.** `commodity_option` (European),
  `commodity_swaption` and `commodity_spread_option` all register under
  `ModelKey::Black76`; `commodity_asian_option` defaults to
  `AsianTurnbullWakeman`. Anything else must be registered explicitly and
  selected through `PricingOptions::with_model`.
- **Geometric averaging needs positive prices.** `CommodityAsianOption`
  validates realized fixings before pricing and rejects non-positive values
  under `AveragingMethod::Geometric`.
- **Determinism.** The Schwartz–Smith Monte Carlo path takes an explicit seed
  (`CommodityMcParams::seed`) and must reproduce bit-identically; the registered
  default uses 100,000 paths × 252 steps.

## Registration

The general checklist is in [`../README.md`](../README.md#adding-an-instrument).
Unlike the other asset classes, this family's landing sites are all named after
it:

| Step | Where |
|------|-------|
| Pricer | `src/pricer/commodity.rs`. Five of the six leaves register through `register_generic!` over `Instrument::base_value`; `commodity_asian_option` and the Schwartz–Smith path on `commodity_option` register concrete pricers |
| Instrument key | `InstrumentType` variant in `src/pricer/keys.rs` |
| JSON tag | `with_instrument_json_registry!` in `../json_loader.rs`, category `"commodity"`. Current tags: `commodity_forward`, `commodity_swap`, `commodity_option`, `commodity_asian_option`, `commodity_swaption`, `commodity_spread_option` |
| Metrics | `register_<name>_metrics` in the leaf's `metrics/`, called from `register_commodity_instrument_metrics` in `src/metrics/core/standard_registry.rs` |
| Metric traits | Options that use the finite-difference greeks path implement `crate::metrics::HasExpiry` and `HasDayCount` — see `commodity_option/traits.rs` |
| Schemas | `mise run rust-gen-schemas`, verified by `mise run rust-check-schemas` |

## Tests and benches

Integration tests live in `../../../tests/instruments/`, compiled into the
single `instruments` target. `commodity/` covers forwards and swaps;
`commodity_option/`, `commodity_asian_option/`, `commodity_spread_option/` and
`commodity_swaption/` have their own directories. Everything is additionally
covered by the cross-cutting registry, serde and dependency-completeness
contract tests.

```bash
# whole target
cargo nextest run -p finstack-quant-valuations --test valuations instruments

# this family (filter is a substring match on the test name)
cargo nextest run -p finstack-quant-valuations --test valuations instruments::commodity

# colocated unit tests
cargo nextest run -p finstack-quant-valuations --lib commodity::

# whole workspace, what CI runs
mise run rust-test
```

Use `cargo nextest`, not `cargo test` — the latter also runs doc tests, which
this project keeps out of the normal loop. Lint with `mise run rust-lint`.

The Criterion bench is `commodity_pricing` in `../../../benches/`, covering
forward, swap, option, Asian, spread and swaption scenarios.

```bash
cargo bench -p finstack-quant-valuations --bench commodity_pricing
mise run rust-bench          # all workspace benches, short sampling
```

## Related

- [`../README.md`](../README.md) — `Instrument` trait, JSON contract, add-an-instrument checklist
- [`../common_impl/README.md`](../common_impl/README.md) — `CommodityUnderlyingParams`, `CommodityConvention`, two-clock plumbing, vol resolution
- [`../fx/README.md`](../fx/README.md) — the other Black-76-centric family
- [`../rates/README.md`](../rates/README.md) — swap-leg pricing kernels reused for schedule construction
- [`../../metrics/README.md`](../../metrics/README.md) — greeks and metric registration
- [`../../../tests/instruments/README.md`](../../../tests/instruments/README.md) — test layout and generated fixtures
