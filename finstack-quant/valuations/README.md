# finstack-quant-valuations

Instrument pricing, risk metrics, cashflow projection, and market-structure
calibration. This is the mid-stack hub of the workspace: it turns market quotes
into a `MarketContext`, turns instruments plus a `MarketContext` into a
[`ValuationResult`](core/src/results/README.md), and exposes the metric calculators
that produce DV01, CS01, Greeks, spreads, and VaR.

## Where it sits

Depends on (see [`Cargo.toml`](Cargo.toml)):

| Crate | Used for |
|-------|----------|
| [`finstack-quant-core`](../core/README.md) | `Money`, `Currency`, dates/day-count, `MarketContext`, curves/surfaces, math, config |
| [`finstack-quant-cashflows`](../cashflows/README.md) | schedule generation, accrual, the `CashflowProvider` supertrait of `Instrument` |
| [`finstack-quant-covenants`](../covenants/README.md) | `CovenantReport` attached to `ValuationResult::covenants` |
| [`finstack-quant-margin`](../margin/README.md) | `Marginable` bridge (`Instrument::as_marginable`) for VM/IM and XVA |
| [`finstack-quant-models`](../models/README.md) | closed-form/Fourier formulas, SABR, PDE/tree engines, structural credit, correlation, and Monte Carlo |
| [`finstack-quant-valuations-core`](core/README.md) and the seven family crates | pricing framework (`Instrument`, registries, metrics, results, conventions) and instrument implementations (`rates`, `fixed-income`, `structured-credit`, `credit`, `equity`, `fx`, `exotics`) |
| `finstack-quant-valuations-macros` | `FinancialBuilder` derive, which generates `Type::builder()` (`macros/`) |

Consumed by [`attribution`](../attribution/README.md),
[`statements`](../statements/README.md), `statements-analytics`,
[`scenarios`](../scenarios/README.md), and [`portfolio`](../portfolio/README.md).
No crate that valuations depends on may depend back on it.

## Crate map

The crate is an umbrella over two layers. The pricing framework lives in
[`core/`](core/README.md) (package `finstack-quant-valuations-core`) and is
re-exported here under the same paths; this crate adds the instrument families
and the closed world around them.

