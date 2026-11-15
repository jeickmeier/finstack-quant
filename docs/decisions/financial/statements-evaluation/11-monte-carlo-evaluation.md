# STATEMENTS-EVALUATION-011 — Monte Carlo paths, quantiles and breach probabilities

Decision status: **Accepted for the full Monte Carlo paths, quantiles and breach contract (2026-11-15)**. Implementation status: **Reconciliation pending**. Validation status: **Existing evidence mapped. Independent qualification pending**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Accepted decision for Monte Carlo paths, quantiles and breach probabilities

1. Require an exact positive path count that fits the path-ID domain, an exact seed, at least one forecast period and explicit report metrics. Check work and storage budgets and size arithmetic before allocating.
2. Evaluate the complete model for each path with fresh state, the selected evaluation context and approved random streams. Select failures in deterministic logical order, with the earliest failing path in path order. Ordinary check suites apply at single-run finalization. [Prepared evaluation](01-graph-precedence-and-prepared-plans.md#accepted-decision-for-dependency-graphs-cell-precedence-and-prepared-evaluation) · [Versioned streams](07-stochastic-forecast-kernels.md#accepted-decision-for-stochastic-forecast-kernels-and-replay) · [Latent correlation](08-correlated-shocks.md#accepted-decision-for-latent-one-peer-correlation).
3. Require exactly one valid typed sample per path for every requested metric and forecast period. Missing or unavailable required samples make the run incomplete, and invalid samples are execution errors. Publish statistics only for complete runs.
4. Use R-7 for sample quantiles. An omitted percentile request selects `0.05`, `0.50` and `0.95`. Requested levels must be distinct, finite and in `[0,1]`. Store and retrieve results by exact probability key and include the result unit, currency where applicable and sample count.
5. Path export is optional. Restoration requires a complete, unique set of path × metric × period keys with valid typed samples. For breach probability, require a typed finite upper threshold compatible with the metric. Count each path once if any forecast-period value is strictly greater than that threshold and divide by the path count. Breach probability is unavailable if path samples are absent.

## Audited domain and configuration

Monte Carlo repeats the complete statement evaluation, including period capital-state updates, and aggregates forecast cells across paths. Required controls are positive path count and seed; at least one forecast period must exist. Requested quantiles must be finite in `[0,1]`, are sorted and deduplicated; omitted or empty requests select 0.05, 0.50 and 0.95. Path-data export defaults off. These defaults are observed, not an adoption of a confidence-level or convergence policy.

## Audited ordered algorithm

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

The accepted contract requires explicit sample coverage and preserves exact sample quantiles. Budget validation, typed samples and complete path restoration remain reconciliation work. Independent moment, quantile and sampling-error qualification remains open.

## Evidence

[Monte Carlo implementation](../../../../finstack-quant/statements/src/evaluator/monte_carlo.rs), [core quantile](../../../../finstack-quant/core/src/math/stats.rs), [engine](../../../../finstack-quant/statements/src/evaluator/engine.rs). Inspected [capital Monte Carlo tests](../../../../finstack-quant/statements/tests/monte_carlo_capital_structure.rs): one path is replayed as deterministic explicit samples and zero-volatility paths equal deterministic results. Inspected [forecast regression](../../../../finstack-quant/statements/tests/finance_forecast_regressions.rs) compares one/four-thread serialized results. These are same-engine path/replay invariants, not independent model accuracy checks. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
