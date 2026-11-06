# STATEMENTS-ANALYTICS-CREDIT-04: Signed-denominator period-growth plausibility

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

The growth screen compares adjacent observations using an absolute prior-value denominator. This permits negative prior values without reversing the direction of improvement; it is a plausibility diagnostic rather than a financial return definition.

## Scope and input domain

One node, decimal lower/upper period-growth bounds and a scope selecting actual, forecast or all current periods. Model-period ordering defines adjacency.

## Algorithm

For each adjacent pair, apply the scope to the current period only. Require both observations finite and `abs(previous) >= f64::EPSILON`; otherwise skip. Compute `g=(current−previous)/abs(previous)`. A strict `g < minimum` or `g > maximum` produces a Warning with `100*abs(g)` as relative deviation. Equality at either bound passes.

## Selection and controls

The three-statement suite uses [−0.5,1.0] over all mapped nodes. The first period cannot be checked. No calendar resampling or annualization occurs.

## Conventions

Bounds are decimal change per model observation, not annualized percentages. A movement from −100 to −50 is +0.5; from −100 to +100 is +2.0. Forecast-only checks can compare a first forecast to the preceding actual observation.

## Limitations, failure and fallback

Near-zero priors, missing inputs and non-finite inputs produce no growth finding. Bounds are not checked for finite values or order; arithmetic overflow is not separately rejected. Cross-period units and cadence are not checked here, and repeated mapped nodes can duplicate findings.

## Alternatives and tradeoffs

Log growth requires positive values; denominator based on current/prior scale changes sign-crossing behavior. Annualized or matched-cadence growth is more comparable across periods but requires a different contract. Explicit zero-base and incomplete statuses would make skipped diagnostics visible.

## Evidence

[growth_rate.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/consistency/growth_rate.rs) owns calculation and scope selection. [consistency_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/consistency_tests.rs) locates within-band, spike and sharp-decline tests. These establish local warning behavior rather than predictive validity of bounds.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