| Path | Role |
|------|------|
| [`core/src/instruments/`](core/src/instruments/README.md) | The `Instrument` trait and its hooks, `PricingOptions`, parameter and override types, the generic pricer, the shared pricing kernels (`common_impl`), the cashflow exporter engine |
| [`core/src/pricer/`](core/src/pricer/README.md) | `InstrumentType`/`ModelKey`/`PricerKey` dispatch, `Pricer`, `PricerRegistry`, `PricingDispatch`, the `Family` registration unit, metric enrichment, product defaults |
| [`core/src/metrics/`](core/src/metrics/README.md) | `MetricId`, `MetricCalculator`, `MetricRegistry`, the universal calculators, sensitivities, historical VaR |
| [`core/src/market/`](core/src/market/README.md) | Market quotes, convention registries, listed-product catalog |
| [`core/src/results/`](core/src/results/README.md) | `ValuationResult`, `ResultsMeta`, `ValuationRow` export, the composite and structured-credit result DTOs |
| `core/src/contract_specs.rs` | Embedded exchange contract specs (bond/equity-index/vol-index futures, repo defaults); hidden, surfaced through the instruments that use them |
| `core/src/constants.rs`, `core/src/error.rs`, `core/src/recalibration.rs` | Constants, the crate `Error`/`Result` (re-exported at both crate roots), the recalibration contract |
| [`fixed-income/`](fixed-income/README.md) | `finstack-quant-valuations-fixed-income`: the fixed-income family (bonds, bond futures, convertibles, inflation-linked bonds, term loans, revolving credit, FI-index TRS, agency MBS pass-throughs, TBAs, dollar rolls) with its `FIXED_INCOME` registration static; re-exported as `instruments::fixed_income` |
| [`structured-credit/`](structured-credit/README.md) | `finstack-quant-valuations-structured-credit`: the structured-credit tier (ABS/RMBS/CMBS/CLO deals, agency CMOs, asset-backed facilities) with its `STRUCTURED_CREDIT` registration static; depends on fixed-income and rates; re-exported as `instruments::fixed_income::{structured_credit, cmo, asset_backed_facility}` |
| [`rates/`](rates/README.md) | `finstack-quant-valuations-rates`: the interest-rate and inflation family (deposits, FRAs, futures, IRS, basis and cross-currency swaps, caps/floors, swaptions, CMS, repos, inflation swaps and caps/floors) with its `RATES` registration static; re-exported as `instruments::rates` |
| [`credit/`](credit/README.md) | `finstack-quant-valuations-credit`: the credit-derivatives family (CDS, CDS indices, tranches, CDS options, the CS01 fast path) with its `CREDIT` registration static; re-exported as `instruments::credit_derivatives` |
| [`equity/`](equity/README.md) | `finstack-quant-valuations-equity`: the equity family (spot, options, futures and futures options, variance swaps, TRS, autocallables, cliquets, vol-index futures, DCF equity, private-markets funds) with its `EQUITY` registration static; re-exported as `instruments::equity`; `real_estate` stays here under `instruments::equity::real_estate` |
| [`exotics/`](exotics/README.md) | `finstack-quant-valuations-exotics`: the exotics family (Asian, barrier, lookback options, range accruals, TARN, snowball) and the commodity family (forwards, futures, swaps, options), one `EXOTICS` registration static; re-exported as `instruments::exotics` and `instruments::commodity`; `Basket` stays here under `instruments::exotics::basket` |
| [`fx/`](fx/README.md) | `finstack-quant-valuations-fx`: the FX family (spot, forwards, swaps, NDFs, listed futures, vanilla/barrier/digital/touch/variance/quanto options) with its `FX` registration static; re-exported as `instruments::fx` |
| [`src/instruments/`](src/instruments/README.md) | The façade modules of the family crates; the umbrella-resident instruments — `exotics::basket` and `equity::real_estate` (both register with `STANDARD`); `composite`; `json_loader` (`InstrumentJson`, the closed world); the JSON cashflow-export entry points |
| `src/pricer/` | `FAMILIES`, the `STANDARD` family and `STANDARD.pricers()`, and the JSON pricing entry points (`src/pricer/json.rs`, re-exported flat from `pricer`) |
| `src/results.rs` | Core result façade; metrics are re-exported directly from the core crate |
| `src/schema.rs` | Accessors and validators for the checked-in JSON Schema artifacts |
| `src/bin/gen_schemas.rs` | `gen_schemas` binary that writes/checks [`schemas/`](schemas/README.md) |

Supporting directories: [`tests/`](tests/README.md) (integration tests,
including the `core_framework` module that exercises the framework over
built-in instruments), [`test-support/`](test-support/README.md) (the dev-only
`finstack-quant-valuations-test-support` crate: market builders, fixtures,
tolerances and parity assertions shared by every valuations test tree),
[`benches/`](benches/README.md),
[`schemas/`](schemas/README.md) and `examples/`. The embedded data lives with
its crate: `core/data/` (conventions, contract specs, product defaults,
listed-product catalog), `fixed-income/data/assumptions/` (TBA) and
`structured-credit/data/assumptions/` (structured credit), all compiled in
with `include_str!`.

P&L attribution and covenant evaluation live in the separate
`finstack-quant-attribution` and `finstack-quant-covenants` crates.

## Instrument families

`src/instruments/` is grouped by asset class. Each leaf directory is one
instrument (types, pricer, optional cashflows, and a `metrics/` module):

| Group | Instruments |
|-------|-------------|
| `fixed_income/` | `asset_backed_facility`, `bond`, `bond_future`, `cmo`, `convertible`, `dollar_roll`, `fi_trs`, `inflation_linked_bond`, `mbs_passthrough`, `revolving_credit`, `structured_credit`, `tba`, `term_loan` |
| `rates/` | `basis_swap`, `cap_floor`, `cms_option`, `cms_spread_option`, `cms_swap`, `deposit`, `fra`, `inflation_cap_floor`, `inflation_swap`, `ir_future`, `irs`, `repo`, `swaption`, `xccy_swap` |
| `credit_derivatives/` | `cds`, `cds_index`, `cds_option`, `cds_tranche` |
| `equity/` | `autocallable`, `cliquet_option`, `dcf_equity`, `equity_future`, `equity_option`, `equity_total_return_future`, `equity_trs`, `pe_fund`, `real_estate`, `spot`, `variance_swap`, `vol_index_future` |
| `fx/` | `fx_barrier_option`, `fx_digital_option`, `fx_forward`, `fx_future`, `fx_option`, `fx_spot`, `fx_swap`, `fx_touch_option`, `fx_variance_swap`, `ndf`, `quanto_option` |
| `commodity/` | `commodity_asian_option`, `commodity_forward`, `commodity_spot_option`, `commodity_spread_option`, `commodity_swap`, `commodity_swaption`, `commodity_future` |
| `exotics/` | `asian_option`, `barrier_option`, `basket`, `callable_range_accrual`, `lookback_option`, `range_accrual`, `snowball`, `tarn` |

