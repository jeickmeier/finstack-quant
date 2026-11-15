# STATEMENTS-ANALYTICS-CREDIT-06: Bundled TTM leverage and coverage checks

- Decision status: **Accepted for the full leverage and coverage screening contract (2026-11-15)**.
- Implementation status: **Reconciliation pending**.
- Validation status: **Existing evidence mapped. Independent qualification pending**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for bundled TTM leverage and coverage checks

1. Calculate bundled leverage as period-end gross interest-bearing debt divided by TTM EBITDA. Calculate interest coverage as TTM EBITDA divided by TTM recognized statement interest expense, including PIK. Calculate bundled debt-service coverage as TTM EBITDA divided by the sum of scheduled cash interest and scheduled principal due on borrowings in the same TTM window. Require a complete supported TTM window and verify units, reporting scope and currency. [Bundled leverage and coverage formulas](../statements-contracts/08-leverage-and-coverage.md#accepted-decision-for-bundled-leverage-and-coverage).
2. For other windows or agreement-specific terms, define a named custom check and declare its numerator, denominator, window and corresponding thresholds.
3. Zero gross debt is valid. Nonpositive EBITDA makes leverage unavailable. Coverage ratios retain signed earnings with a positive denominator. Confirmed zero interest or debt service makes the corresponding coverage ratio inapplicable. Missing required inputs make the check incomplete. Invalid inputs, calendar or arithmetic produce errors.
4. Require explicit, sourced, finite thresholds. Place the leverage warning interval within the error interval, and set the coverage error floor at or below the warning floor. Evaluate the error threshold first. Equality meets the corresponding bound. For each selected period, report the window, operands, ratio or reason it is unavailable, and threshold outcome. [Numerical comparisons and check suites](../statements-contracts/14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites).

For example, USD 210 million of period-end gross debt divided by USD 30 million of TTM EBITDA gives 7× leverage. With USD 12 million of TTM recognized interest expense, interest coverage is 2.5×. Warning and error leverage ceilings of 6× and 10× produce a warning for 7× leverage.

## Audited choice and rationale

Leverage divides current debt by a complete trailing-year earnings sum; coverage divides same-period earnings by interest. Both enforce unit compatibility and prioritize Error thresholds over Warning thresholds. They are distinct clocks, rather than interchangeable credit ratios.

## Audited scope and input domain

Debt and EBITDA nodes for leverage, numerator and denominator nodes for coverage, ordered model/results periods and caller warning/error ranges or floors.

## Audited algorithm

1. Leverage first validates current debt versus earnings units, then obtains finite current debt and the complete trailing EBITDA sum through `reports::trailing_sum_at`, which delegates calendar-window selection to core LTM logic. Incomplete windows, missing/non-finite observations, inconsistent units within that trailing helper or failed/overflowing sums yield no usable sum and skip the check.
2. A full TTM EBITDA <= 0 generates an Error finding. Otherwise compute debt/TTM EBITDA and reject a non-finite ratio. Check the error interval first, then warning interval; strict outside-band comparison means endpoints pass.
3. Coverage requires finite same-period numerator/denominator and compatible units. Negative denominator generates an Error; zero denominator skips. For positive denominator, compute numerator/denominator, rejecting non-finite output. Strictly below error floor is Error, otherwise strictly below warning floor is Warning.
4. `passed` is false only for Error findings, so a warning-level leverage/coverage concern remains a passed check.

## Audited selection and controls

Credit-suite defaults are leverage warning [0,6], error [0,10], coverage warning 1.5 and error 1.0. The LBO factory overrides only leverage warning to [0,8]. Thresholds themselves are not validated for finite/order/nesting constraints by these checks.

## Audited conventions

Ratios are multiples. The leverage denominator is a trailing-year flow, while coverage uses one observation's flows. Neither path computes net debt, adjusted EBITDA or covenant-defined earnings unless the caller's nodes already embody those adjustments. No FX conversion occurs.

## Audited limitations, failure and fallback

A failed trailing-sum lookup, including its unit/error cases, becomes a skipped leverage observation. Zero interest is skipped rather than reported as infinite/undefined coverage. Negative debt can fall below the default zero lower bound. Supplied periods/units must be coherent, and pass alone does not establish complete credit coverage.

## Audited alternatives and tradeoffs

Current annualized earnings, TTM net debt or covenant-specific adjustments are alternative leverage definitions. Infinite coverage for zero interest may be mathematically convenient but requires a distinct representation; an explicit unavailable result is more auditable. Propagating trailing-sum failures would be stricter than skipping.

## Evidence

[leverage.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/credit/leverage.rs), [coverage.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/credit/coverage.rs) and the called [reports.rs](../../../../finstack-quant/statements-analytics/src/analysis/reports.rs) establish the clocks. [credit_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/credit_tests.rs) locates four-quarter/incomplete-TTM tests, error-band selection and zero/negative coverage-denominator tests. Core LTM is independently inventoried in the core date scope; these local tests are arithmetic/contract evidence.

The live trailing-sum helper propagates Core window errors, including unsupported weekly windows, when it attempts window selection. It still returns no sum for incomplete windows and for missing, incompatible or nonfinite observations within the window, and for a nonfinite total. [Live trailing-sum helper](../../../../finstack-quant/statements-analytics/src/analysis/reports.rs).

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
