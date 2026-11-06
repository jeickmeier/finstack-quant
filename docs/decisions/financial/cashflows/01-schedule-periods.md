# CASHFLOWS-001 — Contractual periods, calendars and day-count context

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped to the owning files below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and scope

Cashflow dates are compiled from contractual anchors using the core schedule builder, then enriched with economic accrual dates, cash payment dates and optional reset dates. Plain month/day/week grids, third-Wednesday IMM and CDS twentieth grids are supported. This record owns the cashflows-layer transformations; the underlying date arithmetic and holiday definitions belong to core. Source comments motivate preserving contractual reference periods and separating cash settlement from interest accrual. Adoption of the presets as product defaults remains pending.

## Algorithm and conventions

1. Require start before end and nonnegative payment/reset business-day lags. Resolve calendars strictly; unknown identifiers fail. `CdsImm` overrides requested frequency with quarterly and requested stub with short-back. Other rolls preserve the requested tenor and stub.
2. Delegate anchor generation to `Schedule::builder`. Front stubs anchor backward from the end; back stubs anchor forward from the start. Apply EOM to the nominal grid. Reject EOM with IMM/CDS IMM. Reject ACT/ACT ICMA with third-Wednesday IMM; ICMA EOM requires a month-end regular-grid anchor.
3. Business-day-adjust each shared anchor once. With `adjust_accrual_dates=false`, keep raw accrual endpoints; with `true`, use adjusted endpoints and reject collapsed/reversed intervals. Cash payment is the adjusted end plus `payment_lag_days` business days. Lags apply after business-day adjustment. Distinct periods may not have the same payment date, including across program windows.
4. Preserve raw endpoints as `unadjusted_start/end`. Stub classification uses those raw endpoints, not holiday-shifted durations: plain periods accept either a forward or backward tenor step; IMM requires valid IMM dates separated by the tenor's months; CDS requires consecutive CDS dates.
5. Reset with zero lag is exactly accrual start, without an additional adjustment. Positive lag subtracts business days from accrual start using the fixing calendar, then applies the specified BDC. The standalone period API uses its schedule calendar for resets; floating emission can use a distinct fixing calendar.
6. Build day-count context with frequency, calendar, reference coupon period and termination flag. ICMA regular periods use their raw period; back stubs use the next nominal coupon, front stubs the preceding one; CDS uses the surrounding quarterly twentieth dates. ACT/365L uses the full actual coupon interval to select its denominator. Partial accrual retains that full-period context. ThirtyE/360 ISDA's termination flag applies only when the subinterval reaches the actual terminal accrual endpoint.

Rates and year fractions are decimal; payment and reset lags count business days. `period_accrual` rejects subintervals outside their contractual period. `prorated_actual_accrual` computes `full_yf × subinterval_actual_days / full_period_actual_days` and validates finite nonnegative fractions.

## Selection and controls

`build_periods`, `build_single_period` and the builder compiler share these rules. Schedule constructors default to short-front, no EOM and no payment lag. The bond presets leave accrual dates unadjusted; swap presets adjust them. The inspected preset table is:

| Preset | Tenor / day count | BDC / calendar | Payment lag |
|---|---|---|---:|
| Generic quarterly | 3M / ACT/360 | Modified Following / weekends_only | 0 |
| Generic semiannual | 6M / 30/360 | Modified Following / weekends_only | 0 |
| Generic annual | 1Y / ACT/ACT | Following / weekends_only | 0 |
| USD SOFR swap | 3M / ACT/360 | Modified Following / usny | 2 |
| USD corporate bond | 6M / 30/360 | Following / usny | 0 |
| USD Treasury | 6M / ACT/ACT ICMA | Following / usny | 0 |
| EUR €STR swap | 1Y / ACT/360 | Modified Following / target2 | 2 |
| EUR government bond | 1Y / ACT/ACT ICMA | Following / target2 | 0 |
| GBP SONIA swap | 1Y / ACT/365F | Modified Following / gblo | 0 |
| JPY TONA swap | 1Y / ACT/365F | Modified Following / jpto | 2 |

## Limitations and alternatives

`ScheduleParams::validate` checks calendar and payment lag only; full grid/context validation happens during compilation. `build_single_period` enriches the supplied single interval rather than constructing a multi-period IMM/CDS schedule. Negative payment lags and coincident adjusted payments are rejected rather than combined. Possible alternatives, not implemented here, include merging coincident payments and preserving one global grid through program switches; the latter is explicitly different from [CASHFLOWS-002](02-program-compilation.md).

## Evidence

- Owning algorithms: [date_generation.rs](../../../../finstack-quant/cashflows/src/builder/date_generation.rs), [periods.rs](../../../../finstack-quant/cashflows/src/builder/periods.rs), [schedule presets](../../../../finstack-quant/cashflows/src/builder/specs/schedule.rs).
- Located regression/invariant tests: `adjusted_accrual_boundaries_are_shared_between_consecutive_periods`, `payment_lag_applies_after_business_day_adjustment`, `explicit_and_generated_periods_reject_collapsed_accrual_bounds` in the date-generation source; `short_front_icma_stub_uses_ending_regular_coupon` in periods; [roll-rule integration tests](../../../../finstack-quant/cashflows/tests/cashflows/builder/roll_rule.rs).
- Repository reference anchors: [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions), [ICMA Rule Book](../../../REFERENCES.md#icma-rule-book). These are bibliographic pointers, not a checked clause-to-implementation conformance certificate.
- No market-vendor comparison was executed. The source-level branch inspection is complete for this scope.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
