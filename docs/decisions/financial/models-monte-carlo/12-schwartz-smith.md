# MODELS-MONTE-CARLO-12: Schwartz–Smith joint Gaussian transition and futures expectation

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in process/schwartz_smith.rs and discretization/schwartz_smith.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

Log commodity spot is X+Y; `dX=(-kappa X-lambda_x)dt+sigma_x dW_x`, `dY=mu_y dt+sigma_y dW_y`. Checked kappa/sigmas are finite positive, rho in [-1,1], mu_y/lambda_x finite. `from_spot` accepts positive spot and optional X0 (default zero), then Y0=ln(S0)−X0. The drifts are caller-supplied pricing parameters; no risk-premium calibration is performed here.

## Algorithm and controls

Exact X mean is `X exp(-kappa dt)-lambda_x*(1-exp(-kappa dt))/kappa`, variance `sigma_x²*(1-exp(-2kappa dt))/(2kappa)`. Y mean advances by mu_y dt. Use step correlation `rho*(1-exp(-kappa dt))/kappa / sqrt(dt*(1-exp(-2kappa dt))/(2kappa))`, clamped to [-1,1], to correlate the two exact Gaussian increments. This differs from simply reusing instantaneous rho for unequal kernels. Scheme correlation is internal; zero dt is a no-op. Exact is default; Euler is available.

`futures_price(tau)` returns `exp(exp(-kappa tau)X0+Y0+mu_y tau-lambda_x*(1-exp(-kappa tau))/kappa+0.5 Var(X_tau+Y_tau))`, including the covariance term. It is an undiscounted spot expectation under the supplied drift convention.

## Limitations and alternatives

The raw constructor accepts unchecked initial coordinates; path simulation validates them. Future-price maturity is not independently validated and direct exponentiation may overflow. Euler discretization trades simpler updates for finite-step bias; no deterministic curve-fitting correction is added.

## Evidence

[Process/futures formula](../../../../finstack-quant/models/src/monte_carlo/process/schwartz_smith.rs), [transition](../../../../finstack-quant/models/src/monte_carlo/discretization/schwartz_smith.rs). `schwartz_smith_log_spot_mean_and_futures` in [simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs) directly checks Gaussian means and transition correlation, but futures agreement partly compares production kernels. [Schwartz–Smith 2000](../../../REFERENCES.md#schwartz-smith-2000) is the canonical economic model reference; a pricing expectation under supplied drifts is not evidence of a calibrated market futures curve.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
