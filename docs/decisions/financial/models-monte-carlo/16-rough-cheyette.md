# MODELS-MONTE-CARLO-16: Cheyette rough-volatility rate state and deterministic shift

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in process/cheyette_rough.rs and discretization/cheyette_rough.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

The model evolves x and auxiliary y, publishing short rate `r=x+phi(t)`. Kappa and eta are positive; rho lies in [-1,1]. Phi is a mandatory finite ordered nonnegative-time curve, linearly interpolated with flat tails. Although `sigma_base` uses the `ForwardVarianceCurve` container, its value is used directly as a volatility amplitude, and its square enters y drift. This unit distinction must be preserved.

## Algorithm and controls

With left-endpoint fractional level Y, reconstruct `sigma_t=sigma_base(t)*exp(eta*Y/2-eta²*t^(2H)/4)`. Correlate the rate normal with the generator's driving normal, then Euler-step `x+=(y-kappa*x)dt+sigma_t sqrt(dt)Z` and `y+=(sigma_t²-2kappa*y)dt`. Only afterward add the fractional increment to stored Y. Finite negative reconstructed sigma is floored; nonfinite values propagate. Dedicated scheme and injected-noise entry are mandatory; ordinary drift/diffusion are zero placeholders. Default generator is RL Volterra; stationary/finite-window fBM choices change covariance as described in [record 13](13-fractional-noise.md).

## Limitations and alternatives

Y compensation targets E[sigma²]=sigma_base² only under the assumed driver level variance. Euler y is not clamped and can become negative on coarse steps; there is no stability restriction `2kappa dt≤1`. Phi is supplied, not calibrated to a discount curve here. Published simulation coordinates are x/y; payoff short-rate projection adds phi separately. Exact conditional x/y integration or Markovian rough lifts are unimplemented alternatives.

## Evidence

[Process](../../../../finstack-quant/models/src/monte_carlo/process/cheyette_rough.rs), [stepper](../../../../finstack-quant/models/src/monte_carlo/discretization/cheyette_rough.rs); located tests cover interpolation, left-endpoint sigma, variance mean and zero-mean stochastic increments. [Non-Markov tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests/non_markov.rs) explicitly exclude the short-window case from compensated-mean assertions. [Gatheral–Jaisson–Rosenbaum](../../../REFERENCES.md#gatheral-jaisson-rosenbaum-2018) is rough-volatility context, not a derivation of this Cheyette extension. A direct Cheyette rough-model primary reference, pricing benchmark and adoption rationale are missing.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
