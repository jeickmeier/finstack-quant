# VALUATIONS-STRUCTURED-CREDIT-17: OAS scenario discounting and inverse spread

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: tranche clean-price inversion with optional rate and credit shocks.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

OAS has its own simulation model, separate from the main stochastic waterfall engine. It reruns the same deal cashflow engine under rate/credit modulation, caches path cashflows, then solves one spread. Calling the result OAS does not imply it uses the deal's configured copula or all main-MC stochastic specifications.

## Algorithm and numerical settings

Default configuration is 256 scenarios, stochastic credit true, stochastic rates false, kappa 0.05/year, absolute short-rate sigma 0.01, prepayment beta 7, credit loading 0.3, seed 42 and Brent tolerance 1e-7. Path count is capped at 100,000; zero is promoted to one. If both stochastic switches are off, one scenario is used.

Monthly forward rates anchor the short-rate path. A mean-zero OU deviation starts at zero and advances by its exact Gaussian step. Absolute rate is forward plus Hull-White deterministic alpha plus deviation. The path rate shifts floating coupons and prepayment; prepayment responds to `exp(-beta × (rate - initial monthly forward))` with clamps. A single Gaussian credit factor persists over each path: default receives `exp(-loading Z - loading²/2)`, prepayment the opposite sign with the same mean correction. Rate and credit streams are independent; instrument revolvers use another disjoint stream.

Each future cashflow uses the settlement-relative curve DF, multiplied by `exp(-monthly sum of OU deviations)` and the discrete-integral variance correction `exp(-Var/2)`. This correction targets the curve DF in expectation for the simulated monthly grid. Cached pairs `(Act365F settlement time, cash × path DF)` then receive `exp(-spread × time)`. Brent starts at zero with initial ±0.05 bracket size. The result is annual decimal OAS; model/market prices and standard error are percent of current face. Clean target adds accrued once.

## Limitations, failures and alternatives

Monthly forwards are evaluated from the discount curve's time zero, whereas the path is described from `as_of`. For non-base-date valuations this can anchor to the wrong forward segment. For future settlement the stochastic integral runs from `as_of` without dividing out the stochastic settlement DF, while the deterministic curve DF is settlement-relative. These are specific reconciliation issues, not certified no-arbitrage behavior.

Finite-result and solver failures error, but numeric OasConfig parameters do not receive a complete finite/nonnegative-domain validation here. Forward lookup has a zero-rate fallback. Monthly integration, fixed credit factors and mean correction before probability caps all introduce approximation. Deterministic z-spread is a supported limiting alternative; full calibrated joint rate/credit/copula dynamics are not supplied by these switches.

## Implementation and validation evidence

- [structured_credit/metrics/risk/oas.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/oas.rs)
- [structured_credit/pricing/simulation_engine/pool_flow_source.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flow_source.rs)
- [structured_credit/pricing/simulation_engine/instrument_paths.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_paths.rs)

Existing evidence: [structured_credit/unit/metrics/risk_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/risk_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives), [interest-rate modeling](../../../REFERENCES.md#brigo-mercurio-2006-interest-rate-models), [Brent](../../../REFERENCES.md#brent-1973). These references identify the method family; they do not certify the preset parameters or every implementation branch.

Located deterministic-limit and curve-shape tests do not settle those non-base-date/future-settlement cases.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
