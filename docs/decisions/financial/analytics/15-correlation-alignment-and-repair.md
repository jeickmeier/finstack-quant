# ANALYTICS-015: Complete-case or pairwise correlation and automatic matrix repair

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice and domain

`Performance.correlation_matrix` estimates equal-weight sample correlations and automatically projects an invalid assembled matrix to a valid correlation matrix. Date alignment is chosen at the matrix level: use one all-ticker common active span when it has at least two observations, otherwise use each pair's overlap. This mixes a statistical selection rule with a numerical repair rule; neither is established merely by registering the API.

## Algorithm and controls

Initialize the diagonal to one. For each pair, choose the common span if available or its pairwise intersection otherwise. Fewer than two paired observations errors; either exactly zero sample variance errors; nonfinite correlation errors. No missing observations are filled and no pair is silently replaced by zero. A one-ticker matrix has no pairs to check and returns self-correlation one even if its active window is empty or degenerate.

Flatten and run the shared correlation-matrix validator. If already valid, force diagonal entries to exactly one and clamp off-diagonals to [−1,1], returning `(matrix,false)`. If validation fails, invoke core `nearest_correlation` with its default 200 iterations and Frobenius-change tolerance 1e−10, then validate the repaired result again. Failure to obtain a valid matrix errors. Unflatten, clamp bounds/diagonal and return `(matrix,true)`.

The core projection is the shared Higham/Dykstra PSD/unit-diagonal routine, whose kernel details are owned by the core-numerics methodology scope. The analytics caller does not expose repair options or opt-out. The boolean is true only for the projection path: small bound-clamping after a successful validation can alter entries while still returning false. It does not report which observation spans were used, adjustment magnitude or iteration diagnostics.

## Conventions, limitations and alternatives

Correlation is dimensionless and not annualized. Pairwise-overlap assembly can be indefinite because different coefficients summarize different samples; projection yields a mathematically usable matrix but changes empirical estimates. A common span of only two points is preferred over longer pairwise spans, so sample-size and precision can change discontinuously when another ticker's history changes. No minimum-history policy beyond two observations, shrinkage intensity or sampling-error model is included.

Always-complete-case estimation, explicit pairwise selection without repair, shrinkage covariance and caller-controlled PSD projection are unimplemented alternatives in this method. A valid/repaired matrix is not proof of a suitable portfolio-risk model or independent agreement on automatic repair.

## Implementation and validation evidence

- [Alignment/assembly/repair caller](../../../../finstack-quant/analytics/src/performance/aggregation.rs), [active-span logic](../../../../finstack-quant/analytics/src/performance/mod.rs), [shared moments](../../../../finstack-quant/core/src/math/stats.rs), and [core projection/defaults](../../../../finstack-quant/core/src/math/linalg/nearest_correlation.rs).
- Located inline `correlation_matrix_repairs_indefinite_pairwise_fixture`, clamp-to-unit-bounds and two-asset cases establish dispatch/invariants. [Correctness regressions](../../../../finstack-quant/analytics/tests/correctness_regressions.rs) reject degenerate pairs. They do not independently validate the repaired empirical risk model or justify mixed-span estimation.
- [Core numerical inventory](../core-numerics/README.md) owns the projection algorithm's detailed numerical evidence. [Ledoit–Wolf](../../../REFERENCES.md#ledoitwolf2004) is context for a distinct shrinkage alternative, not a claim that shrinkage is implemented here.

No matrix simulations or numerical tests were executed.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
