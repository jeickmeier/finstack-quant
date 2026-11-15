# STATEMENTS-EVALUATION-007 — Normal, lognormal, mean-reverting and bootstrap forecasts

Decision status: **Accepted for the full stochastic forecast kernel contract (2026-11-15)**. Implementation status: **Reconciliation pending**. Validation status: **Existing evidence mapped. Independent qualification pending**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Accepted decision for stochastic forecast kernels and replay

1. Calculate and validate each canonical forecast step before applying output bounds. For a standard normal innovation `z`, the normal kernel uses `x[t] = x[t−1] + μ + σz` on signed finite levels with finite drift and finite `σ ≥ 0`. The mean-reverting kernel uses `x[t] = x[t−1] + κ(θ − x[t−1]) + σz`, with finite signed levels and target, `κ` in `(0,1]` and finite `σ ≥ 0`. In these additive kernels, drift, target and standard deviation use the node's level units where applicable. [Forecast steps and output bounds](05-forecast-segments-and-bounds.md#accepted-decision-for-forecast-segments-anchoring-and-output-bounds).
2. Lognormal forecasts require a strictly positive finite base and finite per-step `μ` and `σ ≥ 0`. Compute the step in log space, adding `μ − σ²/2 + σz` to the prior log level. The expected gross multiplier is `exp(μ)`. Preserve the model's exponent without clipping. The restored level must be finite and strictly positive. A representable positive subnormal is valid. Zero or an unrepresentable result is a computation error.
3. Select growth or difference bootstrap explicitly. Require at least two finite historical levels ordered at equal model-period spacing. Sample adjacent increments with unbiased uniform draws with replacement. Growth compounds sampled adjacent growth rates and requires strictly positive history and anchor. Difference adds sampled adjacent differences and supports signed levels and anchor.
4. A zero standard deviation uses the deterministic transition without consuming random draws. Use the shared explicit PCG64 generator. Derive the stream from a versioned key with exact method seed, node ID and evaluation mode, plus run seed and path identity where applicable. Retain replay provenance for the effective seed and stream. [Exact stochastic seeds](../statements-contracts/03-forecast-parameter-contract.md#accepted-decision-for-typed-forecasts-and-validation-staging).

## Audited scope and units

Statistical forecasts are discrete per-model-step processes using caller-supplied parameters, not fitted continuous-time processes. Normal/MR drifts and standard deviations have the node's level units; lognormal drift/volatility are dimensionless per-step log-process inputs. No actual-duration scaling or automatic annual-to-step conversion is performed. Bootstrap samples caller history, which need not share the model's dated history.

## Audited algorithms and random controls

The implementation uses the core PCG64 random generator. Independent node seeds mix the configured seed by multiplication with a fixed golden-ratio constant plus a stable node hash; the hash is FNV-1a followed by SplitMix mixing. Monte Carlo adds an explicit stream identity. Repeated seeded calls are intended to be replayable; this does not itself establish statistical suitability.

1. **Normal:** `x[t]=x[t-1]+mu+sigma*z[t]`, with finite base/mu, finite `sigma>=0` and standard normal draws. Negative base and outcomes are allowed. Each result must be finite.
2. **Lognormal:** require positive finite initial base and finite mu/`sigma>=0`; form `e=mu-0.5*sigma^2+sigma*z`, clamp `e` to `[-709,709]`, then `x[t]=x[t-1]*exp(e)`. Clamping is an observed tail repair with an advisory, not an accepted distributional approximation. Nonfinite outputs error; underflow to zero remains finite and can become absorbing. The public label “always positive” is therefore stronger than the enforced floating-point domain at extremes.
3. **Mean reverting:** `x[t]=x[t-1]+k*(theta-x[t-1])+sigma*z`, with finite base/theta, `sigma>=0` and `k in (0,1]`. It is a discrete additive recursion, not the exact transition of a continuous-time Ornstein–Uhlenbeck model. Negative levels are permitted.
4. **Bootstrap:** require at least two finite historical levels. Growth sampling requires strictly positive historical levels and builds increments `r[i]=h[i+1]/h[i]-1`; difference sampling uses `d[i]=h[i+1]-h[i]`. Draw one increment independently with replacement using uniform index `floor(u*n).min(n-1)`, then compound or add recursively. Base need only be finite even for growth mode. Nonfinite generated results error. There is no block resampling or serial-dependence model.

Normal/lognormal/MR still consume random draws at zero sigma but store zero standardized shock for correlation. Bootstrap does not provide those latent normal shocks. Optional output bounds are applied only after generation, as described in [record 05](05-forecast-segments-and-bounds.md).

## Selection, limitations and alternatives

Explicit forecast-method and typed-parameter dispatch controls selection. The audited normal and mean-reverting kernels already admit negative bases. Exponent clipping, absorbing zero and draw consumption at zero standard deviation require reconciliation under the accepted stochastic contract.

Exact continuous-time transitions, transformed-state models, block bootstrap and empirically fitted parameters are unimplemented alternatives with different economic assumptions. Do not interpret seeded replay tests as distribution validation.

## Evidence

[Statistical kernels](../../../../finstack-quant/statements/src/forecast/statistical.rs), [dispatch](../../../../finstack-quant/statements/src/forecast/mod.rs), [Monte Carlo caller](../../../../finstack-quant/statements/src/evaluator/monte_carlo.rs). Located inline seeded-repeatability, different-seed, invalid-parameter, exponent-clamp and zero-volatility tests; [forecast tests](../../../../finstack-quant/statements/tests/forecast/forecast_tests.rs) cover method integration. [PCG bibliography](../../../REFERENCES.md#oneill-2014-pcg) identifies the random-family reference, not an independent certification of this seed/stream mapping. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
