# Real Estate

Single-asset real estate valuation (`RealEstateAsset`) under the two standard
appraisal methods, plus a levered deal wrapper
(`LeveredRealEstateEquity`) that composes the asset with a financing stack.

- **DCF** — discount an explicit annual NOI schedule (less CapEx) plus an
  exit-cap or explicit-sale terminal value.
- **Direct Cap** — capitalize a stabilized NOI at a cap rate.

`RealEstateAsset` is deliberately **unlevered**. Model leverage either by
valuing the debt separately and netting at the portfolio layer, or by using
`LeveredRealEstateEquity`.

## Public surface

Import path: `finstack_quant_valuations::instruments::equity::real_estate`
(`RealEstateAsset` and `LeveredRealEstateEquity` are also re-exported at
`finstack_quant_valuations::instruments`).

| Item | Purpose |
|------|---------|
| `RealEstateAsset` | The unlevered asset. `builder()`, `example()`. |
| `RealEstateValuationMethod` | `Dcf` or `DirectCap`. |
| `RealEstatePropertyType` | `Office`, `Multifamily`, `Retail`, `Industrial`, `Hospitality`, `MixedUse`, `Other`. |
| `RealEstateFinancing` | Supported financing union: bond, term loan, revolving credit, or repo. |
| `LeveredRealEstateEquity` | Asset + `Vec<RealEstateFinancing>`; its exit follows the asset `sale_date`. |

## Module layout

```
real_estate/
├── mod.rs            # re-exports
├── types.rs          # RealEstateAsset, valuation method, property type, example
├── pricer.rs         # DCF / direct-cap NPV, horizon, sale proceeds, rf-bump PV
├── levered.rs        # LeveredRealEstateEquity
├── levered_pricer.rs # equity/financing cashflows, exit date, payoff, PV
└── metrics/
    ├── cap_rates.rs     # going-in / exit cap rate
    ├── returns.rs       # unlevered IRR, multiple, cash-on-cash
    ├── levered.rs       # levered IRR, equity multiple, LTV, DSCR, debt payoff
    └── sensitivities.rs # cap-rate and discount-rate finite differences
```

Both instruments are registered with `register_generic!` under
`InstrumentType::RealEstateAsset` and `InstrumentType::LeveredRealEstateEquity`
in [`src/pricer/equity.rs`](../../../pricer/equity.rs). Pricing runs through
`Instrument::base_value`; `levered_pricer.rs` contains the shared PV and
cashflow helpers used by `LeveredRealEstateEquity`.

## Levered equity composition

`LeveredRealEstateEquity` holds `asset: RealEstateAsset`,
`financing: Vec<RealEstateFinancing>`; its exit is the asset horizon (`sale_date`, else the last NOI date).

- **Value convention**: `PV_equity = PV_asset − PV_financing`, with financing
  valued from the lender's perspective.
- **Return metrics** are computed off a simplified equity cashflow schedule with
  explicit sale proceeds and financing payoff at exit.
- **Financing stack**: any `InstrumentJson` works for PV netting. The
  cashflow-based leverage metrics (DSCR, debt payoff, levered IRR) additionally
  require financing instruments that produce cashflow schedules — `TermLoan`,
  `Bond`, `RevolvingCredit`, `Repo`.

## Conventions that bite

- **Annual NOI**: `noi_schedule` entries must be **annual** NOI amounts. Cap
  rates are quoted annually, so direct cap, exit-cap terminal value and the
  going-in cap rate all apply an annual rate to a single schedule entry. A
  sub-annual schedule silently understates those values.
- **No discount curve**: `RealEstateAsset` has **no** `discount_curve_id` field.
  DCF always discounts at the property's own `discount_rate` with annual
  discrete compounding, `PV = CF / (1 + r)^t` on the asset's `day_count`. The
  PV does not depend on the market context at all. Rate sensitivity comes from
  bumping the additive risk-free component *inside* that rate
  (`RfComponentDv01Calculator`), not from bumping a market curve.
- **Appraisal override**: when `appraisal_value` is set it short-circuits both
  methods and is returned directly (after a currency check).
- **Horizon**: with `sale_date` set, DCF valuation, the cashflow schedule and
  every return metric truncate at `sale_date` and realize terminal proceeds
  there. Otherwise the horizon is the last NOI date on or after `as_of`. Flows
  dated exactly on `as_of` are included undiscounted. Pricing requires `sale_date` on or after `as_of`; the
  instrument stores no valuation date of its own.
- **Terminal value**:
  - `sale_price` set → gross proceeds are `sale_price`, realized at `sale_date`
    (or the last NOI date). Cap-rate sensitivity is then zero by construction.
  - otherwise → exit cap: `TV = NOI_{N+1} / terminal_cap_rate`, with
    `NOI_{N+1} = NOI_N · (1 + terminal_growth_rate)`. Growth is validated into
    `[-100%, 20%]`.
  - `disposition_cost_decimal` (validated into `[0, 1)`) scales gross proceeds by
    `(1 − c)`; `disposition_costs` are dollar line items subtracted afterwards.
- **CapEx**: `capex_schedule` values are treated as **positive outflows** and
  valued as `NOI − CapEx`.
- **Direct cap**: uses `stabilized_noi` when set, otherwise the first future NOI
  entry. `cap_rate` must be positive.

