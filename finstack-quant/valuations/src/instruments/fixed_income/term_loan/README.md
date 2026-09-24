# Term Loan

Institutional term loans, including delayed-draw term loans (DDTL), PIK and
split coupons, covenant-driven events, original issue discount (OID) and
borrower call schedules.

`TermLoan` is the leveraged-loan sibling of [`Bond`](../bond/): a bond has a
single funded notional and a coupon; a term loan has a *commitment limit*, a
draw path, and fees on both drawn and undrawn balances.

## Public surface

Import path:
`finstack_quant_valuations::instruments::fixed_income::term_loan`
(`TermLoan` is also re-exported at `finstack_quant_valuations::instruments`).

| Item | Purpose |
|------|---------|
| `TermLoan` | The runtime instrument. Build with `TermLoan::builder()`; examples: `example`, `example_floating_with_ddtl`, `example_callable`. |
| `RateSpec` | `Fixed { rate_bp }` or `Floating(FloatingRateSpec)` (floors, caps, gearing, reset lag). |
| `AmortizationSpec` | `None` (bullet), `Linear { start, end }`, `PercentPerPeriod { bp }`, `PercentOfOriginalNotional { .. }`, `Custom(..)`. |
| `DdtlSpec`, `DrawEvent`, `CommitmentStepDown`, `CommitmentFeeBase` | Delayed-draw commitment, draw calendar, step-downs, commitment/usage fee bases. |
| `TermLoanCovenantEvents`, `MarginStepUp`, `PikToggle`, `CashSweepEvent` | Covenant-driven margin step-ups, PIK toggles, cash sweeps, draw-stop dates. |
| `OidPolicy`, `OidEirSpec` | OID withheld from proceeds vs tracked separately, plus EIR amortization settings. |
| `LoanCallSchedule`, `LoanCall`, `LoanCallType` | Borrower prepayment options: `Hard`, `Soft`, `MakeWhole { treasury_spread_bp }`. |
| `TermLoanOverrides` | Scenario-time covenant/schedule adjustments (extra step-ups, forced PIK toggles, extra sweeps, draw stop). |
| `TermLoanDiscountingPricer`, `TermLoanTreePricer` | The two registered pricers. |

Field-level documentation is in the rustdoc; this file covers the layout,
conventions and the parts that are easy to get wrong.

## Module layout

```
term_loan/
├── mod.rs         # re-exports + module-level overview
├── types.rs       # TermLoan, RateSpec, builder, examples, Instrument impl
├── spec.rs        # serde-stable term-sheet component types (DDTL, covenants, amortization, calls)
├── overrides.rs   # TermLoanOverrides
├── cashflows.rs   # full internal cashflow schedule (draws, interest, amort, PIK, fees)
├── pricing/
│   ├── discounting.rs  # TermLoanDiscountingPricer (ModelKey::Discounting)
│   └── tree_engine.rs  # TermLoanTreePricer (ModelKey::Tree, callable structures)
└── metrics/       # YTM, YTC, YTW, YT2Y/3Y/4Y, DM, all-in rate, OID EIR, OAS, CS01
```

## Construction

Build with `TermLoan::builder()`; `build()` validates the complete contract.
`TermLoan` is itself the serde-stable shape (the `finstack_quant.instrument/1`
envelope carries it directly).

```rust
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::{Attributes, InstrumentPricingOverrides};
use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use time::macros::date;

// Fixed-rate 5Y bullet with 2.5%-per-period amortization.
let loan = TermLoan::builder()
    .id(InstrumentId::new("TL-USD-5Y"))
    .currency(Currency::USD)
    .notional_limit(Money::new(10_000_000.0, Currency::USD)?)
    .issue_date(date!(2024 - 01 - 01))
    .maturity(date!(2029 - 01 - 01))
    .rate(RateSpec::Fixed { rate_bp: 600 })          // 6.00%
    .frequency(Tenor::quarterly())
    .day_count(DayCount::Act360)
    .business_day_convention(BusinessDayConvention::ModifiedFollowing)
    .stub(StubKind::None)
    .discount_curve_id(CurveId::new("USD-OIS"))
    .amortization(AmortizationSpec::PercentPerPeriod { bp: 250 })
    .coupon_type(CouponType::Cash)
    .instrument_pricing_overrides(InstrumentPricingOverrides::default())
    .attributes(Attributes::new())
    .build()?;
```

Notes that bite:

- Every `Option<T>` field has two setters: `.ddtl(spec)` for the inner value,
  `.ddtl_opt(Some(spec))` for the `Option` — the builder rejects a build that
  leaves a required field unset.
- Rate and fee inputs on the spec types are **integer basis points** (`rate_bp`,
  `usage_fee_bp`, `commitment_fee_bp`, `treasury_spread_bp`,
  `AmortizationSpec::PercentPerPeriod { bp }`).
- `notional_limit` is the *commitment*, not the funded balance. Without a
  `DdtlSpec` the loan funds the full commitment at issue.
- `AmortizationSpec::PercentPerPeriod { bp }` applies to the **declining**
  outstanding balance, so dollar amortization decays geometrically. It is not a
  flat percentage of original notional.