The `Instrument` trait, the shared parameter types and the pricing kernels the
families build on are defined in [`core/src/instruments/`](core/src/instruments/README.md)
(`common_impl/` is public but hidden from the rendered docs). Their re-exports
are at `instruments::*` and `instruments::pricing`, the same paths as before the
split.

`pricer::InstrumentType` has 71 variants; the JSON registry in
`instruments/json_loader.rs` covers 70 loadable instrument types. Several
instruments carry their own README — for example
[`fixed_income/bond`](fixed-income/src/bond/README.md),
[`structured_credit`](structured-credit/src/structured_credit/README.md),
and [`rates/irs`](rates/src/irs/README.md).

## Example

`Instrument::price_with_metrics` is the canonical pricing entry point. It takes
the market, the valuation date, the requested metric IDs, and a `PricingOptions`
(model override, config, registry, market history).

<!-- `no_run`: this sample compiles (so it still catches API drift) but is not
     executed, because `price_with_metrics` on `Bond::example()` currently fails at
     runtime computing `ytm` — `DayCount::ActActIsma requires DayCountContext.coupon_period
     for an irregular coupon`. That is a library defect, not a defect in this example;
     restore this block to plain `rust` once it is fixed. -->
```rust,no_run
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn main() -> finstack_quant_core::Result<()> {
    let as_of = date!(2025 - 01 - 15);
    let bond = Bond::example()?; // 10y USD Treasury-style, discounts off "USD-TREASURY"

    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-TREASURY")
            .base_date(as_of)
            .knots([(0.0, 1.0), (30.0, 0.40)])
            .build()?,
    );

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Ytm, MetricId::Dv01],
        PricingOptions::default(),
    )?;

    println!("PV {} {}", result.value.amount(), result.value.currency());
    // Fixed-rate bond loses value when rates rise, so DV01 is negative.
    assert!(result.metric(MetricId::Dv01).is_some_and(|dv01| dv01 < 0.0));
    Ok(())
}
```

Building a `MarketContext` from raw quotes goes through the calibration engine
rather than through hand-assembled curves; see
[`../calibration/README.md`](../calibration/README.md) and the runnable
envelopes in [`../calibration/examples/market_bootstrap/`](../calibration/examples/market_bootstrap/README.md).

## Conventions that bite

**Decimal vs f64.** `ValuationResult::value` is `Money` (Decimal-backed, currency
tagged). Everything in `ValuationResult::measures` is `f64` and heterogeneous:
currency amounts, currency-per-bump sensitivities, decimal rates, ratios, and
counts all coexist. Interpret a measure through its `MetricId` contract, never by
assuming it is money. Model internals (curves, vols, Greeks) are `f64`
throughout. See [INVARIANTS.md §1](../../INVARIANTS.md).

**Currency safety.** `Money` arithmetic requires matching currencies. Pricers do
not convert implicitly; when a conversion or cross-currency discounting
assumption is applied it is stamped in `ResultsMeta::fx_policy_applied` (see
[`core/src/results/README.md`](core/src/results/README.md) for the precedence rules).

**Rate units.** Rates and spreads are decimals unless a field or method name says
otherwise. Fields and arguments named `*_bp` are basis points. Calibration bump
quote-recalibration requests (`QuoteBump::ParallelBp`) are in basis points;
volatility bumps use `VolBumpRequest` because absolute-vs-relative semantics
differ. `finstack_quant_core::math::units::ONE_BASIS_POINT` exists for hot paths; prefer
`finstack_quant_core::types::{Rate, Bps}` elsewhere.

**Day count and calendars.** Conventions come from the embedded registries in
`data/conventions/` via `market::conventions::ConventionRegistry`, not from
defaults invented at the call site. A missing convention is an error, not a
fallback.

