# finstack-quant-cashflows

Cashflow schedule construction, accrual, and currency-preserving aggregation
for bonds, loans, swaps, and structured products. The crate turns contract
terms — notional, amortization, coupon legs, fees — into a
`CashFlowSchedule` of dated, currency-tagged `Money` flows. Pricing lives in
`finstack-quant-valuations`; this crate stops at the schedule. PSA/SDA
prepayment and default curves, and recovery specs, are calculators for
structured-credit valuations; `CashFlowBuilder.build()` does not consume them.

## Position in the stack

Depends only on `finstack-quant-core`. Consumed by `finstack-quant-valuations`
(instrument cashflows), `finstack-quant-statements` (corkscrew and debt
schedules), `finstack-quant-attribution` (carry and accrual inputs),
`finstack-quant-portfolio`, and both binding crates. Re-exported by the
umbrella crate as `finstack_quant::cashflows`.

## Modules

| Module | Contents |
|--------|----------|
| [`builder`](src/builder/mod.rs) | `CashFlowSchedule`, `CashFlowBuilder`, and every spec type (`ScheduleParams`, coupon/fee/amortization/credit specs) |
| [`accrual`](src/accrual.rs) | `accrued_interest_amount`, `AccrualIndex`, `AccrualConfig`, `AccrualMethod`, `ExCouponRule` |
| [`aggregation`](src/aggregation.rs) | Period bucketing, currency-checked totals, credit-adjusted PV, calendar-year ladders, the Act/365F WAL kernel |
| [`fixings`](src/fixings.rs) | `ProjectedFixing` raw index observations retained for realized-forward scenario rolls |
| [`schema`](src/schema.rs) | The JSON Schema registry (`ARTIFACTS`) and the `jsonschema` resources rendered from it |

The flow types themselves (`CashFlow`, `CFKind`, `CashFlowAccrual`) live in
`finstack_quant_core::cashflow`.

Crate-root exports also include `CashflowScheduleSource`, `CashflowProvider`,
`ScheduleBuildOpts`, the `schedule_from_*` adapters, the serde JSON surface
(`CashflowScheduleBuildSpec`, `build_cashflow_schedule_json`,
`validate_cashflow_schedule_json`, `dated_flows_json`). Dated amounts use
`(Date, Money)` tuples, collected as `Vec<(Date, Money)>` for schedules.

## Building a schedule

`CashFlowSchedule::builder()` returns a `CashFlowBuilder` whose setters take
`&mut self`; `build(curves)` compiles and projects the plan. Passing
`Some(&MarketContext)` supplies the curves floating legs need.

```rust
use finstack_quant_cashflows::builder::{
    CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use rust_decimal_macros::dec;
use time::Month;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let issue = Date::from_calendar_date(2025, Month::January, 15)?;
    let maturity = Date::from_calendar_date(2026, Month::January, 15)?;

    let schedule = CashFlowSchedule::builder()
        .principal(Money::new(1_000_000.0, Currency::USD)?, issue, maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.05),
            schedule: ScheduleParams::semiannual_30360(),
        })
        .build(None)?;

    assert!(!schedule.get_flows().is_empty());
    Ok(())
}
```

Builder surface, grouped:

| Group | Methods |
|-------|---------|
| Principal | `principal`, `principal_exchange`, `amortization`, `add_principal_event` |
| Coupons | `fixed_cf`, `floating_cf`, `step_up_cf`, `fixed_to_float`, `add_fixed_window`, `add_floating_window`, `float_margin_steps` |
| Fees | `fee` |
| Payment split | `add_payment_window`, `payment_split_program` |
| Terminal | `build(curves)` |

Setters record deferred configuration errors rather than panicking; `build`
returns the first one. `principal()` still defaults to issue funding plus
maturity redemption (`PrincipalExchange::InitialAndFinal`). Vanilla IRS and
basis swaps opt out with `PrincipalExchange::None` so coupon math keeps the
notional outstanding without exchanging it.

### Schedule conventions

`ScheduleParams` owns frequency, day count, calendar id, business-day
convention, stub kind, end-of-month rolling, payment lag, accrual-date
adjustment, and the roll rule (standard or CDS IMM grids). Named presets cover
the common desks:

`quarterly_act360`, `semiannual_30360`, `annual_actact`, `usd_sofr_swap`,
`usd_corporate_bond`, `usd_treasury`, `eur_estr_swap`, `eur_gov_bond`,
`gbp_sonia_swap`, `jpy_tona_swap`.

