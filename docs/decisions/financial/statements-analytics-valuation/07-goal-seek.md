# STATEMENTS-ANALYTICS-VALUATION-07: Prepared-statement scalar goal seek and residual acceptance

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice and input domain

[Goal seek](../../../../finstack-quant/statements-analytics/src/analysis/goal_seek.rs) solves `evaluated_target(driver)−target_value=0` by Brent's method. Target value is finite; both nodes and periods must exist. It does not impose a forecast-only driver restriction. The starting driver is its existing literal period value, otherwise `1.0`; a calculated node is not rejected merely because a formula is present, so evaluator source precedence determines whether a literal override controls it.

## Algorithm and numerical controls

1. Prepare the evaluator once and clone the model. Each objective probe inserts only the chosen driver cell, preserving monetary currency when declared; other cells are unchanged. Evaluate with the prepared graph and subtract target.
2. Failed evaluations or missing target return NaN to the solver and retain the latest explanatory cause. Failure converting a probe to a monetary amount also returns NaN but does not store that conversion cause. The original model is never mutated.
3. Explicit bounds must be finite and ordered. Both endpoints are evaluated; nonfinite residuals reject the bracket. An endpoint with absolute residual `<1e−9` is accepted, otherwise signs must bracket a root. Brent runs in that bracket with tolerance `1e−9`.
4. Without bounds, use initial bracket size `max(abs(initial_guess),1)`. First restrict the solver to the initial guess's sign; on failure for a nonzero guess, retry the unconstrained default bracket. Shared [Brent](../../../../finstack-quant/core/src/math/solver.rs) defaults are tolerance `1e−12`, 100 iterations and limits `[-1e6,1e6]`.
5. Re-evaluate the proposed root and require finite driver/residual and `abs(residual) <= 1e−9 × max(abs(target),1)`. This independent final target check prevents solver step convergence alone from claiming success.
6. Return solved value, signed residual, tolerance, evaluation count and `converged=true`. If requested, construct a fresh model with the driver cell and validate its semantics. JSON import validates an embedded model, but does not rerun the solve; omitted diagnostics default, including `converged=false`.

Tolerance combines target units with an absolute floor; it is not scaled to currency precision or driver units. Multiple roots, discontinuities, domain-restricted formulas and a root beyond default bounds are not uniquely resolved by this interface.

## Alternatives and evidence

Explicit brackets are supported and give caller control over root selection. Monotonicity analysis, derivative/Newton solves, multi-variable optimization and alternate-root enumeration are not implemented here. Sign-restricted first search is a heuristic, with an observed unconstrained fallback.

Colocated endpoint and JSON-model tests plus [goal-seek integration cases](../../../../finstack-quant/statements-analytics/tests/analysis_goal_seek.rs) supply numerical examples, residual and model-preservation evidence. [Brent](../../../REFERENCES.md#brent-1973) identifies the numerical algorithm, not a guarantee for arbitrary DSL objectives.

Independent qualification of nonmonotone and discontinuous cases was not established by this audit.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
