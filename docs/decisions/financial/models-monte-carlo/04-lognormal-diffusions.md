# MODELS-MONTE-CARLO-04: GBM, curve-implied interval coefficients and local volatility

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in process/gbm.rs, process/piecewise_gbm.rs, process/local_vol.rs and discretization/exact.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and input domain

Single/multiasset GBM uses `dS=(r-q)S dt+sigma S dW`; finite decimal rates/yields and nonnegative annualized volatility are accepted by checked parameters. Local-volatility diffusion replaces sigma by a supplied `LocalVolSurface.value(t,S)`, keeping constant r−q drift. Surface construction/Dupire extraction is delegated to the volatility domain. Convenience APIs require strictly positive volatility, even though generic GBM supports zero.

## Algorithm and selection

Exact GBM multiplies spot by `exp(integrated_drift - sigma² dt/2 + sigma sqrt(dt) z)`. An optional cumulative drift schedule is linearly interpolated and linearly extrapolated using endpoint slopes; exact stepping takes its endpoint difference. Generic `drift()` still uses constant r−q, so choosing Euler/Milstein ignores this optional schedule. Multiasset exact stepping recovers proportional drift/diffusion coefficients by dividing by state; zero remains absorbing; subnormal states are probed at signed unit level to avoid underflow. Correlation is applied once by the engine.

Piecewise GBM chooses the interval after a knot on equality and extrapolates the last interval. Its exact step freezes that interval through the whole step: callers must align all coefficient boundaries. `bootstrap_forward_gbm` starts at time zero/DF=1, skips nonincreasing checkpoints, computes `r_i=ln(DF_prev/DF_i)/dt`, queries sigma at `(T,S0 exp(-qT)/DF(T))`, differences total variances `sigma(T)²T`, and sets forward sigma to the square root of incremental variance/dt. Nonpositive/nonfinite DFs, dt≤1e-6 and decreasing total variance error.

Default path-simulation selection is exact GBM/multi-GBM, and log-Euler local vol; Euler is an explicit local-vol alternative and Milstein is rejected there. Exact lognormal transitions avoid time-step distribution error for constant coefficients, but monitoring remains grid-dependent.

## Limitations and alternatives

Piecewise structs expose unvalidated vectors and the bootstrap does not independently check callback volatility finiteness/sign; an empty/skipped schedule can leave no intervals. Exponential overflow/underflow is not silently capped; engine/final-state checks catch nonfinite results but a zero underflow can survive. Alternatives include deterministic coefficient integration across knots and local-vol higher-order schemes; neither is this piecewise kernel. Negative-state subnormal use is outside the positive-spot simulation domain.

## Evidence

[GBM](../../../../finstack-quant/models/src/monte_carlo/process/gbm.rs), [piecewise bootstrap](../../../../finstack-quant/models/src/monte_carlo/process/piecewise_gbm.rs), [exact schemes](../../../../finstack-quant/models/src/monte_carlo/discretization/exact.rs), [local vol](../../../../finstack-quant/models/src/monte_carlo/process/local_vol.rs). Tests locate drift-schedule interpolation, boundary interval selection, tiny-state preservation, flat/skew local-vol repricing. [Simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs) compare GBM log moments and correlated log returns to direct formulas; local-vol repricing compares another production formula/surface. [Black–Scholes](../../../REFERENCES.md#black-scholes-1973) and [Dupire](../../../REFERENCES.md#dupire-1994) are canonical model references, not approval of bootstrap/extrapolation policies.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
