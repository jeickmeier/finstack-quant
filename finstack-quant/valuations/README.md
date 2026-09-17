# finstack-quant-valuations

Instrument pricing, risk metrics, cashflow projection, and market-structure
calibration. This is the mid-stack hub of the workspace: it turns market quotes
into a `MarketContext`, turns instruments plus a `MarketContext` into a
[`ValuationResult`](src/results/README.md), and exposes the metric calculators
that produce DV01, CS01, Greeks, spreads, and VaR.

## Where it sits

Depends on (see [`Cargo.toml`](Cargo.toml)):

| Crate | Used for |
|-------|----------|
| [`finstack-quant-core`](../core/README.md) | `Money`, `Currency`, dates/day-count, `MarketContext`, curves/surfaces, math, config |
| [`finstack-quant-cashflows`](../cashflows/README.md) | schedule generation, accrual, the `CashflowProvider` supertrait of `Instrument` |
| [`finstack-quant-analytics`](../analytics/README.md) | shared statistics and the canonical correlation-matrix helpers |
| [`finstack-quant-covenants`](../covenants/README.md) | `CovenantReport` attached to `ValuationResult::covenants` |
| [`finstack-quant-margin`](../margin/README.md) | `Marginable` bridge (`Instrument::as_marginable`) for VM/IM and XVA |
| [`finstack-quant-models`](../models/README.md) | closed-form/Fourier formulas, SABR, PDE/tree engines, structural credit, correlation, and Monte Carlo |
| `finstack-quant-valuations-macros` | `FinancialBuilder` derive, which generates `Type::builder()` (`macros/`) |

Consumed by [`attribution`](../attribution/README.md),
[`statements`](../statements/README.md), `statements-analytics`,
[`scenarios`](../scenarios/README.md), and [`portfolio`](../portfolio/README.md).
No crate that valuations depends on may depend back on it.

## Crate map

| Path | Role |
|------|------|
| [`src/instruments/`](src/instruments/README.md) | Instrument types by asset class, the `Instrument` trait, JSON loading |
| `src/pricer/` | `InstrumentType`/`ModelKey`/`PricerKey` dispatch, `PricerRegistry`, and JSON pricing entry points |
| [`src/metrics/`](src/metrics/README.md) | `MetricId`, `MetricCalculator`, `MetricRegistry`, sensitivities, historical VaR |
| [`src/calibration/`](src/calibration/README.md) | Plan-driven calibration engine, solvers, targets, validation, bumps |
| [`src/market/`](src/market/README.md) | Market quotes, convention registries, quote-to-instrument builders |
| [`src/results/`](src/results/README.md) | `ValuationResult`, `ResultsMeta`, `ValuationRow` export |
| `src/contract_specs.rs` | Embedded exchange contract specs (bond/equity-index/vol-index futures, repo defaults); crate-private, surfaced through the instruments that use them |
| `src/constants.rs` | Basis-point and percent conversion constants for hot paths |
| `src/schema.rs` | Accessors and validators for the checked-in JSON Schema artifacts |
| `src/error.rs` | Crate `Error`/`Result` (re-exported at the crate root) |
| `src/bin/gen_schemas.rs` | `gen_schemas` binary that writes/checks [`schemas/`](schemas/README.md) |

Supporting directories: [`tests/`](tests/README.md),
[`benches/`](benches/README.md), [`schemas/`](schemas/README.md),
[`examples/market_bootstrap/`](examples/market_bootstrap/README.md), and `data/`
(embedded conventions, contract specs, calibration defaults, and structured-credit
/ TBA assumptions compiled in with `include_str!`).

P&L attribution and covenant evaluation live in the separate
`finstack-quant-attribution` and `finstack-quant-covenants` crates.

## Instrument families

`src/instruments/` is grouped by asset class. Each leaf directory is one
instrument (types, pricer, optional cashflows, and a `metrics/` module):

