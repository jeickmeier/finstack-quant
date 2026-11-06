# STATEMENTS-EVALUATION-006 — Deterministic growth, override curves and convergence forecasts

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice and input domain

Deterministic forecasts apply explicitly selected stepwise transformations; no parameter estimation occurs. Rates and curve entries are decimal growth per model period, fade half-life is in model periods, and levels retain the node's units. Forward fill, growth, rate curve, explicit overrides and three fade shapes are distinct supported choices. A conventional financial rationale for each caller's parameterization is not inferred from availability in the API.

## Algorithms and numerical controls

- Forward fill repeats the base without its own finite-base check. The result layer can therefore preserve and warn on a nonfinite base. Growth requires finite base/rate and recurs `x[t]=x[t-1]*(1+r)`; curve growth uses an exactly horizon-length finite rate vector. Every generated result must remain finite. Negative base is permitted with an advisory, and rates below -1 are not rejected; they can change sign. The warning prose suggesting sign cannot change does not hold for that allowed rate domain.
- Override parses every supplied key to a period ID, rejects aliases identifying the same period, requires all keys inside the forecast run, and requires finite base/override values. Walk the supplied period order, replacing the carried level when an override exists and otherwise carrying the most recent level. This method's own carry behavior differs from evaluator-level explicit overrides in [record 05](05-forecast-segments-and-bounds.md).
- Linear fade for horizon `N` uses `x[t]=(1-t/N)*base+(t/N)*target`, `t=1..N`; the final value is exactly the target. The convex combination avoids the avoidable overflow of subtracting opposite extreme endpoints.
- Geometric fade requires nonzero same-sign finite base and target, then uses `x[t]=base*(target/base)^(t/N)`. It forms the raw ratio, so a ratio overflow can fail despite finite representable interpolation values; ratio underflow can collapse a same-sign path to zero. These are source-derived numerical concerns; no new extreme-value experiment was run.
- Exponential fade requires finite positive half-life `H`, uses weight `w=2^(-t/H)` and `x[t]=w*base+(1-w)*target`. It approaches the target without requiring the terminal forecast to equal it. Half-life supplied for another shape is rejected. Base and target are checked even for empty horizon; shape-domain checks occur after the empty-horizon return, so an empty geometric run can bypass same-sign/nonzero checks.

## Selection, failures and alternatives

Typed forecast parameters dispatch directly to these kernels. Bounds apply afterward and do not feed back. Finite endpoint and per-step checks constrain most deterministic methods, while forward fill deliberately exposes nonfinite propagation. Zero horizon produces no forecast values and is not equivalent to a general parameter-validation endpoint.

Log-space geometric interpolation could improve extreme-ratio stability, but it would need sign handling and defined endpoints; it is not the current implementation. Annual growth converted by actual period duration, continuous-time decay and override-triggered restart are different economic models.

## Evidence

[Deterministic kernels](../../../../finstack-quant/statements/src/forecast/deterministic.rs), [fades](../../../../finstack-quant/statements/src/forecast/fade.rs), [override method](../../../../finstack-quant/statements/src/forecast/override_method.rs), [dispatch](../../../../finstack-quant/statements/src/forecast/mod.rs). Located [forecast regression tests](../../../../finstack-quant/statements/tests/finance_forecast_regressions.rs) cover period aliases, duplicate overrides, finite opposite-extreme fades and overflow rejection; [forecast tests](../../../../finstack-quant/statements/tests/forecast/forecast_tests.rs) characterize ordinary curves and growth. These pin implementation behavior; forecast calibration and independent accuracy evidence are not established. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
