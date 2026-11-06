# MODELS-MONTE-CARLO-05: Euler, log-Euler, proportional Milstein and Gaussian processes

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in discretization/euler.rs, discretization/milstein.rs, process/brownian.rs and process/multi_ou.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

The generic diagonal steppers are alternatives selected explicitly, with `simulate_paths` restricting valid process/scheme combinations. Additive Brownian motion uses constant drift mu and diffusion sigma; multi-OU uses independent-coordinate drifts `kappa_i(theta_i-x_i)` with optional driver correlation. Brownian sigmas are finite/nonnegative; multi-OU kappas/thetas must be finite but kappas may be negative or zero.

## Algorithm and controls

Euler evaluates coefficients at the left endpoint and applies `x_i += mu_i dt + b_i sqrt(dt) z_i`. For constant additive coefficients this is the exact increment; multi-OU default is Euler, not an exact OU transition. Log-Euler uses `x_safe=max(x,MIN_POSITIVE)`, divides drift/diffusion by it, and multiplies by `exp((mu/x_safe-(b/x_safe)²/2)dt+(b/x_safe)sqrt(dt)z)`. Milstein is restricted by `ProportionalDiffusion`, adds `0.5 b b' (z²-1)dt`, and estimates `b'=b/x` only when |x|>1e-10, else zero. The engine supplies correlated normals; these loops assume coordinatewise diffusion, not arbitrary matrix diffusion. LMM therefore requires a dedicated scheme.

## Conventions, failure and alternatives

Mu has state units/year, additive sigma state units/sqrt(year), kappa inverse years. Euler does not enforce positivity or stability; CIR/Heston coefficient evaluation truncates negative variance before the square root but Euler output may still be negative. Log-Euler does not convert nonpositive starting values into valid positive states. No adaptive step control exists here. Exact GBM and Hull–White are supported alternatives; an exact correlated multi-OU transition is not selected by this API.

## Evidence

[Euler/log-Euler](../../../../finstack-quant/models/src/monte_carlo/discretization/euler.rs), [Milstein](../../../../finstack-quant/models/src/monte_carlo/discretization/milstein.rs), [Brownian](../../../../finstack-quant/models/src/monte_carlo/process/brownian.rs), [multi-OU](../../../../finstack-quant/models/src/monte_carlo/process/multi_ou.rs). [Simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs) directly check Brownian moments/correlation and the discrete OU mean `theta+(x0-theta)(1-kappa dt)^N`; these verify the implemented approximation rather than continuous-time OU accuracy. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) is the available numerical reference; no separate convergence certification was found.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
