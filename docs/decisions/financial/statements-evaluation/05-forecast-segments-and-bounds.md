# STATEMENTS-EVALUATION-005 — Forecast segment anchoring, overrides and output bounds

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice, scope and rationale

A forecast generates a complete run from a prior anchor, caches it by node/period and lets explicit cells override selected outputs. This preserves elapsed forecast steps, random draws and seasonal phase across overrides. It is not a recurrence restarted from each explicitly supplied forecast value. The rule applies to all method-specific kernels in records 06–10; even a time-series method that ignores its supplied base is reached through this anchor requirement.

## Algorithm and controls

1. Return an existing cached forecast cell when available. Otherwise find the current model index and the latest preceding visible actual anchor. If none exists, find the first preceding permitted explicit value that can anchor the run.
2. Define the run from just after the anchor until the next visible actual period. Explicit forecast values inside the run do not split it. Resolve the base strictly before the first projected period: prefer a visible actual's explicit value, otherwise its evaluated history, then the latest earlier historical value. Missing usable base errors.
3. Parse the typed method parameters and generate the entire run in model-period order. In Monte Carlo, optionally construct correlated shocks and cache latent shocks for the peer mechanism. Cache the full generated series even when explicit cells will ultimately win for part of it.
4. Apply optional minimum and maximum to **finite generated outputs**. Bounds must be finite and ordered. Nonfinite generated values remain nonfinite for diagnostic handling; bounds do not repair them. Clipped outputs are not fed back into the stochastic/deterministic recurrence. Latent shocks remain available before clipping and explicit replacement.
5. The evaluator's overall precedence selects visible explicit value before forecast, as described in [record 01](01-graph-precedence-and-prepared-plans.md). Hidden actual cells under an as-of cutoff can be projected; availability, not merely the stored actual flag, controls eligibility.

## Selection and conventions

The node's explicit forecast specification selects the kernel; there is no fitted automatic model selection. Model entries count forecast steps, regardless of duration. Ordinary single evaluation validates correlation parameters and emits tracing guidance but generates independent series; correlation is applied only in Monte Carlo. Forecast warnings and raw-series errors have different channels: kernel errors propagate, while some advisories are tracing messages rather than result-table warnings.

## Limitations and alternatives

An explicit forecast cell can be economically far from the latent trajectory while later forecast cells remain unchanged. This is an observed supported policy, not a general rule for incorporating new information. Alternatives are restart-on-override, interval-specific specifications or state-conditioned models; those would change present outputs and random stream consumption. Likewise, a clipped stochastic state process differs from output-only clipping. Output bounds do not enforce a model-state domain.

A base is still required for methods whose supplied history determines the calculation. Correlation ordering and hidden/explicit peer configurations require the conditions in [record 08](08-correlated-shocks.md), rather than assuming every evaluated peer cell creates shocks. No out-of-sample forecast accuracy or appropriateness for a particular financial statement is established by this audit.

## Evidence

[Forecast integration](../../../../finstack-quant/statements/src/evaluator/forecast_eval.rs), [method dispatch and bounds](../../../../finstack-quant/statements/src/forecast/mod.rs), [precedence](../../../../finstack-quant/statements/src/evaluator/precedence.rs). Inspected [production reporting regression](../../../../finstack-quant/statements/tests/production_reporting_audit.rs) replaces the first forecast with 999 and verifies later growth/seasonal cells equal the baseline. Inspected [forecast regressions](../../../../finstack-quant/statements/tests/finance_forecast_regressions.rs) verify peer clipping and explicit replacement preserve follower latent paths. These are strong behavior characterizations, not independent forecasting validation. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