- Every `FloatingRateSpec` field is honored. The loan-level `calendar_id`
  drives the payment schedule and business-day adjustment; the spec's
  `fixing_calendar_id` (the loan calendar when unset) drives the reset lag and
  overnight observations. Overnight index floors and caps apply to each daily
  fixing unless `overnight_index_constraints` is `period`.
- `settlement_days` defaults to **2** as the quote anchor, not the LSTA
  par-trade convention (T+7, with delayed compensation beyond T+7). Set
  `settlement_days: 7` when marking to the LSTA par target:

  ```json
  { "settlement_days": 7 }
  ```

## Pricing

Both pricers are registered in [`src/pricer/fixed_income.rs`](../../../pricer/fixed_income.rs):

| Pricer | `ModelKey` | Use |
|--------|-----------|-----|
| `TermLoanDiscountingPricer` | `Discounting` | Deterministic cashflow projection and discounting (default). |
| `TermLoanTreePricer` | `Tree` | Callable structures — values the borrower's prepayment option and backs `Oas` / `EmbeddedOptionValue`. |

The discounting path generates the full internal schedule (DDTL draws,
interest, amortization, PIK capitalization, fees), filters to cash flows, and
values them on the settlement date on the loan's discount curve; the callable
tree uses the same settlement-date origin. The instrument PV is then carried to
`as_of`, like the bond and the revolver: `DF(as_of, settlement) × settlement
value` plus the flows paid between `as_of` and settlement. Quote-space measures
(discount margin, yields, OAS, the quoted CS01 target) stay on the settlement
date, where a quoted price applies.

**Call settlement**: hard/soft calls redeem dirty — clean strike times
pre-exercise outstanding plus cash accrued at the exercise step. Coupon-date
accrued is zero, so the coupon booked in the schedule is not double-counted.

**Floating + rates-only tree**: a positive `hw1f_sigma` without
`credit_curve_id` is rejected. An unset volatility uses a frozen projection;
supply a hazard curve so stochastic future resets can re-fix on the
rates-credit lattice.

**PIK treatment**: PIK interest capitalizes into outstanding principal and is
excluded from PV; it shows up in the final redemption amount. This matches
institutional practice — PIK grows the debt balance rather than producing a
cash flow.

**Sign convention**: lender view. Draws are outflows, interest / fees /
amortization / redemption are inflows.

## Metrics

Registered for `InstrumentType::TermLoan` in `metrics/mod.rs`:

| `MetricId` | Meaning |
|-----------|---------|
| `Ytm` | IRR to final maturity |
| `custom("ytc")` | Yield to first call |
| `Ytw` | Minimum yield across call dates and maturity |
| `custom("yt2y")`, `custom("yt3y")`, `custom("yt4y")` | IRR to fixed 2/3/4-year horizons |
| `DiscountMargin` | Additive spread for floating-rate loans that reproduces the price |
| `custom("all_in_rate")` | Effective borrower cost including fees |
| `custom("oid_eir_amortization")` | OID effective-interest-rate amortization schedule |
| `Oas`, `EmbeddedOptionValue` | Callable-tree metrics. OAS is quoted or solved; `EmbeddedOptionValue` is holder `P_callable − P_straight`. |
| `Dv01`, `BucketedDv01` | Parallel and key-rate risk on the selected model. Quoted callable loans freeze the solved OAS under curve bumps. |
| `Cs01`, `BucketedCs01` | Parallel and key-rate credit risk on the selected model. The credit tree re-bootstraps par spreads; discounting uses z-spread. |

`Theta` is registered universally by `metrics::standard_registry()`.

## Bindings

- **Python**: `finstack_quant.valuations.instruments.TermLoan` — a typed
  wrapper with `TermLoan.example()` and `to_json`/`from_json`. Generic pricing
  runs through `finstack_quant.valuations.instruments.price_instrument(...)`.
- **WASM**: `valuations.instruments.TermLoan`, plus
  `valuations.instruments.priceInstrument` and
  `valuations.instruments.instrumentCashflowsJson`.

Both use `InstrumentJson::TermLoan` inside the `finstack_quant.instrument/1`
envelope; unknown fields are rejected on deserialize.

## Limitations

- Deterministic cashflow projection: no stochastic prepayment or default model.
  Credit risk enters through the discount/credit curve, not through simulated
  default events.
- Covenant evaluation consumes the supplied `TermLoanCovenantEvents` /
  `TermLoanOverrides` inputs; it does not read live financial statements.
  Statement-driven covenant testing lives in `finstack-quant-covenants` and
  `finstack-quant-statements-analytics`.
- Fees beyond upfront / commitment / usage require extending `DdtlSpec`.
- Single currency per loan; multi-currency facilities are not modeled.
- Revolving utilization belongs to
  [`../revolving_credit/`](../revolving_credit/), not here.

## Verification

```bash
# Term-loan unit + integration tests
cargo nextest run -p finstack-quant-valuations --test instruments term_loan::

# Whole workspace (never `cargo test` — it runs doctests)
mise run rust-test

# Lints
mise run rust-lint
```

## See also

- [`../../README.md`](../../README.md) — instrument module map and how to add one
- [`../bond/README.md`](../bond/README.md) — shared cashflow-spec and quote conventions
- [`../revolving_credit/README.md`](../revolving_credit/README.md) — the drawn/undrawn sibling
- [`INVARIANTS.md`](../../../../../../INVARIANTS.md) — Decimal/f64, determinism and serde invariants
