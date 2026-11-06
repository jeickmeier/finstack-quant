# MODELS-NUMERICAL-014: Theta time marching, startup damping and Thomas solves

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and algorithm

[stepper.rs](../../../../finstack-quant/models/src/pde/stepper.rs) marches calendar model time backward from T to zero with positive h=t_from−t_to. Solve (I−θh A_to)u_new=(I+(1−θ)h A_from)u_old plus both endpoint-weighted source/boundary corrections. CN uses θ=1/2, implicit Euler 1 and explicit 0. `custom` clamps finite θ to [0,1]; nonfinite θ fails at stepping. `rannacher(k,N)` uses θ=1 for the first k **full** intervals and θ=1/2 thereafter; it does not split them into halfsteps.

[operator.rs](../../../../finstack-quant/models/src/pde/operator.rs) solves I−αA using unpivoted Thomas elimination and back substitution. Each pivot must be finite and exceed 1e−12·max(|constituent diagonal|,|elimination term|,1); otherwise return `ThomasError`. No pivoting or alternate solver fallback exists. Even θ=0 follows the identity-system Thomas path. RHS finiteness and solution residuals are not independently checked here.

## Selection, time grid and caching

[solver.rs](../../../../finstack-quant/models/src/pde/solver.rs) requires an explicit grid and stepper, positive finite maturity and N>0. Time levels are uniform with exact terminal zero. Initialize interior terminal payoff, march N intervals, apply any obstacle afterward, then reconstruct boundaries. If `is_time_homogeneous` is true, reuse the operator assembled at maturity and refresh boundary scalar RHS data; otherwise assemble both endpoints each step. Caller promises include fixed coefficients, source and boundary type.

## Stability gates, alternatives

For θ<1/2 require h≤dx_min²/(2 max|a|), sampling diffusion at interior nodes and both endpoints/midpoint in time. This is a diffusion-only bound: pure advection has an infinite reported bound, and reaction stiffness is ignored. It is not a sufficient generic explicit stability check, nor does three-time sampling bound arbitrary time-dependent diffusion peaks. θ≥1/2 is not gated. Pivoted tridiagonal solves or adaptive stepping would be new alternatives.

## Evidence

Inline `heat_equation_cn_converges_faster`, `explicit_step_rejects_cfl_violation`, `cfl_bound_uses_max_diffusion_over_step_interval_not_just_t_from`, `thomas_rejects_degenerate_pivot`, and `thomas_accepts_well_conditioned_system` cover heat diffusion and algebra. They do not certify pure-advection/reaction behavior. [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) and [Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes) are generic references; they do not establish startup-damping accuracy for a specific product.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
