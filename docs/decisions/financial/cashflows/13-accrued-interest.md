# CASHFLOWS-013 — Schedule-driven accrued interest and ex-coupon entitlement

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped to `accrual.rs` and its period-context helpers. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice and input domain

Accrued interest is reconstructed from a validated schedule's coupon amounts and accrual metadata, not from instrument rate specifications. Default configuration is Linear, includes PIK, has no ex-coupon rule and no supplied frequency. Interest-like kinds and optional PIK participate; fees do not. ACT/ACT ICMA may require caller frequency. Explicit issue metadata is required whenever coupons exist, even when flow accrual metadata supplies dates.

## Period and amount preparation

1. Validate schedule and coupon currency against initial notional. Group same-payment rows only when accrual start/end, day count, calendar, nominal reference coupon and termination flag match. Sum cash and PIK separately; use the first positive supplied accrual factor as the bucket's factor.
2. Use explicit accrual start/end where available; otherwise infer first start from issue and later starts from the preceding bucket's end, with payment date as end. Ignore nonpositive-length periods. This fallback is an assumption for ad hoc schedules, not equivalent to full contractual metadata.
3. Reconstruct outstanding at each segment start. For related segments sharing payment/day-count/calendar/reference-period identity, remember the earliest coupon start so ex-coupon entitlement covers all segments, including segments not yet begun.
4. Coupon total `C` includes PIK according to configuration. Skip zero totals. Full fraction `A` is stored positive accrual factor or calculated day count; skip nonpositive `A`. Store opening notional `N` for compounded accrual.

## Active window, day count and ex-coupon logic

A coupon is active while `coupon_start <= as_of < payment_date`. Completed accrual can therefore remain fully accrued during a payment lag. If payment precedes accrual end, the active window ends at payment. For each segment, clip elapsed endpoint into its bounds. For ACT/365L use stored full fraction times actual elapsed/full days; otherwise rescale `elapsed = stored_A × daycount_elapsed/daycount_full` when full day count is finite positive. Clamp elapsed into `[0,A]`.

Ex-date is payment minus `days_before_coupon`: business days when a calendar ID is supplied, calendar days otherwise; maximum 366 days. Require ex-date strictly after the whole coupon's start. Settlement exactly on ex-date retains entitlement: ex-coupon applies only for `ex_date < as_of < payment_date`. In that interval use `e = elapsed−A ≤ 0`, including future balance segments with zero elapsed. Before ex-coupon, future segments are skipped. This subtracts the entire selected coupon entitlement over the relevant payment, not only the segment active on settlement.

## Accrual formulas

- Linear: `AI=C×e/A`.
- Compounded: require `N>0` (otherwise return zero), set `q=C/N`, require finite `q>-1`. If `|q|<1e-12`, use linear. Otherwise with `f=e/A`, ordinary accrual is `N×expm1(f×ln1p(q))`; ex-coupon accrual for `f<0` is `−N×expm1(−f×ln1p(q))`.

The compounded ex-coupon branch represents negative growth over the remaining fraction. It is not computed as elapsed compound growth minus the full coupon. Aggregate active segments with Neumaier compensation and reject nonfinite coupon totals/formula results/final totals. Coupon-rate fields are not used to recompute `C`.

## Alternatives and limitations

Linear and compounded accrual are supported choices; the latter is a fractional growth assumption over an already-known coupon total. It is not daily replay of overnight observations. [CASHFLOWS-008](08-floating-replay.md) supplies a different state-driven accrued amount. The bucket logic does not compare differing positive factors on otherwise identical rows; the first factor wins. Default PIK inclusion and strict ex-date entitlement are product policies, not universal conventions.

## Evidence

- [accrual.rs](../../../../finstack-quant/cashflows/src/accrual.rs), [contractual accrual helpers](../../../../finstack-quant/cashflows/src/builder/periods.rs).
- Located hand-formula tests in accrual source: `ex_coupon_negative_ai_linear_golden`, `record_date_retains_coupon_and_next_settlement_rebates_interest`, `compounded_accrual_positive_golden`, `compounded_accrual_ex_window_negative_golden`, plus invalid logarithm domain, future-leap denominator, ICMA frequency and coincident-payment identity checks. [compiler_regressions.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/compiler_regressions.rs) covers forfeiture across principal segments.
- [ICMA Rule Book](../../../REFERENCES.md#icma-rule-book) and [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions) are bibliographic pointers. This audit did not verify a clause establishing each product's ex-date equality, PIK inclusion or compounded negative accrual formula.
- No test execution or independent market accrued-interest benchmark was performed.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
