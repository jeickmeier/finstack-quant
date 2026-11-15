# STATEMENTS-EVALUATION-008 — Latent one-peer correlation and execution order

Decision status: **Accepted for the full latent one-peer correlation contract (2026-11-15)**. Implementation status: **Reconciliation pending**. Validation status: **Existing evidence mapped. Independent qualification pending**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Accepted decision for latent one-peer correlation

1. A correlation specification names one known, distinct peer and a finite coefficient `ρ` in `[−1,1]`. Active nonzero correlation requires Monte Carlo and positive-volatility Gaussian forecast methods on both nodes. Reject active correlation in ordinary evaluation. [Stochastic forecast kernels](07-stochastic-forecast-kernels.md#accepted-decision-for-stochastic-forecast-kernels-and-replay).
2. At exact `ρ = 0`, use the same independent stream as an uncorrelated node and add no active dependency. At `ρ = 1`, copy the peer shock exactly. At `ρ = −1`, negate it exactly. For interior nonzero coefficients, combine the peer and independent standardized shocks as `ρ × z_peer + sqrt((1 − ρ)(1 + ρ)) × z_independent`.
3. Topologically order the active peer graph deterministically and reject cycles. Match peer shocks on the same canonical path and forecast slot. Generate needed latent shocks through masked slots and slots with explicit published values. An unavailable required peer shock makes the follower result unavailable with a reason. [Canonical forecast suffixes](05-forecast-segments-and-bounds.md#accepted-decision-for-forecast-segments-anchoring-and-output-bounds).
4. Store the final standardized shock by run, path, node and period before applying bounds or rounding. Downstream peers in a chain use that final shock. Record the graph, coefficients and stream mapping for replay. [Versioned streams](07-stochastic-forecast-kernels.md#accepted-decision-for-stochastic-forecast-kernels-and-replay).

## Audited input domain

Monte Carlo correlation combines a previously generated peer's latent standardized normal shock with an independent draw. Only Normal, LogNormal and MeanReverting forecasts participate. The configuration requires both peer node and finite `rho in [-1,1]`; it does not accept a full covariance matrix. A chain of these relationships induces additional correlations, which are not separately targeted or solved.

## Audited algorithm and controls

1. Validate the peer exists. During generation require the peer's shock cache already contains a shock for every requested forecast date. An evaluated explicit level does not, by itself, create a latent shock stream.
2. Generate the independent normal stream with a seed formed from the configured seed **XOR** stable node hash, plus the Monte Carlo stream offset. This differs from the independent-forecast path's multiplicative/additive mixing. Consequently adding a correlation specification with `rho=0` does not preserve the original uncorrelated node's exact draws, even though its distributional construction is independent.
3. For each date use `z=rho*z_peer+sqrt(max(1-rho^2,0))*z_independent`, then feed z into the selected recurrence in [record 07](07-stochastic-forecast-kernels.md). Cache z directly before output clipping/explicit replacements; if target sigma is zero, cache zero instead.
4. With nonzero rho, an all-zero peer shock run is rejected as unavailable variance. A zero coefficient does not require nonzero peer variance, but still follows the peer-cache workflow. Correlation against an explicit, nonstatistical or not-yet-generated peer may therefore fail despite the peer's level being available.
5. Ordinary single-path evaluation validates the configuration but logs that correlation is ignored and generates an independent series. Correlation is enabled by Monte Carlo evaluation, not merely by a seed on the ordinary evaluator.

## Ordering, conventions and failure boundary

The current-period DAG includes formula/mask dependencies, **not forecast-peer dependencies**. The peer must already have been generated under the resolved evaluation order; model declaration/formula order can therefore affect whether a valid-looking correlation request succeeds. This is a confirmed source-level integration condition, not a reproduced statistical failure. The contracts audit also found that peer validation does not add graph edges.

The coefficient describes standardized innovation correlation, not correlation of levels, log returns after clipping, or calculated statement outputs. Zero volatility, nonlinear transformations, explicit values and bounds can make realized output correlation different. Preserving latent shocks under clipping and rounding is intentional in the implemented design.

## Alternatives and tradeoffs

The current correlated generator uses the same node seed mixer and PCG64 path generator as independent forecasts. It still looks up peer shocks even at zero correlation. Deterministic peer ordering, zero-coefficient dependency removal and exact endpoint handling remain reconciliation work. [Current correlated generator](../../../../finstack-quant/statements/src/forecast/statistical.rs) · [Independent dispatch](../../../../finstack-quant/statements/src/forecast/mod.rs).

## Evidence

[Correlated generator](../../../../finstack-quant/statements/src/forecast/statistical.rs), [forecast caller](../../../../finstack-quant/statements/src/evaluator/forecast_eval.rs), [graph construction](../../../../finstack-quant/statements/src/evaluator/dag.rs). Inspected [forecast regressions](../../../../finstack-quant/statements/tests/finance_forecast_regressions.rs): rounded peer levels, three-node shock chains, clipping/explicit replacements and serial-versus-parallel wire outputs test latent-stream preservation. They are replay/consistency checks, not independent population correlation confidence tests. No tests were executed, and the cited replay evidence does not bound sampling error or qualify the wider correlation domain.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
