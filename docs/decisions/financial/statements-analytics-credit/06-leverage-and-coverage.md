# STATEMENTS-ANALYTICS-CREDIT-06: Debt/TTM earnings and current-period coverage checks

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Leverage divides current debt by a complete trailing-year earnings sum; coverage divides same-period earnings by interest. Both enforce unit compatibility and prioritize Error thresholds over Warning thresholds. They are distinct clocks, rather than interchangeable credit ratios.

## Scope and input domain

Debt and EBITDA nodes for leverage, numerator and denominator nodes for coverage, ordered model/results periods and caller warning/error ranges or floors.

## Algorithm

1. Leverage first validates current debt versus earnings units, then obtains finite current debt and the complete trailing EBITDA sum through `reports::trailing_sum_at`, which delegates calendar-window selection to core LTM logic. Incomplete windows, missing/non-finite observations, inconsistent units within that trailing helper or failed/overflowing sums yield no usable sum and skip the check.
2. A full TTM EBITDA <= 0 generates an Error finding. Otherwise compute debt/TTM EBITDA and reject a non-finite ratio. Check the error interval first, then warning interval; strict outside-band comparison means endpoints pass.
3. Coverage requires finite same-period numerator/denominator and compatible units. Negative denominator generates an Error; zero denominator skips. For positive denominator, compute numerator/denominator, rejecting non-finite output. Strictly below error floor is Error, otherwise strictly below warning floor is Warning.
4. `passed` is false only for Error findings, so a warning-level leverage/coverage concern remains a passed check.

## Selection and controls

Credit-suite defaults are leverage warning [0,6], error [0,10], coverage warning 1.5 and error 1.0. The LBO factory overrides only leverage warning to [0,8]. Thresholds themselves are not validated for finite/order/nesting constraints by these checks.

## Conventions

Ratios are multiples. The leverage denominator is a trailing-year flow, while coverage uses one observation's flows. Neither path computes net debt, adjusted EBITDA or covenant-defined earnings unless the caller's nodes already embody those adjustments. No FX conversion occurs.

## Limitations, failure and fallback

A failed trailing-sum lookup, including its unit/error cases, becomes a skipped leverage observation. Zero interest is skipped rather than reported as infinite/undefined coverage. Negative debt can fall below the default zero lower bound. Supplied periods/units must be coherent, and pass alone does not establish complete credit coverage.

## Alternatives and tradeoffs

Current annualized earnings, TTM net debt or covenant-specific adjustments are alternative leverage definitions. Infinite coverage for zero interest may be mathematically convenient but requires a distinct representation; an explicit unavailable result is more auditable. Propagating trailing-sum failures would be stricter than skipping.

## Evidence

[leverage.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/credit/leverage.rs), [coverage.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/credit/coverage.rs) and the called [reports.rs](../../../../finstack-quant/statements-analytics/src/analysis/reports.rs) establish the clocks. [credit_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/credit_tests.rs) locates four-quarter/incomplete-TTM tests, error-band selection and zero/negative coverage-denominator tests. Core LTM is independently inventoried in the core date scope; these local tests are arithmetic/contract evidence.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
