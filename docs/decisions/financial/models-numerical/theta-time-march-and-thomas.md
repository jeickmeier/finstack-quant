# MODELS-NUMERICAL-014: Theta time marching, startup damping and Thomas solves

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Independent exact-arithmetic, binary64 range, startup and event checks executed. Production solver, price, risk and host qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked theta marching and tridiagonal solves

1. **Use the checked prepared model-time grid.** A 1D solve
   requires a finite positive horizon \(T\) and at least one
   actual interval. Use the canonical \(0\)-to-\(T\) knots
   and reverse traversal from
   [CORE-NUMERICS-20](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots).
   A uniform request supplies its base count explicitly. A caller
   can supply an admitted nonuniform grid or the exact union with
   required events. Use actual adjacent widths
   \(h=t_{\rm old}-t_{\rm new}>0\), including on a uniform
   grid. Check counts and storage before allocation. Remove the
   separate clamped backward grid construction. Expiry or
   zero-horizon payoff belongs to the product.

2. **Keep exact event knots and separate contractual identity.**
   The product prepares event times and confirms eligibility
   on its model clock. Exact grid lookup associates eligible
   events to nodes. Distinct represented times stay distinct,
   while multiple events at a node retain their contractual
   identities and order. Remove epsilon-based union and
   matching in the live discrete barrier and 1D exercise paths.
   A requested event missing from the prepared grid fails
   explicitly. Neither the theta step nor the grid infers
   payoff order, observation status, or exercise eligibility.

3. **Select finite theta in the closed interval \([1/2,1]\).**
   Crank–Nicolson is \(1/2\) and backward Euler is 1.
   A custom value must satisfy the same finite range at
   admission. Remove clamping, explicit Euler and the other
   below-half variants. The live equity and barrier paths
   use only the retained range. Remove the diffusion-only CFL
   check and its claims about unrestricted coefficients.

