# Cashflows methodology audit

Audit date: **2026-11-05**. Source revision: **`7e82fd03ad98346727769e88dd5ac5d84b56e9fc`**.

[CASHFLOWS-001 — Contractual periods, calendars and day-count context](01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context)
is **Accepted on2026-11-08** after complete whole-record review.
[CASHFLOWS-002 — Coupon and payment program compilation](02-program-compilation.md#proposed-decisions-for-whole-record-review)
is **Proposed** for complete whole-record review; **14 other records remain Draft**.
Implementation status is **Observed within the defined file scope**; runtime
reconciliation and independent qualification remain pending. Production validation
is **Evidence inspected/located, not executed**; independent reference examples
are reported separately in record001. Existing code/tests establish observed
behavior, not financial adoption or universal correctness. Outstanding work is
tracked in the [cashflow follow-ups](../FOLLOW_UPS.md#cashflows).

The complete accepted001 package inherits accepted core date/grid/day-count rules,
removes broad presets and generic CDS overrides, and retains original nominal
phase/context, explicit calendar/reset/accrual roles and coincident payment
identity. It composes ICMA nominal slices, preserves full-context ACT365L and
explicit termination facts, guards immutable period queries and unifies the
single-period helper with canonical grid admission. The narrow core015 metadata endpoint clarification was accepted with this
package on2026-11-08 and applied to that existing record in place. Program/window/coupon/replay/reporting decisions retain their
separate owning records; this package changes no runtime or generated artifact.

The complete proposed002 package selects independent atomic coupon/payment
windows explicitly, including re-anchoring from redundant cuts and strict001
anchor/context admission. It replaces sticky errors with atomic fallible updates,
requires globally valid strict payment nesting, explicit settlement/exchange and
owner observation cutoff, and removes implicit Cash tails/curve-date inference.
Fixed period-start rates, margin FROM and payment UNTIL remain distinct disclosed
operators. Exact monetary state, genuine source errors and complete local plan
publication retain accepted authority; actual helpers/specs/hosts/schema/parity
require reconciliation. Broader amount/principal/fixing/lifecycle choices remain
with their own records. The complete package is proposed; runtime and independent
financial/host/geometry qualification remain pending.

## Scope and completeness

The assigned slice contains **35 Rust source files** under `finstack-quant/cashflows/src`: **30 algorithm-bearing files and 5 support files**, with no test-only or generated source files. Every file was read and classified exactly once in [coverage.json](coverage.json). Inline tests were located, selected formula/state examples inspected, and production functions following inline test blocks were included. Sixteen records cover the meaningful algorithm choices below; source-file count is not used as a substitute for algorithm coverage.

This scope includes schedule construction, coupon/program compilation, economic/cash timing, principal and amortization, term/overnight rates, replay, fees, credit assumption calculators, accrued interest, balance reconstruction, residual normalization, nominal/PV aggregation, WAL and fixing materialization. No inverse solver, stochastic RNG, regression fit, calibration optimizer or exercise decision is implemented in these owned files. The replay APIs support exercise-date consumers but do not choose exercise. Discount/hazard/forward curve kernels and holiday/day-count arithmetic are delegated to core; product-specific defaults/prepayment cashflow loops and swap-style overnight engines belong to their owning slices.

The [crate manifest](../../../../finstack-quant/cashflows/Cargo.toml) was also inspected: default features enable JSON Schema and schema-resource validation; there is no feature-selected alternative financial algorithm. Serde/spec field defaults and calendar IDs are material inputs and are documented in the records. `schema.rs`, schema CLI and module exports are support because they package schemas/examples or re-export algorithms; `specs/mod.rs` is algorithm-bearing because it implements probability-vector selection/validation. Generated JSON schemas are not counted as Rust sources. [tests/main.rs](../../../../finstack-quant/cashflows/tests/main.rs) registers the cited consolidated integration-test modules.

## Methodology inventory

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation |
|---|---|---|---|---|
| Contractual date grids for fixed/floating legs | Plain tenor, front/back stubs, EOM, IMM, quarterly CDS twentieth grid | ScheduleParams; CDS overrides tenor/stub | [CASHFLOWS-001](01-schedule-periods.md) | date_generation + periods; grid/context tests located |
| Accrual versus cash/reset dates | Optional adjusted accrual; payment BDC then business-day lag; reset subtracts business days | Bond presets unadjusted; swap presets adjusted; fixing calendar can differ | [001](01-schedule-periods.md) | Shared endpoints, zero-lag and collapsed-date branches inspected |
| Stub and partial day-count context | Raw-grid stub classification; ICMA quasi coupons; ACT/365L fixed denominator; terminal flag only at final endpoint | DayCount/frequency/roll/context | [001](01-schedule-periods.md) | period_accrual and context helpers; ICMA/leap tests located |
| Market schedule presets | USD generic/SOFR/corporate/Treasury, EUR €STR/government, GBP SONIA, JPY TONA | Explicit convenience constructors | [001](01-schedule-periods.md) | Exact preset table inspected |
| Coupon/payment windows | Sorted cut points; independent per-window grids; exactly one coupon; nested payment override | Fluent builder and JSON variants | [CASHFLOWS-002](02-program-compilation.md) | compiler + coupon_api; overlap/window tests located |
| Step-up versus floating margin programs | Fixed steps apply by raw period start; floating steps rebuild windows from each date | Explicit program variant | [002](02-program-compilation.md) | Step-up and margin-step branches inspected |
| Cash/PIK payment program boundaries | Supplied split runs until each date; cash after final date | payment_split_program or explicit windows | [002](02-program-compilation.md) | Step-date validation and selection inspected |
| JSON dispatch and defaults | Strict fields, canonical Rust builder, curve-base cutoff, initial/final exchange default | build_cashflow_schedule_json/build spec | [002](02-program-compilation.md) | json.rs inspected; strictness/roundtrip evidence located |
| Economic principal events | Explicit par delta independent of cash price/payment date; pre-issue history retained | add_principal_event | [CASHFLOWS-003](03-principal-and-emission-order.md) | Checked Decimal state; dated draw/repayment regressions located |
| Cashflow emission ordering | Coupons → PIK → events → amortization → fees → installment update → redemption | Fixed internal pipeline | [003](03-principal-and-emission-order.md) | Pipeline read; PIK/terminal-date tests located |
| Initial funding/final redemption | Negative issue funding and positive final balance; redemption follows terminal coupon payment | InitialAndFinal default; None explicit | [003](03-principal-and-emission-order.md) | Effective-date gates and no-coupon fallback inspected |
| Scheduled principal reduction | None, LinearTo, StepRemaining, original-percent, remaining-percent, CustomPrincipal | One AmortizationSpec | [CASHFLOWS-004](04-amortization.md) | Preparation, bounds and Decimal emission inspected |
| Windowed equal-principal amortization | Capture live opening balance; later draws over remaining dates; final full sweep | LinearBetween | [004](04-amortization.md) | Start/end alignment and later-draw branches; regressions located |
| Fixed coupon computation | Segmentwise `B×r×year_fraction`, checked Decimal products | Fixed/StepUp compiled leg | [CASHFLOWS-005](05-coupon-cash-and-pik.md) | Balance segments and terminal context inspected |
| Coupon split and capitalization | Cash/Pik/Split; negative cash allowed; fixed negative PIK rejected; floating signed segments with nonnegative net | CouponType plus payment override | [005](05-coupon-cash-and-pik.md) | Signed floating PIK formula test inspected |
| Term forward projection | Embedded-tenor forward at reset; convert curve to contract annualization | Simple/unset compounding; curve ID | [CASHFLOWS-006](06-floating-rate-projection.md) | rate_helpers and emission; reset/basis tests located |
| Floating rate transformation | Index bounds → spread/gearing → all-in bounds; spread-inclusive or affine gearing | Explicit flags; spread-inclusive default | [006](06-floating-rate-projection.md) | Checked params and infallible helpers inspected |
| Fixing cutoff and missing data | Exact past fixing, same-day preference/project, future fixings ignored; Error/SpreadOnly/FixedRate | build_as_of or curve base; Error default | [006](06-floating-rate-projection.md) | Supplied-series validation and full-coupon fallback inspected |
| Facility/tranche term RateSpec | Same projection kernel; fixing required on/before valuation; any explicit compounding rejected | RateSpec::try_rate_for_period | [006](06-floating-rate-projection.md) | Distinct branch read |
| Overnight observation dates | In-arrears, lookback, shifted observations/weights, cutoff, leading weekend carry | Explicit compounding method/calendar | [CASHFLOWS-007](07-overnight-compounding.md) | Full compiler read; carry formula tests inspected |
| Overnight numerical aggregation | Weighted arithmetic average or product of simple daily factors; index-basis conversion | SimpleAverage versus compounded variants | [007](07-overnight-compounding.md) | Positive factors, finite rates, basis checks inspected |
| Overnight floor/cap placement | Daily index constraints before product or period constraints after aggregation | Daily default; Period explicit | [007](07-overnight-compounding.md) | Daily/period examples inspected |
| Overnight changing principal | Difference cumulative prefix amounts; final period-bound adjustment spread over accrual time | Builder's fixed internal algorithm | [007](07-overnight-compounding.md) | Uniform-adjustment and signed-PIK test formulas inspected |
| Incremental overnight replay | Replace partial carry factor; cache same fixing; monotone transactional advance | Reusable accumulator | [007](07-overnight-compounding.md) | Weekend checkpoint formula inspected; atomicity tests located |
| Floating coupon state and settlement | Fix rate separately from capturing notional; require complete replay; split finite amount | CompiledFloatingCoupon | [CASHFLOWS-008](08-floating-replay.md) | State transition/domain tests located |
| Replay accrued amount | Stored notional × elapsed fraction × replayed/fixed all-in rate | Explicit compiled-coupon method | [008](08-floating-replay.md) | Exact replay-through guard and ACT/365L logic inspected |
| Fixed/periodic fee amount | Raw dated fixed amount; annual bp on drawn or floored undrawn balance | FeeSpec; PointInTime default/TWA explicit | [CASHFLOWS-009](09-fees.md) | Contract-day-count fee formula inspected |
| Fee tier selection | Inclusive greatest threshold; whole-input rate; zero below first | evaluate_fee_tiers | [009](09-fees.md) | Ordered-threshold tests located |
| CPR/CDR ↔ monthly mortality | Survival-equivalent power via expm1/log; checked and explicit clamped APIs | Shared helper selected by caller | [CASHFLOWS-010](10-prepayment-and-mortality.md) | Boundary/roundtrip tests located; interleaved helper inspected |
| Prepayment assumption curves | Constant CPR, PSA ramp, CMBS inclusive lockout, ABS depletion, annual-CPR vector | Explicit PrepaymentModelSpec | [010](10-prepayment-and-mortality.md) | Piecewise formulas and indexing read; PSA examples inspected |
| Conditional default curves | Constant/vector CDR and SDA ramp/plateau/decline | Explicit DefaultModelSpec | [CASHFLOWS-011](11-default-and-recovery-assumptions.md) | SDA landmark examples inspected |
| Original-balance default targets | Cumulative loss/severity or annual timing shares; divide increment by surviving balance | CumulativeLoss/Timing; optional actual survival | [011](11-default-and-recovery-assumptions.md) | Par-build test inspected; whole-curve validation located |
| Recovery-rate assumptions | Constant rate or one-minus-severity vector; tail hold; direct clamp/fallback | RecoveryModelSpec; lag stored for consumer | [011](11-default-and-recovery-assumptions.md) | Evaluation and validation branches read |
| Schedule canonicalization/validation | Total row order; separate structural/economic validation; principal tolerances | Constructors, serde, explicit validate | [CASHFLOWS-012](12-schedule-lifecycle.md) | Full lifecycle and mutation paths inspected |
| Outstanding principal replay | Economic-date order; explicit delta overrides inferred kind; skip initial funding once | Schedule balance views | [012](12-schedule-lifecycle.md) | Date/delta/rounding tests located |
| Public remaining-cashflow view | Rebase movements before as_of; preserve pending cash or economic-only rows; include on-date cash | Non-overridable provider normalization | [012](12-schedule-lifecycle.md) | Normalization cases and registration inspected |
| Schedule merge/scale | Union rows/calendars; conservative representation precedence; agreed metadata only; scale cash/deltas | Explicit utility calls | [012](12-schedule-lifecycle.md) | All branches read; mixed-currency/canonical tests located |
| Schedule accrued interest | Metadata buckets; actual balance segments; linear or fractional compounded growth | Linear/include-PIK defaults; explicit frequency | [CASHFLOWS-013](13-accrued-interest.md) | Context, bucket, finite-domain branches read |
| Ex-coupon entitlement | Calendar/business-day subtraction; strict date-after-ex rule; rebate every coupon segment | Optional ExCouponRule, max 366 days | [013](13-accrued-interest.md) | Boundary and multisegment tests located |
| Nominal period aggregation | Half-open windows, sparse buckets, currency separation, Neumaier sums | Explicit periods; no FX | [CASHFLOWS-014](14-aggregation-and-life.md) | Boundary/order/currency branches inspected |
| Calendar-year cash/PV ladder | Group all rows by cash year; principal/nonprincipal classification; caller PVs | Free arrays or single-currency schedule wrapper | [014](14-aggregation-and-life.md) | Kind/length/finite checks inspected |
| Weighted average life | Future positive principal weighted ACT/365F; schedule uses par delta/economic date | WAL helper/schedule method | [014](14-aggregation-and-life.md) | Par/cash-date cases located |
| Survival-adjusted cash PV | Relative discount and conditional survival; on/past-date/noncash zero; post-default cash unweighted | Optional hazard; curve IDs resolved by caller | [CASHFLOWS-015](15-credit-adjusted-pv.md) | Full row/period formula inspected |
| Estimated principal recovery PV | FIFO funded lots; default mass over funded interval; recovery at repayment date | Positive optional recovery; hazard required | [015](15-credit-adjusted-pv.md) | Independent interval formula inspected |
| Realized-forward fixing roll | Existing exact fixings win; inclusive crossed endpoints; raw projections; conflict tolerance | materialize_fixings | [CASHFLOWS-016](16-fixing-materialization.md) | Normalization and immutable merge read; roll tests located |

## Evidence limits

No runtime sources, tests, fixtures or selected financial methods were changed. No Rust build, test or clippy task was run for this documentation audit. Local documentation checks verify source coverage, record references and links; they do not establish mathematical accuracy. Evidence distinguishes direct formula checks, state/roundtrip/invariant tests and comparisons to the same production helper. Bibliographic anchors were checked for existence; linked standards were not independently revalidated clause by clause.

## Follow-up tracking

The [cashflow follow-ups](../FOLLOW_UPS.md#cashflows) own adoption questions, implementation discrepancies and validation work. Individual records retain the observed algorithms, evidence and scope limitations.
