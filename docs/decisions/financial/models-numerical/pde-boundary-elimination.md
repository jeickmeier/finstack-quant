# MODELS-NUMERICAL-013: PDE boundary elimination and reconstruction

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Independent affine substitution, stage, corner and range checks executed. Production solver, product risk and host qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked PDE boundary elimination and reconstruction

1. **Use one checked boundary law for each required edge sample.**
   Keep Dirichlet, increasing-coordinate Neumann, Linear and
   LinearInExp. The problem selects the law explicitly on the
   prepared axis and supplies finite signed scalars where required.
   Use one validated relation \(U_e=A U_1+B U_2+d\), with
   \(U_1,U_2\) the first and second nodes inward from the
   edge. The operator and the reconstructed field use the same
   relation and admitted widths. Use a factored form when needed.
   [Prepared spatial grids](pde-grids-interpolation-and-greeks.md#accepted-decision-for-checked-pde-grids-interpolation-and-coordinate-greeks)

2. **Dirichlet fixes the edge ordinate.** For signed value
   \(g\), use \((A,B,d)=(0,0,g)\) and return the admitted
   value at the edge. The scalar has solution-value units. Its
   removed stencil coupling enters the adjacent row's affine
   term exactly once.

3. **Neumann fixes the slope in the increasing axis direction.**
   For slope \(g\) per coordinate unit and actual boundary width
   \(h_b\), use \((A,B)=(1,0)\) and \(d=-h_b g\) on
   the lower edge or \(d=+h_b g\) on the upper edge. This is
   the one-sided difference across that boundary cell. An outward
   normal slope must be converted by the caller. Zero slope extends
   a constant from the adjacent interior value.

4. **Linear continues the inward secant in the grid coordinate.**
   Let \(h_i\) be the actual width between the two inward nodes.
   With \(R=h_b/h_i\), the edge is
   \(U_1+R(U_1-U_2)\), with \((A,B,d)=(1+R,-R,0)\).
   Use the same local ratio at either side.

5. **LinearInExp continues the secant in the exponential coordinate.**
   Use the same affine relation with
   \[
   R_{\rm lower}=\frac{1-e^{-h_b}}{e^{h_i}-1},\qquad
   R_{\rm upper}=\frac{e^{h_b}-1}{1-e^{-h_i}}.
   \]
   Use checked \(\operatorname{expm1}\), scaled or logarithmic
   equivalents. The ratios require local coordinate differences.
   On a log-spot axis the closure extends an affine function of
   spot and imposes zero discrete spot curvature across the
   boundary triple. The financial model owns selection and
   qualification of this far-field approximation.
   [Boundary references](../../../REFERENCES.md#checked-pde-boundary-elimination-and-reconstruction)

6. **Require the selected law's full stencil.** Linear and
   LinearInExp require two interior nodes on the normal axis.
   Remove their automatic constant extension on a three-node
   line. A product can explicitly use Neumann with zero slope
   for a constant extension. Dirichlet and Neumann admit one
   interior node. Apply both edge contributions to that single
   row. Reject a missing inward sample instead of inventing a
   ghost point or switching laws.

7. **Substitute the edge in the spatial row exactly once.** With
   edge coupling \(q\), remove the edge unknown and add \(qA\)
   to the adjacent interior diagonal, \(qB\) to the next
   inward coefficient when present, and \(qd\) to the affine
   generator forcing. This equals evaluation of the uneliminated
   row on the reconstructed field. For the difference form in
   [012](pde-spatial-stencils.md#accepted-decision-for-checked-directional-and-mixed-pde-stencils),
   a Neumann edge contributes \(qd\), and an affine edge combines
   \(-qR\) with the existing inward difference weight. Use the
   time march's own weights for the boundary forcing, with no
   extra factor or split. Preserve signed post-elimination
   coefficients. The pre-elimination neighbor sign rule supplies
   no boundary stability or positivity guarantee.

8. **Evaluate boundary effects at their complete scale.** Check
   required axis geometry, sizes, finite stage values and callback
   results before use. Use stable products, differences and
   combined sums for edge values, eliminated rows and forcing.
   Preserve constant fields and genuine zeros. A detached ratio
   or product can overflow or underflow even when the required
   effect is representable. Return a contextual input, shape,
   callback, resource or range error when admission or the required
   evaluation fails. No width floor, coefficient clamp or partially
   successful field is part of the contract.

9. **Bind boundary samples to the solve, time and phase.** Use the
   actual forward-model time, side, transverse node and applicable
   solver or contractual event phase. Sample each required rule and
   scalar once for the applicable time and phase, using the same
   sample in elimination and reconstruction. Old-time rows use
   old-time data. Receiving rows and the receiving full field use
   receiving-time data. Each mixed-derivative stage reconstructs
   from its own interior field, including startup substeps where
   applicable. Distinct event phases at the same timestamp can have
   different edge laws or values. Records
   [014](theta-time-march-and-thomas.md) and
   [016](mcs-adi-time-march.md) own time weights and stage order.

10. **Use the terminal and final stage interiors.** For a
    positive-horizon solve, initialize terminal interior values
    from payoff and construct required edges at the terminal
    stage from the selected boundary laws. Remove terminal
    edge payoff evaluations that are overwritten before use.
    Terminal edge compatibility and contractual payoff, exercise
    and rebate phases belong to the product. Reconstruct the
    returned full field from the actual final interior, including
    any applicable exercise projection, at the output time.
    Product maturity and settlement branches retain their own
    payoff rules.

11. **Give the x edges explicit corner ownership.** In 2D copy
    the interior, reconstruct the lower and upper y edges at
    interior x knots, then reconstruct the x edges over the
    complete y axis, including corners. At corners the x law
    uses fresh y-edge values for any inward samples. The y
    callback has no corner sample. The x callback may prescribe
    a corner with Dirichlet. Each corner has one declared owner.
    This ordering is a project convention for the truncated
    domain and affects the explicit mixed stencil.

12. **Reuse rows only while their linear forms remain valid.**
    Reuse is scoped to the same solve, prepared grid and admitted
    invariant coefficients and source. A time-varying Dirichlet
    value or Neumann slope refreshes the affine contribution
    when the row's boundary form is unchanged. When the form
    changes, re-eliminate the affected rows or lines from the
    validated original stencil using the current sample. A
    corner-only form change affects reconstruction. Homogeneity of
    the interior coefficients and source gives no permission to
    keep a stale boundary form or to reuse a timestamp-only
    snapshot across a contractual event.

13. **Reconcile the live owners.** Use checked closure and
    reconstruction in equity 1D, Heston 2D and continuous
    and discrete barrier paths, including direct stepping.
    Preserve actual continuous barrier endpoints and dated
    rebate treatment. Discrete monitoring and extraction keep
    their product-selected event phase and qualified domain.
    Grid extent, bridge closure selection, variance-edge
    economics and model admissibility remain product or bridge
    decisions. Keep boundary internals Rust-only. Existing
    pricing and host routes propagate closure and solver failures.
    [PDE model bridges](black-scholes-and-heston-pde-bridges.md)

14. **Qualify the discrete algebra and price accuracy separately.**
    Compare substituted rows with reconstructed full rows on
    asymmetric grids and both sides for each law. Include
    minimum lines, changed boundary types and scalars, corner
    and stage identity, extreme scales, valid negative values,
    invalid inputs and cached-versus-fresh assembly. Qualify
    boundary placement and truncation separately from mesh and
    time error in Black–Scholes far fields, Heston variance edges
    and contractual barrier and rebate cases. A verified local
    closure supplies no general far-field or time-march accuracy
    claim.

## Observed choice and algorithm

[boundary.rs](../../../../finstack-quant/models/src/pde/boundary.rs) defines four edge policies. Dirichlet fixes u=g and moves the missing boundary coupling into the RHS. Neumann is the derivative in the increasing coordinate direction: lower u0=u1−h g, upper uN=uN−1+h g. [operator.rs](../../../../finstack-quant/models/src/pde/operator.rs) eliminates this same one-sided value into the adjacent interior row, matching reconstruction in [solver.rs](../../../../finstack-quant/models/src/pde/solver.rs).

`Linear` extends the slope of the first/last two interior points, u_edge=u1+ratio·(u1−u2), ratio=h_boundary/h_interior. `LinearInExp` uses the exponential-coordinate spacing ratio: lower (1−exp(−hb))/(exp(hi)−1); upper (exp(hb)−1)/(1−exp(−hi)), evaluated with `expm1`. This extends an affine function of exp(x), appropriate for log-spot far fields. With only one interior node both variants extend a constant.

## Selection, ordering and controls

Problem callbacks provide edge types and scalar values at each evaluation time. Homogeneous operator caches refresh scalar Dirichlet/Neumann RHS data; this assumes fixed boundary type and fixed operator coefficients/source. [adi.rs](../../../../finstack-quant/models/src/pde/adi.rs) fills interior y edges first, then all x edges including corners, so the cross stencil consumes fresh edge values. The x-edge rule therefore has corner precedence if supplied edge conditions conflict.

## Limitations, alternatives and evidence

Neumann uses first-order one-sided boundary differences; linear/exponential extension is a closure assumption, not a remote exact boundary. The model bridge chooses these automatically, but domain size remains the caller's responsibility. Time-varying boundary types or source under a declared homogeneous problem are incompatible with cache reuse. Higher-order ghost-node, exact asymptotic or degenerate-PDE boundary closures would be different choices.

- On log knots \((0,\ln 2,\ln 8,\ln 16)\),
  \(U=3+2e^x\) has interior values 7 and 19. The
  exponential ratios \(1/6\) and \(4/3\) reconstruct 5
  and 35. Substitution preserves exact lower and upper row
  actions 40 and −2 for test rows with
  (edge, diagonal, inward) weights \((6,-4,2)\) and
  \((3,-6,1)\), respectively. The upper inward coefficient
  becomes −3. On \((0,1,4)\) with one interior value 2,
  lower Neumann slope −3 and upper Dirichlet value 9,
  the edges are 5 and 9. The row \((2,-5,3)\) with
  source 7 gives 34 in either form.
- On \((0,1,2)\), a one-step Crank–Nicolson heat example
  from time 1 to 0 starts at interior value 3 with old left
  Dirichlet 2 and right Dirichlet 0. Switching the left law
  to coordinate Neumann slope 1 at time 0 gives interior
  value \(1/3\). A stale Dirichlet row using 1 as the new
  boundary value gives \(3/4\).
- On \(x=(0,1,3)\), \(y=(0,2,5)\), hold the interior
  value at 7. Set the y edge values to \(2+t\) and
  \(8+2t\), the lower x slope to \(1+y+t\), and
  the upper x value to \(9+2y+(1+y)t\). The mixed
  tensor derivative is \(9/10\) at time 1 and
  \(131/180\) at time 0. Reusing the old y values
  at the left corners gives an incorrect \(3/4\) at
  time 0.
- An upper exponential ratio at \(h_b=750,h_i=1\)
  exceeds binary64, while its product with the minimum
  positive subnormal is about 411.0041. A lower Neumann
  edge with \(h_b=2\), \(g=U_1=2^{1023}\) has
  finite value \(-2^{1023}\) even though \(h_b g\)
  overflows.

Inline `linear_boundaries_preserve_affine_profile_on_nonuniform_grid`, `neumann_preserves_linear_profile_on_nonuniform_grid`, `time_homogeneous_cache_refreshes_time_varying_boundaries_1d`, and the matching 2D tests exercise algebra and cache invariants. [bridge.rs](../../../../finstack-quant/models/src/pde/bridge.rs) includes `log_spot_far_field_converges_to_deep_itm_call_value`, a BS-formula comparison. General [Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes) does not supply an adoption decision for these financial boundaries.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
