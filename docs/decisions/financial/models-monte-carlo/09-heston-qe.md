# MODELS-MONTE-CARLO-09: Heston QE variance, integrated-variance approximation and martingale correction

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in process/heston.rs, discretization/qe_heston.rs and pricer/heston.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

Heston carries spot and instantaneous variance, with correlated spot/variance Brownian drivers. Checked model inputs come from `HestonPricingParams`: finite decimal r/q, strictly positive finite kappa/theta/sigma_v/v0, and rho strictly between -1 and 1; generic process constructors trust the supplied object. Variance v/theta/v0 is annualized volatility squared, kappa is inverse years and time is a year fraction. Default and convenience paths use QE with threshold 1.5 and trapezoidal integrated variance. Explicit Euler uses engine correlation and full-truncation coefficient evaluation. Feller is an inclusive diagnostic, not an enforcement gate.

## Algorithm and selection

1. Sample v_next from the shared [QE kernel](08-cir-qe.md) using z[1]. Use independent z[0] for the orthogonal spot innovation; the scheme marks correlation internal.
2. Approximate integrated variance as I=c0+c1*v+c2*v_next. Trapezoidal uses (0,dt/2,dt/2). `MeanReversionAdjusted` uses c1=c2=(1-exp(-kappa dt))/(2kappa), c0=theta*dt−2theta*c1, reverting to trapezoidal for |kappa dt|<1e-8. `exact_variance()` selects this approximation; it is not an exact integrated CIR path sample.
3. Clamp rho to [-1,1] and I at zero. Define Apre=−c2/2+(rho/sigma_v)(kappa*c2+1), A=Apre+(1-rho²)c2/2. If the QE distribution MGF at A is finite and positive, set K0*=−ln(MGF)−(1-rho²)(c0+c1*v)/2 and log-spot increment to drift_increment+K0*+Apre*v_next+sqrt((1-rho²)I)z[0].
4. If that MGF does not exist, silently use the uncorrected approximation `drift_increment-I/2+(rho/sigma_v)(v_next-v-kappa*theta*dt+kappa*I)+orthogonal_noise`. For |sigma_v|≤1e-10 use the deterministic-variance lognormal step with full spot noise. Exact/QE drift increments may use the optional cumulative drift schedule; Euler drift ignores that schedule.

## Failure behavior and alternatives

The MGF-failure fallback loses the explicit conditional martingale correction; no fallback counter is reported. `MeanReversionAdjusted` changes the approximation, not the underlying model. Exact Heston simulation and Fourier pricing are meaningful alternatives, but are not this stepping algorithm.

## Evidence

[Heston process](../../../../finstack-quant/models/src/monte_carlo/process/heston.rs), [QE spot update](../../../../finstack-quant/models/src/monte_carlo/discretization/qe_heston.rs), [convenience dispatch](../../../../finstack-quant/models/src/monte_carlo/pricer/heston.rs). Located tests include `k0_star_one_step_martingale`, `k0_star_multi_step_martingale_at_high_vol_of_vol`, branch positivity and integrated-variance checks. [Engine tests](../../../../finstack-quant/models/src/monte_carlo/engine/tests.rs) compare Euler/QE at zero rho (cross-method production comparison, not independent truth). [Andersen 2008](../../../REFERENCES.md#andersen-2008-heston-qe), [Heston 1993](../../../REFERENCES.md#heston-1993), and [Broadie–Kaya](../../../REFERENCES.md#broadie-kaya-2006-exact-heston) distinguish model, approximation and exact alternative.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