Accrual boundaries are left unadjusted by default (bond/ICMA convention); the
swap presets set `adjust_accrual_dates = true` so both accrual boundaries roll
with the business-day convention (ISDA 2006 §4.10). Only payment dates roll in
the bond case. Every coupon- or payment-window boundary (a fixed-to-float
switch, margin step or PIK toggle) cuts the horizon, and each piece gets its
own schedule: with the default front stub, coupon dates before an off-cycle
boundary roll back from it rather than from issue. Snap such dates onto the
accrual grid when the coupon dates must not move. `annual_actact()` is ISDA Act/Act, not ICMA (use
`eur_gov_bond` / `usd_treasury` for government bonds).

## Accrual

`accrued_interest_amount(&schedule, as_of, &AccrualConfig)` returns the accrued
amount for one date. For many dates on the same schedule, build an
`AccrualIndex` once and call `accrued_at` repeatedly — it precomputes coupon
periods and the outstanding-notional path.

`AccrualMethod::Linear` (the default) is the ICMA Rule 251.1 convention.
`AccrualMethod::Compounded` uses true exponential compounding
(`N × expm1(f × ln1p(r))`) and should not be cited as ICMA-style; it exists for
instruments that genuinely compound inside a coupon period. `ExCouponRule`
counts backward to the coupon record date. Settlement on that date retains
the coupon; settlement after it and before payment trades ex-dividend, with
accrued interest equal to the negative rebate of the remaining stub.

## Aggregation

All bucketing uses half-open intervals `[period.start, period.end)` — a flow
dated exactly on `period.end` belongs to the next period. Periods must be
sorted and non-overlapping; the public entry points validate this and return
`Error::Validation` otherwise.

| Function | Returns |
|----------|---------|
| `aggregate_by_period(flows, periods)` | `IndexMap<PeriodId, IndexMap<Currency, Money>>`; unsorted input is sorted first |
| `aggregate_cashflows_checked(flows, target)` | Single `Money`; every flow must already be in `target`, otherwise `Error::CurrencyMismatch` |
| `calendar_year_ladder(dates, kind_labels, amounts, pvs)` | `Vec<CalendarYearLadderRow>` keyed by calendar year |
| `credit_adjusted_cashflow_pvs(cashflows, discount_factors, hazard, recovery_rate, as_of)` | Native-currency row PVs as `Vec<f64>`, using the complete stream to identify funded principal |

Per-currency totals accumulate through a Neumaier-compensated `f64` sum over
`Money::amount()`; no per-flow ISO-4217 rounding is applied during
accumulation. PV aggregation assigns **zero PV** to flows dated on or before
the valuation base date, while plain amount aggregation still counts them.

`credit_adjusted_cashflow_pvs` discounts recovery at the scheduled repayment date.
Recovery begins only when principal is funded or PIK is capitalized; repayments
consume funding in FIFO order, separately per currency. Explicit principal
deltas determine recovery face amounts independently of cash settlement prices.
After the economic principal reduction has passed, an unpaid settlement is
valued as a cash receivable, consistently before and after schedule normalization.
Already-paid economic replay rows carry no recovery. A final PIK coupon paid
before capitalization carries no funded-principal recovery before payment.
`as_of` is the valuation date. Survival is conditional on it, flows on or
before it have zero PV, and discount factors are supplied per row.

## Conventions

- Amounts are `Money` with an explicit currency. Nothing in this crate performs
  FX; checked aggregation rejects mixed currencies rather than converting. See
  [`INVARIANTS.md`](../../INVARIANTS.md).
- Coupon rates are `rust_decimal::Decimal` decimals: `dec!(0.05)` is 5%. Fields
  named with a `_bp` suffix are basis points.
- `CFKind` is `#[non_exhaustive]`; downstream `match` needs a catch-all arm.
- `CFKind::is_cash_settlement` is the guard for "is this dated cash": `Pik` is a
  capitalization event and `DefaultedNotional` is a write-down, so both return
  `false` and are excluded from `dated_flows_json`.
- Prepayment/default rate conversions (`annual_to_monthly`, `monthly_to_annual`) live in the private `builder::credit_rates`
  module and are re-exported from `builder`. They validate their inputs and
  return `Result`.
- Errors are `finstack_quant_core::Error`; this crate defines no error type of
  its own.

## JSON and schemas

`CashflowScheduleBuildSpec` is the serde-first construction path used by the
bindings: `notional`, `issue_date`, `maturity`, plus optional `coupon_program`,
`payment_program`, `fees`, `principal_events`, and `principal_exchange`. It is
`#[serde(deny_unknown_fields)]`, and dates are ISO `YYYY-MM-DD` strings
(`time::Date` serde; schema type `finstack_quant_core::wire::DateWire`).