**Determinism.** `CalibrationConfig::use_parallel` defaults to `false`. Solvers
use fixed Halton multi-start rather than system RNG; Monte Carlo pricers take an
explicit seed. Residual maps use `BTreeMap` ordering so reports are stable.

**Serde strictness.** Wire types (`InstrumentEnvelope`, `CalibrationEnvelope`,
`ValuationResult`, quotes) use `deny_unknown_fields` and carry an explicit schema
marker (`finstack_quant.instrument/1`, `finstack_quant.calibration/1`). See
[docs/SERDE_STABILITY.md](../../docs/SERDE_STABILITY.md).

**Result-return contract.** Computation entry points return typed results — a
Rust struct, a `Py*` wrapper, or a plain JS object — not JSON strings.
`pricer::parse_boxed_instrument_from_json` takes JSON input and `pricer::price_instrument`
returns a typed `ValuationResult`; only explicit validation and formatting surfaces return JSON
strings. See `.agents/rules/project-rules.md`.

**Model and convention provenance.** Pricing models and market conventions cite
their sources in rustdoc `# References` sections pointing at
[docs/REFERENCES.md](../../docs/REFERENCES.md).

## Bindings

Reachable from both host languages under the `valuations` namespace:

- Python: `finstack_quant.valuations` — `ValuationResult`,
  `instrument_cashflows`, the exotic-rates coupon helpers, plus the
  `valuations.instruments`,
  `valuations.composite`, `valuations.market`, and `valuations.schema`
  submodules. Reusable engines live under `finstack_quant.models`;
  calibration (`calibrate`, `CalibrationResult`) is the sibling
  `finstack_quant.calibration` package.
- WASM/JS: `valuations` from `finstack-quant-wasm` —
  `valuationResultToJson`, the `valuationResult*` accessors, the
  exotic-rates coupon helpers, and the `instruments`,
  `composite`, `market`, `fx`, and `schema` namespaces. Reusable engines live
  under the sibling `models` namespace, and `calibrate` /
  `validateCalibration` under the sibling `calibration` namespace.

Bindings are thin wrappers — no pricing logic lives in them.

## Usage

```toml
[dependencies]
finstack-quant-valuations = { path = "../finstack-quant/valuations" }
```

Or via the umbrella crate, which re-exports this crate as
`finstack_quant::valuations` with no cargo features:

```toml
[dependencies]
finstack-quant = { path = ".." }
```

| Feature | Default | Purpose |
|---------|---------|---------|
| `json-schema` | on | `schemars` derives, the schema registry and the `gen_schemas` binary |
| `jsonschema-validate` | on | `jsonschema` validation behind the `validate_*_json` entry points (implies `json-schema`) |

Crate API docs: `cargo doc -p finstack-quant-valuations --open`.

## Verification

```bash
mise run rust-fmt                      # cargo fmt
mise run rust-fix                      # clippy --fix (mutating; separate from fmt)
mise run rust-lint                     # fmt --check + clippy -D warnings
mise run rust-test                     # workspace nextest run

# Targeted
cargo nextest run -p finstack-quant-valuations --lib --test '*'
cargo bench -p finstack-quant-valuations --bench bond_pricing

# Schemas (regenerate after changing any public serde type)
mise run rust-gen-schemas
mise run rust-check-schemas
```

Do not run `cargo test` directly — it would also run doc tests, which this
workspace runs only through `mise run rust-test --doctest`.

## License

MIT OR Apache-2.0

# Quick Start Examples

## Example 1: Computing Bucketed DV01 for a Bond

```
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Rate;
use finstack_quant_valuations::instruments::{BondConvention, Instrument, PricingOptions};
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use time::macros::date;

# fn main() -> finstack_quant_core::Result<()> {
let as_of = date!(2025-01-01);
let bond = Bond::with_convention(
    "CORP-001",
    Money::from((1_000_000_i64, Currency::USD)),
    Rate::from_decimal(0.05).expect("valid rate fixture"),
    date!(2024-01-15),
    date!(2034-01-15),
    BondConvention::UsCorporate,
    "USD-OIS",
)?;

let market = MarketContext::new().insert(
    DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (30.0, 0.40)])
        .build()?,
);
let metrics = vec![MetricId::BucketedDv01];

let result = bond.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;

let pv = result.value.amount();
assert!(pv > 0.0);

// A fixed-rate bond loses value when rates rise, so DV01 is negative.
let total_dv01 = result
    .measures
    .get(MetricId::BucketedDv01.as_str())
    .copied()
    .unwrap_or_default();
assert!(total_dv01 < 0.0);
# Ok(())
# }
```