## Fields you typically set

| Group | Fields |
|-------|--------|
| Core | `id`, `currency`, `valuation_method`, `noi_schedule`, `day_count` |
| DCF | `discount_rate` (required), `terminal_cap_rate`, `terminal_growth_rate` |
| Direct cap | `cap_rate` (required), `stabilized_noi` |
| Sale modeling | `sale_date`, `sale_price` |
| Transaction | `purchase_price`, `acquisition_costs` (line items), `disposition_cost_decimal` and/or `disposition_costs` |
| Cashflow realism | `capex_schedule` |
| Override | `appraisal_value` |

```rust
use finstack_quant_valuations::instruments::equity::real_estate::{
    RealEstateAsset, RealEstatePropertyType, RealEstateValuationMethod,
};
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::types::InstrumentId;
use time::macros::date;

let noi_schedule: Vec<(Date, f64)> = vec![
    (date!(2026 - 01 - 01), 100_000.0),
    (date!(2027 - 01 - 01), 100_000.0),
    (date!(2028 - 01 - 01), 100_000.0),
];

let asset = RealEstateAsset::builder()
    .id(InstrumentId::new("RE-OFFICE-DCF"))
    .currency(Currency::USD)
    .valuation_method(RealEstateValuationMethod::Dcf)
    .property_type_opt(Some(RealEstatePropertyType::Office))
    .noi_schedule(noi_schedule)
    .discount_rate_opt(Some(0.08))
    .terminal_cap_rate_opt(Some(0.055))
    .day_count(DayCount::Act365F)
    .attributes(Attributes::default())
    .build()?;
```

`RealEstateAsset::example()` builds the same shape with a 5-year flat NOI
schedule.

## Metrics

### `RealEstateAsset`

| `MetricId` | Meaning |
|-----------|---------|
| `custom("real_estate::discount_rate01")` | PV change per +1bp property discount-rate move |
| `custom("real_estate::going_in_cap_rate")` | First future NOI / `purchase_price`, falling back to the asset PV when `purchase_price` is unset |
| `custom("real_estate::exit_cap_rate")` | The configured `terminal_cap_rate`; errors when unset |
| `custom("real_estate::unlevered_irr")` | Requires `purchase_price` + `terminal_cap_rate` |
| `custom("real_estate::unlevered_multiple")` | Requires `purchase_price` + `terminal_cap_rate` |
| `custom("real_estate::unlevered_cash_on_cash_first")` | Requires `purchase_price` |
| `custom("real_estate::cap_rate01")` | PV change per +1bp cap-rate move: DirectCap bumps `cap_rate`, DCF bumps `terminal_cap_rate` |

### `LeveredRealEstateEquity`

| `MetricId` | Meaning |
|-----------|---------|
| `Dv01`, `BucketedDv01` | Parallel / key-rate market-curve risk of the financing stack |
| `custom("real_estate::levered_irr")` | IRR on the equity cashflow schedule |
| `custom("real_estate::equity_multiple")` | Equity multiple at exit |
| `custom("real_estate::ltv")`, `custom("real_estate::ltv_at_origination")` | Loan-to-value |
| `custom("real_estate::dscr_min")`, `custom("real_estate::dscr_min_interest_only")` | Minimum debt service coverage |
| `custom("real_estate::debt_payoff_at_exit")` | Financing payoff at the exit date |
| `custom("real_estate::discount_rate01")`, `custom("real_estate::cap_rate01")` | The asset sensitivities, applied through the wrapper (financing PV does not depend on either rate) |

**Sensitivity units**: `discount_rate01` and `cap_rate01` are currency per 1bp,
`(PV(r + h) − PV(r − h)) / (2h) × 1bp`, with `h` the resolved `rate_bump_bp`
(1bp by default). `cap_rate01` is not applicable (an error) when the value does
not depend on a cap rate: DirectCap without `cap_rate`, or DCF with an explicit
`sale_price` or no `terminal_cap_rate`.

**DSCR definition**: `dscr_min` and `dscr_min_interest_only` measure NOI over
**scheduled** debt service — cash interest and fees, plus scheduled
amortization for `dscr_min`. Balloon principal at maturity, prepayments and
revolver movements are excluded.

## Bindings

Reachable from Python and WASM through the JSON envelope
(`InstrumentJson::RealEstateAsset` / `InstrumentJson::LeveredRealEstateEquity`
inside `finstack_quant.instrument/1`):

- **Python**: `finstack_quant.valuations.instruments.price_instrument(...)`.
- **WASM**: `valuations.instruments.priceInstrument`.

There is no typed real-estate class in either binding.

## Verification

```bash
# Real estate pricing and metric tests
cargo nextest run -p finstack-quant-valuations --test valuations instruments::real_estate::

# Whole workspace (never `cargo test` — it runs doctests)
mise run rust-test

# Lints
mise run rust-lint
```

Tests live in
[`tests/instruments/equity/real_estate/`](../../../../tests/instruments/equity/real_estate/).

## See also

- [`../../fixed_income/term_loan/README.md`](../../fixed_income/term_loan/README.md) — the usual financing leg
- [`../../README.md`](../../README.md) — instrument module map and how to add one
- [`INVARIANTS.md`](../../../../../../INVARIANTS.md) — Decimal/f64, determinism and serde invariants
