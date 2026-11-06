# STATEMENTS-EVALUATION-003 — Historical offsets, growth rates and reporting windows

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and scope

Temporal formulas read exact model-period history; reporting aggregates use named period windows. Supported calendars include fiscal IDs with explicit model bounds. Values retain the source metric's signed units; percent changes and annualized growth return decimal ratios. These are statement transformations, not price-return valuation rules. Inspection is complete; the daily/weekly CAGR clock conflicts with an already accepted decision.

## Ordered calculation

1. `lag(expr,n)` requires an integer nonnegative offset; zero evaluates the current expression. Positive offsets resolve a historical `PeriodId`, returning NaN if the slot is absent. Fiscal daily offsets use actual model starts plus/minus calendar days so fiscal-year boundaries are navigated correctly. In a standalone context without model bounds, only resolvable same-year backward fiscal-day shifts fall back to PeriodId arithmetic; unavailable/out-of-range geometry returns NaN. Expressions are reevaluated in a context for the historical row, with future history excluded; typed capital references use historical snapshots.
2. `lead` is unsupported and errors. `shift` accepts a signed integer: negative produces NaN, positive delegates to lag, zero uses current. Lag/diff/pct_change/shift integer helpers accept `abs(fract(value))<=1e-10` and then truncate toward zero; growth-rate lag instead tests distance from the rounded integer with that tolerance. Missing capital snapshots produce NaN; an existing snapshot with an invalid instrument/component remains an error.
3. `diff` and `pct_change` default to lag one. For zero lag, finite input returns zero. Percentage change is `(current-base)/base`; `abs(base)<1e-10` produces a diagnostic and NaN. Negative bases otherwise remain supported, so this is algebraic signed change, not a universal economic growth measure.
4. `growth_rate` defaults the lag to periods per year, requires positive integer lag, finite positive base of at least `1e-10` and finite nonnegative current value, and computes `(current/base)^(1/Y)-1`. Fixed-frequency monthly/quarterly/semiannual/annual periods use `Y=lag/frequency`. Daily and weekly periods use **Actual/Actual** between the periods' final included dates (`end-1`); fiscal dates require model geometry. This dated branch has no explicit day-count override.
5. YTD sums finite values from ID index one through current. QTD is monthly-only and calculates start index `((current.index-1)/3)*3+1`, constructing a Gregorian month ID with the current year label. FiscalYTD accepts an exact integer start month 1–12 and similarly constructs a Gregorian month ID from the current month/year labels, moving the year back when necessary. Neither QTD nor FiscalYTD derives its boundary from explicit fiscal/Gregorian model dates. The cited evidence does not establish their full fiscal-ID geometry. These functions permit incomplete windows and skip nonfinite cells; they are not the stricter TTM policy.
6. TTM delegates window construction to core. Monthly/quarterly/semiannual/annual require 12/4/2/1 named slots; weekly uses 52 weeks. Daily uses model days in `[exclusive_end-12 calendar months, exclusive_end)`, accepting absent weekends/holidays as off-grid days and requiring sufficient leading calendar coverage. A missing slot, invalid window or any nonfinite required value yields NaN. No partial-window annualization occurs.

## Selection, limitations and alternatives

These operations are selected explicitly in formulas. Arbitrary-expression evaluation can be more expensive than a simple column lookup but uses the same time boundary. Observation-count rolling windows are a separate choice in [record 04](04-historical-statistics-and-ewm.md). The accepted rejection of ambiguous partial TTM remains intact; a finite-only YTD must not be advertised as a complete-year measure.

[ADR 0008](../conventions/0008-financial-defaults-and-cagr.md) already selects Actual/365 Fixed for an explicit dated CAGR window. The daily/weekly Actual/Actual branch therefore conflicts with that accepted contract; it is **not** a newly optional methodology vote. Frequency annualization is expressly outside that dated default's scope. Calendar-day versus business-day daily grids and 52-week TTM are explicit reporting conventions, not automatic elapsed-time equivalents. A configurable dated clock or strict-completeness YTD are possible alternatives, not implemented defaults.

## Evidence

[Temporal formulas](../../../../finstack-quant/statements/src/evaluator/formula_timeseries.rs), [period aggregates](../../../../finstack-quant/statements/src/evaluator/formula_aggregates.rs), [core LTM geometry](../../../../finstack-quant/core/src/dates/periods/ltm.rs). Inspected `daily_and_weekly_growth_use_elapsed_period_end_dates` in [temporal regressions](../../../../finstack-quant/statements/tests/finance_temporal_regressions.rs): it generates observations using the same Actual/Actual helper and checks 20% recovery. That confirms current convention wiring but is not independent evidence for the accepted Actual/365F basis. The same file locates fiscal offsets, typed capital references and daily calendar/business-grid TTM cases. No tests were executed; the inspected regression does not qualify the accepted fixed-365 clock over leap windows.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
