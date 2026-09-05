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
| [`aggregation`](src/aggregation.rs) | Period bucketing, currency-checked totals, credit-adjusted PV, calendar-year ladders |
| [`primitives`](src/lib.rs) | `CashFlow` / `CFKind` re-exported from core, plus `is_cash_settlement_kind` |
| [`traits`](src/traits.rs) | `CashflowScheduleSource`, `CashflowProvider`, `ScheduleBuildOpts`, and the `schedule_from_*` adapters |
| [`json`](src/json.rs) | Serde-first construction: `CashflowScheduleBuildSpec`, `build_cashflow_schedule_json`, `validate_cashflow_schedule*`, `dated_flows_json`, `accrued_interest` |
| [`schema`](src/schema.rs) | Published JSON Schema artifacts and `jsonschema` resources |

Crate-root type aliases: `DatedFlow = (Date, Money)` and
`DatedFlows = Vec<DatedFlow>`.

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
| Coupons | `fixed_cf`, `floating_cf`, `step_up_cf`, `fixed_to_float`, `add_fixed_window`, `add_floating_window`, `float_margin_stepup` |
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
the bond case. Each coupon-program window is an independent schedule with a
fresh stub at conversion; a fixed-to-float switch does not continue the
pre-switch roll. `annual_actact()` is ISDA Act/Act, not ICMA (use
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
models ex-dividend windows, where accrued interest becomes the negative rebate
of the remaining stub.

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
| `credit_adjusted_cashflow_pv(cashflow, discount_factor, survival_probability, recovery_rate, base)` | Checked survival-weighted PV of a single `CashFlow` as `f64`, under payment-date recovery semantics |

Per-currency totals accumulate through a Neumaier-compensated `f64` sum over
`Money::amount()`; no per-flow ISO-4217 rounding is applied during
accumulation. PV aggregation assigns **zero PV** to flows dated on or before
the valuation base date, while plain amount aggregation still counts them.

`credit_adjusted_cashflow_pv` fixes recovery at the scheduled payment date.
The integrated / default-midpoint variant, selected by `RecoveryTiming`, is
reached only through the `pub(crate)` period-PV kernel that valuations pricers
call; `RecoveryTiming` and `DateContext` are public types for that kernel's
signature but have no host binding.

## Conventions

- Amounts are `Money` with an explicit currency. Nothing in this crate performs
  FX; checked aggregation rejects mixed currencies rather than converting. See
  [`INVARIANTS.md`](../../INVARIANTS.md).
- Coupon rates are `rust_decimal::Decimal` decimals: `dec!(0.05)` is 5%. Fields
  named with a `_bp` suffix are basis points.
- `CFKind` is `#[non_exhaustive]`; downstream `match` needs a catch-all arm.
- `is_cash_settlement_kind` is the guard for "is this dated cash": `Pik` is a
  capitalization event and `DefaultedNotional` is a write-down, so both return
  `false` and are excluded from `dated_flows_json`.
- Prepayment/default rate conversions (`cpr_to_smm`, `smm_to_cpr`,
  `cdr_to_mdr`, `mdr_to_cdr`) live in the private `builder::credit_rates`
  module and are re-exported from `builder` and from the crate root, so hosts
  can bind them flat. They validate their inputs and return `Result`.
- Errors are `finstack_quant_core::Error`; this crate defines no error type of
  its own.

## JSON and schemas

`CashflowScheduleBuildSpec` is the serde-first construction path used by the
bindings: `notional`, `issue`, `maturity`, plus optional `coupon_program`,
`payment_program`, `fees`, `principal_events`, and `principal_exchange`. It is
`#[serde(deny_unknown_fields)]`, and dates go through
`finstack_quant_core::wire::date` (ISO `YYYY-MM-DD`).

```rust
use finstack_quant_cashflows::{build_cashflow_schedule_json, dated_flows_json};

let spec_json = r#"{
  "notional": {
    "initial": { "amount": "1000000", "currency": "USD" },
    "amort": "none"
  },
  "issue": "2024-08-31",
  "maturity": "2025-08-31",
  "coupon_program": []
}"#;

let schedule_json = build_cashflow_schedule_json(spec_json, None).unwrap();
let flows_json = dated_flows_json(&schedule_json).unwrap();
assert!(!flows_json.is_empty());
```

`validate_cashflow_schedule_json` parses, validates, and re-serializes a
schedule payload; `validate_cashflow_schedule` is the in-memory twin and
delegates to `CashFlowSchedule::validate`.

Seven component schemas are checked in under
[`schemas/cashflow/1/`](schemas/cashflow/1) — `amortization_spec`,
`coupon_specs`, `default_model_spec`, `fee_specs`, `prepayment_model_spec`,
`recovery_model_spec`, `schedule_params` — indexed by
[`schemas/index.json`](schemas/index.json). Regenerate with
`mise run rust-gen-schemas`; verify with `mise run rust-check-schemas`. The
Rust serde types remain authoritative for semantic validation.

See [`docs/SERDE_STABILITY.md`](../../docs/SERDE_STABILITY.md) for the wire
stability rules.

## Bindings

- **Python** — typed submodules under `finstack_quant.cashflows`:
  `primitives`, `builder`, `accrual`, `aggregation`, `schema`, plus the JSON
  bridge (`build_cashflow_schedule_json`, `validate_cashflow_schedule_json`,
  `dated_flows_json`, `accrued_interest`) and the four rate conversions flat on
  the package root.
- **WASM** — JSON-only surface in
  [`exports/cashflows.js`](../../finstack-quant-wasm/exports/cashflows.js):
  `accruedInterest`, `buildCashflowScheduleJson`, `validateCashflowScheduleJson`,
  `datedFlowsJson`, and `cprToSmm` / `smmToCpr` / `cdrToMdr` / `mdrToCdr`.

The authoritative contract, including the deliberately Rust-only surface, is
[`parity_contract.toml`](../../finstack-quant-py/parity_contract.toml)
(`[crates.cashflows]`).

## Tests and benchmarks

| Path | Contents |
|------|----------|
| [`tests/cashflows.rs`](tests/cashflows.rs) | Aggregator for the `tests/cashflows/` tree (builder cases, worked examples, schema round-trips) |
| [`tests/coupon_spec_strictness.rs`](tests/coupon_spec_strictness.rs) | `deny_unknown_fields` and spec-validation behavior |
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
cargo clippy -p finstack-quant-cashflows --lib --bins --tests --examples --all-features -- -D warnings
cargo nextest run -p finstack-quant-cashflows --lib --test '*'
cargo bench -p finstack-quant-cashflows --bench cashflow_hot_paths
```

Workspace gates (`mise run rust-lint`, `mise run rust-test`, `mise run rust-doc`
— the last one runs doctests) are what CI enforces. Use `cargo nextest`, not
`cargo test`, for crate-scoped runs; see
[`CONTRIBUTING.md`](../../CONTRIBUTING.md).
