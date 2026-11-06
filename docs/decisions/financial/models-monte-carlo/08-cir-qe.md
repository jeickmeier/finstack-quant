# MODELS-MONTE-CARLO-08: CIR/CIR++ dynamics and quadratic-exponential sampling

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in process/cir.rs, discretization/qe_common.rs and discretization/qe_cir.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

CIR uses `dx=kappa(theta-max(x,0))dt+sigma sqrt(max(x,0))dW`, with checked kappa/sigma>0 and theta≥0. Feller failure logs a warning and is allowed. CIR++ adds a deterministic piecewise constant shift to the published rate, while the internal state remains the unshifted factor. Default simulation uses QE; Euler is an explicit nonpositivity-preserving alternative.

## Algorithm and controls

Compute exact conditional mean m and variance s² with `exp(-kappa dt)` and `expm1`-stable immigration. If m or s² equals zero, return m. Otherwise psi=s²/m². For psi≤psi_c use `(sqrt(m-a)+sqrt(a)z)²`, where `a=m psi/[2(1+sqrt(1-psi/2))]`. Otherwise use a point mass p at zero plus exponential tail: `beta=2/(s²/m+m)`, `p=1-m beta`; u=normal-CDF(z), capped below one; return zero if u≤p, else `ln((1-p)/(1-u))/beta`, floored at zero. Default threshold=1.5; override must be in [1,2]. Shared code also exposes exact exponential moments of this approximating conditional distribution for Heston.

CIR++ rate is x+shift(t), using the knot effective on equality and flat extrapolation. `simulate_paths` accepts the actual starting rate, subtracts initial shift, simulates the factor, and adds the shift back to every reported time. Low-level process stepping expects the unshifted factor.

## Failure behavior and alternatives

QE preserves nonnegative states but is a moment-matched approximation, not exact noncentral-chi-square simulation. Negative incoming x is truncated before moments. Feller failure is permitted, and zero mass is meaningful. Shift calibration is not performed here, and positive factor does not imply positive shifted rate. Alternatives include exact CIR sampling and smaller-step Euler; only the latter is exposed in the simulation dispatcher.

## Evidence

[Process](../../../../finstack-quant/models/src/monte_carlo/process/cir.rs), [QE moments and branches](../../../../finstack-quant/models/src/monte_carlo/discretization/qe_common.rs), [CIR adapter](../../../../finstack-quant/models/src/monte_carlo/discretization/qe_cir.rs). Located tests check both conditional moments, point mass, inverse mixture CDF, tiny means, near-deterministic moments, and exact equality of Heston/CIR variance kernels (shared-kernel comparison). [Simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs) use explicit CIR means and shift addition. [Andersen QE](../../../REFERENCES.md#andersen-2008-heston-qe) is a canonical algorithm reference; the located tests do not establish parameter-wide weak-error accuracy.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