```rust
use finstack_quant_cashflows::{build_cashflow_schedule_json, dated_flows_json};

let spec_json = r#"{
  "notional": {
    "initial": { "amount": "1000000", "currency": "USD" },
    "amort": "none"
  },
  "issue_date": "2024-08-31",
  "maturity": "2025-08-31",
  "coupon_program": []
}"#;

let schedule_json = build_cashflow_schedule_json(spec_json, None).unwrap();
let flows_json = dated_flows_json(&schedule_json).unwrap();
assert!(!flows_json.is_empty());
```

`validate_cashflow_schedule_json` parses, validates, and re-serializes a
schedule payload; for an in-memory schedule call `CashFlowSchedule::validate`.

Fourteen schemas, one per entry of `schema::ARTIFACTS`, are checked in under
[`schemas/cashflow/1/`](schemas/cashflow/1): the inputs `accrual_config`,
`cashflow_schedule_build_spec` and `schedule_build_opts`; the outputs
`cashflow_schedule` and `period_aggregation`; and the components
`amortization_spec`, `coupon_specs`, `default_model_spec`, `fee_specs`,
`floating_coupon_spec`, `prepayment_model_spec`, `recovery_model_spec`,
`schedule_params` and `step_up_coupon_spec`. They are indexed by
[`schemas/index.json`](schemas/index.json). Regenerate with
`mise run rust-gen-schemas`; verify with `mise run rust-check-schemas`.
Validation renders the same registry, and the Rust serde types remain
authoritative for semantic validation.

See [`docs/SERDE_STABILITY.md`](../../docs/SERDE_STABILITY.md) for the wire
stability rules.

## Bindings

- **Python** — typed submodules under `finstack_quant.cashflows`:
  `builder`, `accrual`, `aggregation`, `fixings`, `schema`, plus the JSON
  bridge (`build_cashflow_schedule_json`, `validate_cashflow_schedule_json`,
  `dated_flows_json`) on the package root; the rate conversions live on
  `builder`. The `CashFlow`, `CFKind` and `CashFlowAccrual` row types are
  `finstack_quant.core.cashflow`.
- **WASM** — the `cashflows` namespace in
  [`exports/cashflows.js`](../../finstack-quant-wasm/exports/cashflows.js):
  `CashFlowSchedule`, `CashFlowBuilder` and `AccrualIndex` handles; spec,
  aggregation, fixing and rate-conversion functions over plain wire objects;
  and the JSON bridge (`buildCashflowScheduleJson`,
  `validateCashflowScheduleJson`, `datedFlowsJson`). The `CFKind` and
  `CashFlow` helpers (`cfKindParse`, `cashFlowValidate`, …) are in `core`.

The authoritative contract, including the deliberately Rust-only surface, is
[`parity_contract.toml`](../../finstack-quant-py/parity_contract.toml)
(`[crates.cashflows]`).

## Tests and benchmarks

| Path | Contents |
|------|----------|
| [`tests/main.rs`](tests/main.rs) | Single integration-test target; declares the `tests/cashflows/` tree (builder cases, worked examples, schema round-trips) |
| `tests/main.rs` module `coupon_spec_strictness` | `deny_unknown_fields` and spec-validation behavior |
| [`benches/cashflow_hot_paths.rs`](benches/cashflow_hot_paths.rs) | Build, accrual, and aggregation hot paths |
| [`benches/cashflow_scaling.rs`](benches/cashflow_scaling.rs) | Complexity scaling as schedule length grows |

## References

Entries live in [`docs/REFERENCES.md`](../../docs/REFERENCES.md):

- Day-count and schedule conventions —
  [`#isda-2006-definitions`](../../docs/REFERENCES.md#isda-2006-definitions)
- Bond-market accrued-interest conventions —
  [`#icma-rule-book`](../../docs/REFERENCES.md#icma-rule-book)

## Verification

```bash
mise run rust-lint-crate -- finstack-quant-cashflows
mise run rust-test-crate -- finstack-quant-cashflows
mise run rust-bench-crate -- finstack-quant-cashflows cashflow_hot_paths
```

Workspace gates (`mise run rust-lint`, `mise run rust-test`, `mise run rust-doc`
— the last one runs doctests) are what CI enforces; see
[`CONTRIBUTING.md`](../../CONTRIBUTING.md).