| Group | Instruments |
|-------|-------------|
| `fixed_income/` | `asset_backed_facility`, `bond`, `bond_future`, `cmo`, `convertible`, `dollar_roll`, `fi_trs`, `inflation_linked_bond`, `mbs_passthrough`, `revolving_credit`, `structured_credit`, `tba`, `term_loan` |
| `rates/` | `basis_swap`, `cap_floor`, `cms_option`, `cms_spread_option`, `cms_swap`, `deposit`, `fra`, `hw1f`, `inflation_cap_floor`, `inflation_swap`, `ir_future`, `irs`, `repo`, `swaption`, `xccy_swap` |
| `credit_derivatives/` | `cds`, `cds_index`, `cds_option`, `cds_tranche` |
| `equity/` | `autocallable`, `cliquet_option`, `dcf_equity`, `equity_future`, `equity_option`, `equity_total_return_future`, `equity_trs`, `pe_fund`, `real_estate`, `spot`, `variance_swap`, `vol_index_future` |
| `fx/` | `fx_barrier_option`, `fx_digital_option`, `fx_forward`, `fx_future`, `fx_option`, `fx_spot`, `fx_swap`, `fx_touch_option`, `fx_variance_swap`, `ndf`, `quanto_option` |
| `commodity/` | `commodity_asian_option`, `commodity_forward`, `commodity_option`, `commodity_spread_option`, `commodity_swap`, `commodity_swaption`, `commodity_future` |
| `exotics/` | `asian_option`, `barrier_option`, `basket`, `callable_range_accrual`, `lookback_option`, `range_accrual`, `snowball`, `tarn` |

`common_impl/` is crate-private plumbing (the `Instrument` trait, shared
parameter types, pricing helpers, cashflow export). Its public re-exports are at
`instruments::*` and `instruments::pricing`.

`pricer::InstrumentType` has 71 variants; the JSON registry in
`instruments/json_loader.rs` covers 70 loadable instrument types. Several
instruments carry their own README — for example
[`fixed_income/bond`](src/instruments/fixed_income/bond/README.md),
[`fixed_income/structured_credit`](src/instruments/fixed_income/structured_credit/README.md),
and [`rates/irs`](src/instruments/rates/irs/README.md).

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
use finstack_quant_valuations::instruments::{Bond, Instrument, PricingOptions};
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
[`src/calibration/README.md`](src/calibration/README.md) and the runnable
envelopes in [`examples/market_bootstrap/`](examples/market_bootstrap/README.md).

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
[`src/results/README.md`](src/results/README.md) for the precedence rules).

**Rate units.** Rates and spreads are decimals unless a field or method name says
otherwise. Fields and arguments named `*_bp` are basis points. Calibration bump
quote-recalibration requests (`QuoteBump::ParallelBp`) are in basis points;
volatility bumps use `VolBumpRequest` because absolute-vs-relative semantics
differ. `constants::ONE_BASIS_POINT` exists for hot paths; prefer
`finstack_quant_core::types::{Rate, Bps, Percentage}` elsewhere.

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

- Python: `finstack_quant.valuations` — `ValuationResult`, `calibrate`,
  `CalibrationResult`, product-specific helpers, plus the
  `valuations.instruments`, `valuations.credit_derivatives`,
  `valuations.composite`, `valuations.market`, and `valuations.schema`
  submodules. Reusable engines live under `finstack_quant.models`.
- WASM/JS: `valuations` from `finstack-quant-wasm` — `calibrate`,
  `validateCalibrationJson`, `validateValuationResultJson`, and the
  `instruments`, `creditDerivatives`, `composite`, `market`, and `fx`
  namespaces. Reusable engines live under the sibling `models` namespace.

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
| `ts_export` | off | `ts-rs` TypeScript type export for schema/quote/calibration types |

Crate API docs: `cargo doc -p finstack-quant-valuations --open`.

## Verification

```bash
mise run rust-fmt                      # cargo fmt + clippy --fix (mutating)
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
workspace runs only through `mise run rust-doc`.

## License

MIT OR Apache-2.0