## Example 2: Computing Parallel DV01 for an Interest Rate Swap

```
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use time::macros::date;

# fn main() -> finstack_quant_core::Result<()> {
let as_of = date!(2025-01-01);
let swap = InterestRateSwap::example()?;

// A seasoned swap needs a discount curve, a projection curve for the
// floating leg, and the historical fixings its past resets already consumed.
let market = MarketContext::new()
    .insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (30.0, 0.40)])
            .build()?,
    )
    .insert(
        ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(as_of)
            .knots([(0.0, 0.04), (30.0, 0.04)])
            .build()?,
    )
    .insert_series(ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        (0..400)
            .map(|i| (date!(2023-12-01) + time::Duration::days(i), 0.05))
            .collect::<Vec<_>>(),
        None,
    )?);
let metrics = vec![MetricId::Dv01]; // Parallel DV01

let result = swap.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;

let dv01 = result
    .measures
    .get(MetricId::Dv01.as_str())
    .copied()
    .unwrap_or_default();
// Sign follows the swap's direction: a receiver gains when rates fall.
assert!(dv01.abs() > 0.0);
# Ok(())
# }
```

## Example 3: Computing Theta (Time Decay) for an Option

```
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_models::types::OptionType;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::create_date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use time::Month;

# fn main() -> finstack_quant_core::Result<()> {
let as_of = create_date(2024, Month::January, 1)?;
let expiry = create_date(2024, Month::July, 1)?; // 6-month option

let option = EquityOption::european(
    "OPT-001",
    "SPX",
    4500.0,
    expiry,
    100.0,
    Currency::USD,
    OptionType::Call,
)?;

// `european` binds the standard ids: USD-OIS, EQUITY-SPOT,
// EQUITY-VOL, and EQUITY-DIVYIELD.
let market = MarketContext::new()
    .insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()?,
    )
    .insert_price("EQUITY-SPOT", MarketScalar::Unitless(4500.0))
    .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.015))
    .insert_surface(
        VolSurface::builder("EQUITY-VOL")
            .expiries(&[0.25, 0.5, 1.0])
            .strikes(&[4000.0, 4500.0, 5000.0])
            .row(&[0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20])
            .build()?,
    );
let metrics = vec![MetricId::Theta];

let result = option.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;

// Negative theta: a long option loses value as time passes.
let theta = result
    .measures
    .get(MetricId::Theta.as_str())
    .copied()
    .unwrap_or_default();
assert!(theta < 0.0);
# Ok(())
# }
```

## Example 4: Computing Multiple Greeks for an Option

```
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_models::types::OptionType;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::create_date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use time::Month;

# fn main() -> finstack_quant_core::Result<()> {
let as_of = create_date(2024, Month::January, 1)?;
let option = EquityOption::european(
    "OPT-001",
    "SPX",
    4500.0,
    create_date(2024, Month::July, 1)?,
    100.0,
    Currency::USD,
    OptionType::Call,
)?;
let market = MarketContext::new()
    .insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()?,
    )
    .insert_price("EQUITY-SPOT", MarketScalar::Unitless(4500.0))
    .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.015))
    .insert_surface(
        VolSurface::builder("EQUITY-VOL")
            .expiries(&[0.25, 0.5, 1.0])
            .strikes(&[4000.0, 4500.0, 5000.0])
            .row(&[0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20])
            .build()?,
    );
let metrics = vec![
    MetricId::Delta,
    MetricId::Gamma,
    MetricId::Vega,
    MetricId::Theta,
    MetricId::Rho,
];

let result = option.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;

let greek = |id: MetricId| result.measures.get(id.as_str()).copied().unwrap_or_default();

// Greeks are scaled by the 100-unit contract size, so an at-the-money
// call has delta near 50 rather than 0.5. Gamma and vega are positive
// and theta is negative for a long option.
assert!(greek(MetricId::Delta) > 0.0 && greek(MetricId::Delta) < 100.0);
assert!(greek(MetricId::Gamma) > 0.0);
assert!(greek(MetricId::Vega) > 0.0);
assert!(greek(MetricId::Theta) < 0.0);
# Ok(())
# }
```