4. **Use the actual endpoint operators and affine data.** For
   the reduced spatial matrices \(A_o,A_n\) and affine terms
   \(s_o,s_n\), where each \(s\) is source plus boundary
   forcing, set \(\alpha=\theta h\), \(\beta=(1-\theta)h\)
   and solve
   \[
   (I-\alpha A_n)u_n=(I+\beta A_o)u_o+\beta s_o+\alpha s_n.
   \]
   Subscripts denote the departing and receiving forward-model
   times on the backward march. Use each actual endpoint
   and phase under the accepted spatial and boundary contracts.
   Backward Euler has exactly zero departing weight. Skip
   that operator and affine sample when the stage has no
   other use for them. Apply the receiving affine term once.
   Preserve the scale of complete weighted contributions.
   [Spatial operator](pde-spatial-stencils.md#accepted-decision-for-checked-directional-and-mixed-pde-stencils),
   [Boundary law](pde-boundary-elimination.md#accepted-decision-for-checked-pde-boundary-elimination-and-reconstruction)

5. **Keep the live full-interval startup under the name damped
   Crank–Nicolson.** An explicit integer \(K\) selects the first
   \(K\) full prepared intervals for backward Euler, followed by
   Crank–Nicolson. Require \(0\le K\le N\) for the actual
   prepared interval count \(N\). Zero is plain
   Crank–Nicolson and \(K=N\) starts with all intervals
   implicit. The startup uses the same knots and no halfsteps.
   Remove the old Rannacher name for this 1D profile.
   Keep four full startup intervals for the existing 1D
   equity route and two for the existing barrier route on
   grids that admit them. Treat these counts as product
   configurations subject to qualification. The separate 2D
   startup scheme belongs to [016](mcs-adi-time-march.md).
   [Startup analysis](../../../REFERENCES.md#checked-theta-marching-and-tridiagonal-solves)

6. **Restart damping at explicitly designated projections.**
   Reach an event with its incoming numerical interval, apply
   the owner-selected projection in contractual order, then
   reset the damping count when that event designates a
   restart. Terminal initialization starts the first window.
   Discrete barriers restart after their eligible monitoring
   projections. A subsequent restart supersedes the previous
   window. Stop at the final actual interval if fewer than
   \(K\) remain. Preserve the event-specific boundary sample
   at each side. Additional exercise projection, timing and
   damping choices belong to
   [015](pde-exercise-projection.md) and the product.

7. **Use one interval method through the live pricing routes.**
   Reconcile ordinary Solver1D use and the discrete barrier
   direct march with the same checked step and exact grid.
   In a paired barrier calculation that uses common
   discretization for numerical parity, use the same
   prepared time knots and interval weights for the paired
   claims. Apply monitoring projections to the applicable
   claims only. Replace the epsilon next-observation and
   zero-lag repair with the product's actual selected event
   and phase. The product owns dates, monitoring at
   valuation or expiry, rebate economics and parity
   acceptance. Keep existing pricing model keys.

8. **Check all stage inputs and publish a complete step.** Validate
   finite admitted times and actual width, grid and vector
   dimensions, finite old values, coefficients, edge data
   and required outputs. Reuse validated work storage and
   admitted endpoint rows only under 012 and 013. Carry
   initial and final interiors and boundary reconstruction
   under 013. Propagate callback, grid, allocation, arithmetic
   and solve errors with interval and applicable row context.
   A failed step leaves its input field intact and supplies
   no accepted next state.

9. **Use a single checked unpivoted Thomas kernel.** Solve the
   actual tridiagonal stage system with forward elimination
   and back substitution in linear work. For the stage matrix
   bands \(\ell_i,d_i,r_i\) and RHS \(b_i\), initialize
   \(p_0=d_0,z_0=b_0\). For successive rows use
   \(w_i=\ell_i/p_{i-1}\),
   \(p_i=d_i-w_i r_{i-1}\) and
   \(z_i=b_i-w_i z_{i-1}\). Back-substitute
   \(u_i=(z_i-r_i u_{i+1})/p_i\), omitting the absent
   last neighbor. Use nonempty exact lengths, finite stage
   inputs and reusable checked scratch. Use the same kernel
   for ADI line solves. The ADI march retains its own time
   and stage choices.
   [Tridiagonal elimination](../../../REFERENCES.md#checked-theta-marching-and-tridiagonal-solves)

10. **Require finite resolved nonzero computed pivots.** Remove the
    relative pivot magnitude floor with its unit minimum.
    Use checked fused or scaled equivalents for the
    stage bands, Schur updates, eliminated RHS and
    back-substitution. Distinguish numerical range failures
    from a resolved zero pivot. A tiny representable pivot
    can pass the output check. A positive step and weight
    can have a nonzero stage effect even if their detached
    product rounds to zero. A zero pivot is failure of
    the selected unpivoted factorization. Return the failure
    with row context and use no automatic solver switch.

11. **Require a componentwise backward-error check.** For the
    assembled stage matrix \(B\), original RHS \(b\)
    and returned candidate \(u\), compute
    \[
    E=\max_i\frac{|b_i-\sum_j B_{ij}u_j|}
                       {|b_i|+\sum_j |B_{ij}u_j|}.
    \]
    Only the actual tridiagonal row entries participate.
    Use zero for an exactly zero numerator and denominator.
    Evaluate the complete scaled residual and denominator
    before comparison, without a unit or small-denominator
    floor. Require finite candidate and
    \(E\le64\,\epsilon_{64}\) inclusively, where
    \(\epsilon_{64}=2^{-52}\). This fixed tolerance is a
    project acceptance choice for the linear solve. A small
    residual does not establish uniqueness, conditioning,
    forward error or correctness of the discretized PDE.
    [Componentwise residual provenance](../../../REFERENCES.md#checked-theta-marching-and-tridiagonal-solves)

12. **Qualify time stability and accuracy in the selected regime.**
    The standard scalar theta amplification has the usual
    A-stability property for the retained theta range on the
    dissipative test equation. Crank–Nicolson can still
    oscillate. Signed reaction and post-elimination rows can
    yield amplification, negative coefficients or singular
    implicit systems. Preserve the admitted spatial economics
    and propagate detected factorization and arithmetic
    failures. Avoid a general unconditional stability,
    positivity or convergence claim for the assembled
    product solver.

13. **Verify the march, solve and live product routes.** Qualify
    exact event order, restart, source and boundary endpoint
    weights, zero-weight branches, time extremes, positive
    reaction, stiff decay, tiny pivots and residual rejection.
    Check product startup counts with independent price and
    risk references for smooth and kinked payoffs, barrier
    observation and rebate phases, and American exercise
    under its owner. Separate time, spatial, domain and
    solve error. Keep generic stepper and linear solve state
    in Rust. Propagate pricing failures through existing
    Python and WASM routes.

## Observed choice and algorithm

[stepper.rs](../../../../finstack-quant/models/src/pde/stepper.rs) marches calendar model time backward from T to zero with positive h=t_from−t_to. Solve (I−θh A_to)u_new=(I+(1−θ)h A_from)u_old plus both endpoint-weighted source/boundary corrections. CN uses θ=1/2, implicit Euler 1 and explicit 0. `custom` clamps finite θ to [0,1]; nonfinite θ fails at stepping. `rannacher(k,N)` uses θ=1 for the first k **full** intervals and θ=1/2 thereafter; it does not split them into halfsteps.

[operator.rs](../../../../finstack-quant/models/src/pde/operator.rs) solves I−αA using unpivoted Thomas elimination and back substitution. Each pivot must be finite and exceed 1e−12·max(|constituent diagonal|,|elimination term|,1); otherwise return `ThomasError`. No pivoting or alternate solver fallback exists. Even θ=0 follows the identity-system Thomas path. RHS finiteness and solution residuals are not independently checked here.

## Selection, time grid and caching

[solver.rs](../../../../finstack-quant/models/src/pde/solver.rs) requires an explicit grid and stepper, positive finite maturity and N>0. Time levels are uniform with exact terminal zero. Initialize interior terminal payoff, march N intervals, apply any obstacle afterward, then reconstruct boundaries. If `is_time_homogeneous` is true, reuse the operator assembled at maturity and refresh boundary scalar RHS data; otherwise assemble both endpoints each step. Caller promises include fixed coefficients, source and boundary type.

## Stability gates, alternatives

For θ<1/2 require h≤dx_min²/(2 max|a|), sampling diffusion at interior nodes and both endpoints/midpoint in time. This is a diffusion-only bound: pure advection has an infinite reported bound, and reaction stiffness is ignored. It is not a sufficient generic explicit stability check, nor does three-time sampling bound arbitrary time-dependent diffusion peaks. θ≥1/2 is not gated. Pivoted tridiagonal solves or adaptive stepping would be new alternatives.

## Evidence

- For one interior point with unit widths, zero Dirichlet
  edges, zero diffusion and positive drift 3, the spatial
  diagonal is −3. An explicit step of width 1 amplifies
  the input by −2 despite an infinite diffusion-only CFL
  bound. Reaction −3 alone does the same. Crank–Nicolson
  at diagonal −3 gives −1/5, and at diagonal +2 its
  receiving matrix is singular.
- For scalar decay \(h\lambda=-6\), Crank–Nicolson,
  one full backward-Euler interval and two equal
  backward-Euler halfsteps give amplification
  \(-1/2,1/7,1/16\), respectively. On the four
  equal intervals from 1 to 0 with \(\lambda=-2\),
  terminal value 1, \(K=1\) and a +1 projection at
  \(1/2\), restarting gives \(14/25\). Continuing
  without a restart gives \(63/125\).
- The exact knot union retains both 0.5 and its next
  greater binary64 value. Repeated events at the latter
  coordinate share the same index and keep their identities.
- A scalar system with pivot and RHS \(2^{-40}\) solves
  to 1 despite the observed pivot cutoff. With
  \(\alpha=1+2^{-27}\) and \(A_{00}=1-2^{-27}\),
  the complete diagonal is \(2^{-54}\) even though
  the detached product rounds to 1. With
  \(\theta=1/2,h=2^{-1074},A_{00}=2^{1023}\),
  the implicit adjustment is \(2^{-52}\) despite
  detached \(\theta h\) rounding to zero.
- The nonsingular matrix \(\begin{pmatrix}0&-1\\-1&1\end{pmatrix}\)
  with RHS \((2,3)\) has solution \((-5,-2)\) and
  a first-pivot breakdown under Thomas. Replacing the
  leading diagonal with \(2^{-54}\) and RHS with
  \((-1,0)\) gives nonzero pivots but ordinary
  binary64 elimination can return \((0,1)\). Its
  componentwise backward error is 1, so the proposed
  residual gate rejects it.

Inline `heat_equation_cn_converges_faster`, `explicit_step_rejects_cfl_violation`, `cfl_bound_uses_max_diffusion_over_step_interval_not_just_t_from`, `thomas_rejects_degenerate_pivot`, and `thomas_accepts_well_conditioned_system` cover heat diffusion and algebra. They do not certify pure-advection/reaction behavior. [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) and [Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes) are generic references; they do not establish startup-damping accuracy for a specific product.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
