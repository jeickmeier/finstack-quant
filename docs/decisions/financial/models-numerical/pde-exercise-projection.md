# MODELS-NUMERICAL-015: Grid-based PDE early-exercise projection

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Independent projection, event, interpolation and Bermudan quadrature checks executed. Production American and Bermudan pricing, risk and host qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked grid-based PDE exercise

1. **Use explicit payoff projection for the 1D profile.** The solver
   first computes the accepted theta continuation over an actual
   interval. At an eligible receiving node use the componentwise
   maximum of continuation and the admitted exercise payoff. The
   American method is a discrete stopping approximation over the
   prepared time grid. The Bermudan method applies the comparison
   only at its qualified exercise nodes. The projection is a
   separate stage from the linear solve. Coupled complementarity
   pricing would require its own method and qualification.
   [Explicit payoff method](../../../REFERENCES.md#checked-grid-based-pde-exercise)

2. **Bind one fixed exercise payoff to the spatial grid.** This
   profile uses a single time-independent payoff vector indexed by
   the exact interior knots. The owner supplies the holder's
   immediate exercise value in the same units and at the same
   phase as continuation. Check the entire shape and all finite
   signed entries before solving. Generic signed entries are valid.
   An owner with time-varying, path-dependent or differently
   phased exercise amounts needs a separately qualified route.
   The generic solver supplies no payoff, rebate or settlement
   interpretation.

3. **Select only admitted receiving knots.** American exercise in
   this profile compares at every receiving knot in \([0,T)\),
   including valuation at zero. The owner must qualify that
   exercise set for the instrument. Bermudan exercise requires a
   nonempty set of distinct exact receiving knots in \([0,T)\).
   It compares at zero only when explicitly selected. Admit the
   schedule before stepping and reject nonfinite, out-of-horizon
   or missing knots, and repeated entries for the same exercise
   decision. Remove epsilon matching. The owner may prepare an
   exact required-time union before solving.
   [Checked theta event grid](theta-time-march-and-thomas.md#accepted-decision-for-checked-theta-marching-and-tridiagonal-solves)

4. **Let the product resolve contractual phases and expiry.** The
   positive-horizon solver initializes the terminal state from the
   owner's terminal payoff. Keep \(T\) out of the early schedule.
   Include any qualified same-time terminal rights in the owner's
   terminal event treatment. At preterminal times preserve event
   identities and owner order for exercise, cash, settlements and
   other events sharing a knot. This profile consumes one prepared
   comparison per exercise knot. For example, with continuation
   4, exercise payoff 5 and cash 2, the answer is 6 if only
   continuation receives the cash and 7 if both branches receive
   it. A coordinate match supplies no contractual entitlement or
   order.

5. **Apply the exact maximum with strict exercise preference.** For
   each interior node, let \(C_i\) be its checked continuation
   and \(G_i\) its exercise payoff. Use \(G_i\) when
   \(G_i>C_i\), and keep \(C_i\) otherwise. At equality
   retain continuation and treat the holder as indifferent.
   Copy the selected ordinate directly, including its signed zero.
   This satisfies the discrete obstacle exactly at every eligible
   interior node. Remove penalty factors, iteration counts,
   obstacle shortfall tolerance and step width from the projection.
   Validate continuation and prepare any diagnostics before
   publishing the complete stage.

6. **Keep damping an explicit time-march decision.** Use the
   accepted full-interval startup and apply exercise after each
   eligible incoming step. An ordinary American comparison does
   not restart damping. A designated discrete discontinuity can
   restart it only under the owner's prepared event policy and
   record 014. A solver failure cannot be replaced by an exercise
   payoff before the continuation comparison.

7. **Report adjacent-node exercise brackets.** Compare the
   unprojected values with the exercise payoff. Record every
   adjacent interior pair whose strict \(G>C\) flag differs,
   with the actual receiving time, both knot endpoints and the
   side that strictly prefers exercise. Ties belong to the side
   that retains continuation. Report in backward time order and
   increasing spatial order within a time. A single-state field
   or a one-interior-node grid has no bracket. A bracket localizes
   a discrete preference change and does not supply an interpolated
   or continuous free boundary, an observed exercise or a Greek.

8. **Reconstruct spatial edges from the projected interior.** Use
   the same admitted boundary laws and actual phase as in
   [013](pde-boundary-elimination.md#accepted-decision-for-checked-pde-boundary-elimination-and-reconstruction).
   Boundary nodes do not independently undergo an obstacle
   comparison in this profile. The financial owner must qualify
   compatibility of the selected domain and closure with the
   exercise economics. A failed edge or interpolation check
   remains an error.

9. **Check eligible immediate exercise at the actual valuation
   spot.** The equity owner first obtains the checked unit value
   from the solved grid under
   [011](pde-grids-interpolation-and-greeks.md#accepted-decision-for-checked-pde-grids-interpolation-and-coordinate-greeks).
   At a valuation phase that allows immediate exercise, compare
   it with the checked literal unit exercise payoff at the actual
   spot. Use the greater value before applying signed quantity
   and converting to Money. Grid interpolation alone can fall
   below the actual payoff between knots. At an ineligible
   valuation phase use the grid result from the admitted exercise
   schedule. This comparison does not repair out-of-domain queries
   or validate continuation accuracy.

10. **Reconcile existing product and risk routes.** Use the
    projection in live positive-volatility American equity PDE
    pricing with qualified continuous carry. Generic Bermudan
    remains available in the Rust solver. The equity PDE model
    keeps its existing unsupported live Bermudan and future
    discrete-dividend American cases. Preserve the owner paths
    for observed exercise, settled or expired claims and
    deterministic zero volatility. Market Greeks continue to
    use the selected pricing model through their existing
    bump-and-reprice rules. Grid preference brackets and
    coordinate derivatives do not replace those metrics.
    Propagate product errors through existing Rust, Python and
    WASM pricing routes. The exercise kernel and grid diagnostics
    remain Rust-owned.

11. **Qualify discrete behavior, price and risk independently.**
    Verify exact obstacle satisfaction, ties, multiple regions,
    event adjacency, endpoint and phase rules, and actual-spot
    exercise before quantity scaling. Qualify American time-grid
    error separately from Bermudan fixed exercise-date pricing
    and from the spatial grid, boundary and time-march errors.
    Use independent references with aligned payoffs, clocks and
    conventions, and product-specific price, risk and bracket
    acceptance criteria. Include an American put, a qualified
    non-dividend call with nonnegative rates, and a two-date
    Bermudan put. Compare at common spatial and time grids where
    assessing the effect of opportunity sets. A discrete floor
    and a passing linear-solve residual do not certify continuous
    American convergence or accuracy near a payoff or exercise kink.
    [Exercise and price references](../../../REFERENCES.md#checked-grid-based-pde-exercise)

## Observed choice and algorithm

[exercise.rs](../../../../finstack-quant/models/src/pde/exercise.rs) implements an operator-split obstacle update, not a simultaneous linear complementarity solve. After each selected PDE interval, for any u<g apply u←u/(1+P)+P g/(1+P), repeated `iterations` times. Default P=1e8 and one iteration. The update is independent of dt after validating dt>0; the factor equals λdt in the motivating formulation. It approaches intrinsic from below and may retain a finite shortfall.

Before the update, record every adjacent interior pair whose strict continuation status `(u>g)` changes. The solver returns each transition as (t_to, coordinate of its right-hand node), including multiple regions. This is a grid diagnostic, not a solved continuous free boundary. All-continuation or all-exercise states produce no transition points.

## Selection and domain

[solver.rs](../../../../finstack-quant/models/src/pde/solver.rs) enables American at every post-step time level, or Bermudan only within 1e−10 of supplied times. Bermudan times must be finite, inside [0,T] and already aligned to the uniform grid; the solver rejects off-grid times instead of snapping/refining. Exercise payoff length must equal the number of interior nodes and all payoff/continuation values must be finite. P must be finite positive, with iterations≥1. The public builder exposes exercise payoffs/schedules but not custom penalty tuning; the penalty type's module is private.

## Limitations and alternatives

The maturity condition is supplied by the problem; an exercise time at T does not run a separate projection before the first interval. American exercise is a time-grid approximation, with no automatic refinement or repeated post-exercise damping. A policy iteration, projected SOR or coupled penalty/LCP method would be a different implementation. Do not label the convex pull as a converged global complementarity solution.

## Evidence

- With continuation 0, exercise payoff 1, penalty \(10^8\)
  and one iteration, the observed update gives
  \(10^8/(1+10^8)<1\). The proposed comparison returns 1.
  On a two-interior-node pure diffusion grid with unit widths,
  zero Neumann edges and a unit backward-Euler step, the
  implicit matrix is
  \(B=\begin{pmatrix}2&-1\\-1&2\end{pmatrix}\).
  With RHS \((0,1)\) and exercise payoff \((1,0)\),
  continuation is \((1/3,2/3)\), the comparison gives
  \((1,2/3)\), and the coupled complementarity solution is
  \((1,1)\). The projected result meets its nodewise floor
  while the second coupled residual is \(-2/3\).
- On log knots corresponding to spots 1 and 4, a put struck
  at 5 has nodal exercise values 4 and 1. If both nodal
  values are on the obstacle, interpolation at spot 2
  gives 2.5. Literal exercise pays 3.
- At spatial knots \((-4,-2,-1,0,2,7)\) with
  \(C=(2,0,3,0,1,0)\) and \(G=(1,0,4,0,1,0)\),
  only the node at −1 strictly prefers exercise. The
  brackets are \([-2,-1]\) and \([-1,0]\). A
  Bermudan event at the next greater binary64 value after
  0.5 belongs only to that knot even when the grid contains
  0.5 as well.
- A put with \(S=K=100\), \(r=0.10\), \(q=0\),
  \(\sigma=0.20\), \(T=0.25\), and exercise at 0.125
  and expiry has a two-date Bermudan value of approximately
  2.94974096976 by lognormal quadrature of the mid-date
  maximum of immediate payoff and the remaining European
  value. The European value is approximately 2.82635979627.
  Forsyth and Vetzal report the corresponding American
  put at 3.07008 on their refined coupled finite-volume
  calculation and 3.06999 on their 3,200-step tree.
  [Published benchmark tables](../../../REFERENCES.md#checked-grid-based-pde-exercise)

Inline `american_penalty_enforces_floor`, `exercise_boundaries_cover_calls_and_multiple_regions`, and `exercise_rejects_malformed_inputs_before_mutation` test local behavior. `w08_rannacher_american_put_price_matches_implicit_within_discretisation_error` compares two versions of the same PDE/obstacle kernel with a 0.5% relative tolerance; it is not an independent American reference. [Broadie–Detemple](../../../REFERENCES.md#broadie-detemple-1996) is a potential benchmark-family reference, not evidence approving this projection.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
