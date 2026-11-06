# STATEMENTS-EVALUATION-011 — Monte Carlo paths, quantiles and breach probabilities

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice, domain and configuration

Monte Carlo repeats the complete statement evaluation, including period capital-state updates, and aggregates forecast cells across paths. Required controls are positive path count and seed; at least one forecast period must exist. Requested quantiles must be finite in `[0,1]`, are sorted and deduplicated; omitted or empty requests select 0.05, 0.50 and 0.95. Path-data export defaults off. These defaults are observed, not an adoption of a confidence-level or convergence policy.

## Ordered algorithm

1. Prepare the model once. For each zero-based path create a clean evaluator/cache and derive stream ID `seed*6364136223846793005 + path_index` with wrapping integer arithmetic. Node-specific generators then use the mapping in [records 07](07-stochastic-forecast-kernels.md) and [08](08-correlated-shocks.md).
2. Evaluate every model period, including actual history needed by formulas and capital state. Native execution uses Rayon fold/reduce; WASM is serial. If multiple paths fail, reduction retains the error from the smallest path index.
3. Collect all nodes at every forecast period. Any nonfinite forecast cell fails aggregation, even if another output node is the user's primary interest; undefined actual-history cells are not themselves included in these forecast sample checks. Retain forecast-period warnings, sort with deterministic period/node/value-bit keys and deduplicate.
4. Keep every sample internally whether or not path export is requested. Verify the accumulated count equals requested paths, sort each metric-period sample, and evaluate R-7 quantiles: `h=(n-1)q`, linearly interpolate between adjacent order statistics. There is no standard error, convergence stopping rule, antithetic sampling or online approximate quantile in this path.
5. Optional path data is a long table sorted by path, metric and period text. The path identifier is represented as `u32` by conversion from the path index, without a separate configuration cap enforcing that range. This is a large-domain serialization limitation, not a measured feasible-memory failure.
6. Breach probability counts paths with **any** forecast value strictly greater than the supplied threshold and divides by path count. It needs the retained path vectors. Threshold finiteness is not explicitly checked; NaN comparisons are false and therefore yield zero breaches. Quantile lookup selects the first stored probability within `1e-12`; distinct near-identical requested quantiles can therefore alias at lookup. Percentile tables name columns `p` plus probability times100, rounding to an integer label within `1e-9` percentage units; duplicate column labels error, and missing stored quantiles become NaN.

## Replay and output limitations

Internal path vectors are skipped by serialization. After a wire roundtrip, breach calculations need exported path data to reconstruct them; an otherwise valid quantile-only result cannot reconstruct pathwise events. Restore uses table columns, not an independently certified unique/complete `(path,metric,period)` key set. Complete table-shape validation must not be mistaken for full probabilistic sample validation.

Monte Carlo does not run ordinary finalization's optional check suite or build each path's ordinary Money result view. Its all-forecast-cells-finite gate is stricter than ordinary evaluation's warning-and-store behavior. Memory grows with paths times metrics times forecast periods despite path export being off. Neither a fixed seed nor deterministic reduction measures Monte Carlo error.

## Alternatives and tradeoffs

A streaming estimator could reduce memory but change exact quantiles; explicit pathwise checks or a convergence estimator would add different outputs. These are not implied implementation changes. A fixed finite sample alone does not establish moment/quantile accuracy or bound sampling error.

## Evidence

[Monte Carlo implementation](../../../../finstack-quant/statements/src/evaluator/monte_carlo.rs), [core quantile](../../../../finstack-quant/core/src/math/stats.rs), [engine](../../../../finstack-quant/statements/src/evaluator/engine.rs). Inspected [capital Monte Carlo tests](../../../../finstack-quant/statements/tests/monte_carlo_capital_structure.rs): one path is replayed as deterministic explicit samples and zero-volatility paths equal deterministic results. Inspected [forecast regression](../../../../finstack-quant/statements/tests/finance_forecast_regressions.rs) compares one/four-thread serialized results. These are same-engine path/replay invariants, not independent model accuracy checks. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
