# STATEMENTS-EVALUATION-007 — Normal, lognormal, mean-reverting and bootstrap forecasts

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice, scope and units

Statistical forecasts are discrete per-model-step processes using caller-supplied parameters, not fitted continuous-time processes. Normal/MR drifts and standard deviations have the node's level units; lognormal drift/volatility are dimensionless per-step log-process inputs. No actual-duration scaling or automatic annual-to-step conversion is performed. Bootstrap samples caller history, which need not share the model's dated history.

## Algorithms and random controls

The implementation uses the core PCG64 random generator. Independent node seeds mix the configured seed by multiplication with a fixed golden-ratio constant plus a stable node hash; the hash is FNV-1a followed by SplitMix mixing. Monte Carlo adds an explicit stream identity. Repeated seeded calls are intended to be replayable; this does not itself establish statistical suitability.

1. **Normal:** `x[t]=x[t-1]+mu+sigma*z[t]`, with finite base/mu, finite `sigma>=0` and standard normal draws. Negative base and outcomes are allowed. Each result must be finite.
2. **Lognormal:** require positive finite initial base and finite mu/`sigma>=0`; form `e=mu-0.5*sigma^2+sigma*z`, clamp `e` to `[-709,709]`, then `x[t]=x[t-1]*exp(e)`. Clamping is an observed tail repair with an advisory, not an accepted distributional approximation. Nonfinite outputs error; underflow to zero remains finite and can become absorbing. The public label “always positive” is therefore stronger than the enforced floating-point domain at extremes.
3. **Mean reverting:** `x[t]=x[t-1]+k*(theta-x[t-1])+sigma*z`, with finite base/theta, `sigma>=0` and `k in (0,1]`. It is a discrete additive recursion, not the exact transition of a continuous-time Ornstein–Uhlenbeck model. Negative levels are permitted.
4. **Bootstrap:** require at least two finite historical levels. Growth sampling requires strictly positive historical levels and builds increments `r[i]=h[i+1]/h[i]-1`; difference sampling uses `d[i]=h[i+1]-h[i]`. Draw one increment independently with replacement using uniform index `floor(u*n).min(n-1)`, then compound or add recursively. Base need only be finite even for growth mode. Nonfinite generated results error. There is no block resampling or serial-dependence model.

Normal/lognormal/MR still consume random draws at zero sigma but store zero standardized shock for correlation. Bootstrap does not provide those latent normal shocks. Optional output bounds are applied only after generation, as described in [record 05](05-forecast-segments-and-bounds.md).

## Selection, limitations and alternatives

Explicit forecast-method and typed-parameter dispatch controls selection; no model family is selected automatically from the node's economic meaning. The generic module prose that all statistical methods reject negative bases does not match Normal/MR implementation. Clipped lognormal exponents, underflow absorption, per-step drift conventions and lack of fitted calibration limit the implemented model domain. They do not imply that users agreed to those approximations.

Exact continuous-time transitions, transformed-state models, block bootstrap and empirically fitted parameters are unimplemented alternatives with different economic assumptions. Do not interpret seeded replay tests as distribution validation.

## Evidence

[Statistical kernels](../../../../finstack-quant/statements/src/forecast/statistical.rs), [dispatch](../../../../finstack-quant/statements/src/forecast/mod.rs), [Monte Carlo caller](../../../../finstack-quant/statements/src/evaluator/monte_carlo.rs). Located inline seeded-repeatability, different-seed, invalid-parameter, exponent-clamp and zero-volatility tests; [forecast tests](../../../../finstack-quant/statements/tests/forecast/forecast_tests.rs) cover method integration. [PCG bibliography](../../../REFERENCES.md#oneill-2014-pcg) identifies the random-family reference, not an independent certification of this seed/stream mapping. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
