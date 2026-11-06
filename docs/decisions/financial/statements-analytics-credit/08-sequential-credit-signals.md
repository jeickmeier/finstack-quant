# STATEMENTS-ANALYTICS-CREDIT-08: Negative-FCF runs and adverse-trend counters

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Sequential counters flag sustained negative FCF or consecutive adverse node changes. They share model-order state but count different events: negative observations versus adverse transitions.

## Scope and input domain

FCF node and positive warning/error observation counts; trend node, increasing-is-good or decreasing-is-good direction, positive lookback transition count and chosen severity.

## Algorithm

- FCF increments a counter for each strictly negative finite value; zero, positive, missing or non-finite observations reset it. At each period compare against the error count first, then warning count, emitting findings throughout a long run. Only positivity of the configured counts is enforced, not error >= warning.
- Trend walks adjacent periods. For increasing-is-good, a strict decline is adverse; for decreasing-is-good, a strict increase is adverse. An adverse move increments the counter, while equality, improvement or missing/non-finite comparison resets it. A current non-finite value produces a Warning and reset; a non-finite prior comparison is unavailable without the same current-period warning.
- A lookback of three therefore requires three consecutive adverse transitions, ordinarily four observations. The first period is not evaluated as a current trend observation. On reaching the count, emit configured severity for every continuing adverse period.

## Selection and controls

Credit-suite FCF counts default to two/four. Suite trends inspect EBITDA levels and debt levels, not derived leverage/coverage. Check result passes depend on Error severity, including the configured trend severity.

## Conventions

Counts are observations/transitions, not months or years. Model order is authoritative, with no sorting or gap-duration interpretation. Missing values break a run rather than preserve a prior signal.

## Limitations, failure and fallback

No unit/cadence check validates adjacent comparisons. A long gap counts as one transition, and missing data can clear deterioration. FCF non-finite values themselves have no finding here; coverage depends on a separate non-finite check. An initial non-finite trend observation is not caught by this trend loop.

## Alternatives and tradeoffs

Rolling-window frequency, slope/regression or elapsed-time deterioration measures capture different persistence and require more data and policy. Preserving runs across missing data avoids clearing signals but can imply unobserved continuity; explicit unknown-state counters avoid that inference.

## Evidence

[fcf_sign.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/credit/fcf_sign.rs), [trend.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/credit/trend.rs) and [suites.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/suites.rs) are the owners. [credit_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/credit_tests.rs) locates consecutive-negative, direction-specific deterioration and `trend_nan_period_warns_and_resets` tests. These evidence counter semantics only.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
