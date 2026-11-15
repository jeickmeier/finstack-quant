# STATEMENTS-ANALYTICS-CREDIT-04: Signed period change relative to prior magnitude

- Decision status: **Accepted for the full period-change screening contract (2026-11-15)**.
- Implementation status: **Reconciliation pending**.
- Validation status: **Existing evidence mapped. Independent qualification pending**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for signed period change relative to prior magnitude

1. Name the diagnostic signed change relative to prior magnitude and calculate it as `(current−prior)/|prior|`. A positive result means the recorded value increased. If the prior value is exactly zero, retain the signed difference and mark the ratio unavailable with the zero-base reason. Every nonzero prior, including a tiny value, is eligible for checked arithmetic.
2. Compare immediately adjacent periods on the verified reporting calendar. Select actuals, forecasts or both according to the current period. Explicitly elect whether to compare the first forecast with the immediately preceding actual and label that transition when included. Bounds apply to each reporting period without annualization. [Model timelines and readiness](../statements-contracts/01-model-timeline-and-values.md#accepted-decision-for-model-timelines-values-and-readiness).
3. Select explicit, unique nodes with compatible units. Require finite, ordered, inclusive decimal bounds and identify their source for each series and cadence. A ratio outside a bound is a breach in either advisory or required mode.
4. Report both observations, signed difference, ratio or reason it is unavailable, and the applicable bounds. Missing or masked required inputs make the check incomplete. Invalid inputs or unrepresentable arithmetic produce errors. A scope with no applicable pairs is unevaluated. [Numerical comparisons and check suites](../statements-contracts/14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites).

For example, a change from −100 to −50 gives +50%, which is within an inclusive upper bound of +50%. A change from zero to 50 has a signed difference of +50 and an unavailable ratio.

## Audited choice and rationale

The growth screen compares adjacent observations using an absolute prior-value denominator. This permits negative prior values without reversing the direction of improvement; it is a plausibility diagnostic rather than a financial return definition.

## Audited scope and input domain

One node, decimal lower/upper period-growth bounds and a scope selecting actual, forecast or all current periods. Model-period ordering defines adjacency.

## Audited algorithm

For each adjacent pair, apply the scope to the current period only. Require both observations finite and `abs(previous) >= f64::EPSILON`; otherwise skip. Compute `g=(current−previous)/abs(previous)`. A strict `g < minimum` or `g > maximum` produces a Warning with `100*abs(g)` as relative deviation. Equality at either bound passes.

## Audited selection and controls

The three-statement suite uses [−0.5,1.0] over all mapped nodes. The first period cannot be checked. No calendar resampling or annualization occurs.

## Audited conventions

Bounds are decimal change per model observation, not annualized percentages. A movement from −100 to −50 is +0.5; from −100 to +100 is +2.0. Forecast-only checks can compare a first forecast to the preceding actual observation.

## Audited limitations, failure and fallback

Near-zero priors, missing inputs and non-finite inputs produce no growth finding. Bounds are not checked for finite values or order; arithmetic overflow is not separately rejected. Cross-period units and cadence are not checked here, and repeated mapped nodes can duplicate findings.

## Audited alternatives and tradeoffs

Log growth requires positive values; denominator based on current/prior scale changes sign-crossing behavior. Annualized or matched-cadence growth is more comparable across periods but requires a different contract. Explicit zero-base and incomplete statuses would make skipped diagnostics visible.

## Evidence

[growth_rate.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/consistency/growth_rate.rs) owns calculation and scope selection. [consistency_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/consistency_tests.rs) locates within-band, spike and sharp-decline tests. These establish local warning behavior rather than predictive validity of bounds.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
