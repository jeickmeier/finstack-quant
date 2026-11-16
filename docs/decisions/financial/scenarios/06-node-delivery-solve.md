# SCENARIOS-006: Tenor matching and simultaneous native-interpolant shock delivery

- Decision status: **Accepted 2026-11-15 — complete exact tenor matching and checked simultaneous node delivery package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent date mapping, exact-width geometry and simultaneous linear and log-linear delivery calculations verified.** Production execution, conditioning and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for exact scenario coordinates and checked node delivery

1. **Resolve exact source coordinates.** Admit an actual date, a positive
   tenor with an explicit origin and roll convention, or a native numeric
   coordinate where supported by the source. Resolve the tenor to its actual
   date and map once on the retained source clock. Use the selected view's
   reference-coordinate difference for rate factors and original source
   time for CPI and level curves. Reject repeated resolved coordinates,
   including collisions of distinct dates. Retire approximate-year
   fallback, epsilon snapping and the `0Y` pseudo-tenor.
   [Accepted tenor arithmetic](../core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions) ·
   [ACT/365F definition](https://strata.opengamma.io/apidocs/com/opengamma/strata/basics/date/DayCounts.html#ACT_365F).

2. **Require an explicit delivery choice.** Exact mode selects one eligible
   original source pillar per request and applies the owner's direct
   parameter shock. Evaluated delivery selects coordinates within the
   original pillar span. Reject endpoint substitution and extrapolated
   delivery. An actual origin pillar may be eligible for a native price or
   volatility-index level. CPI's pinned origin and the evaluated zero rate
   at the rate-factor conditioning reference are ineligible targets.
   [Accepted direct rate and CPI parameters](05-direct-rate-shocks.md#accepted-decision-for-typed-direct-rate-and-cpi-parameter-shocks) ·
   [Accepted level pillars](../core-market/price-curves.md#accepted-decision-explicit-level-pillars-and-parameter-shocks).

3. **Deliver evaluated rate-factor zero moves directly.** For discount
   and projection factors, center an exact triangular adjustment on each
   resolved target. Select its neighbors from the ordered union of native
   source knots and target coordinates. Each shape has unit weight at its
   center and zero weight at all other requested centers. Apply the moves
   together through the complete factor evaluator, preserving its prior
   shapes and original source grid. No inverse solve is needed.
   [Accepted exact rate geometry](../core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning).

4. **Elect adjustable pillars explicitly for the inverse.** For evaluated
   CPI use original-anchor annual growth plus the requested rate change.
   For commodity use the pre-action level scaled by the requested relative
   change. For the volatility index use the pre-action level plus absolute
   index points. Supply an ordered set of distinct original pillars with
   the same count as target rows. Hold other pillars fixed. Adjust CPI
   annual-growth parameters or native commodity and volatility-index
   levels. Preserve the source's anchor where applicable, grid,
   interpolation, tails and valid domain.
   [Accepted CPI parameter shocks](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks) ·
   [Hazard and CDS record 007](07-hazard-quote-shocks.md).

5. **Use a bounded joint solve with a required local identification gate.**
   Start at zero pillar changes. Use the accepted fallible
   Levenberg–Marquardt solver with explicit parameter scales, residual
   tolerance, step tolerance and work limit. Divide each signed residual
   by its own finite positive tolerance in native target units. Build one
   checked candidate per trial and evaluate all rows on it. At the
   returned candidate require resolved full local rank of the undamped
   scaled Jacobian under an explicit relative cutoff. A validated
   all-zero request may return the unchanged evaluator directly.
   [Accepted solver](../core-numerics/levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes) ·
   [Accepted conditioning arithmetic](../calibration-framework/15-sensitivity-diagnostics.md#accepted-decision-for-calibration-sensitivities-and-conditioning).

6. **Re-evaluate every row on the exact post-action evaluator.** Include
   exact-pillar and zero-move rows in a mixed request. Require each signed
   native residual to pass its own tolerance. Preserve requested and
   realized moves, selected pillars, controls and actual work. Later
   authored actions may move the same levels again. Publish only through
   the accepted scenario transaction and retain the per-action evidence
   under the reporting owner.
   [Accepted scenario execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution) ·
   [Reporting record 017](17-reports-and-replay.md).

## Independent calculation checks

- A one-year tenor from 2024-01-01 maps to `366/365` under ACT/365F.
  A represented point one ULP from the left of a three-ULP interval
  has exact linear weights `2/3` and `1/3`.
- A linear midpoint increment of `1` admits endpoint moves `(2,0)`,
  `(1,1)` and `(0,2)`. Simultaneous increments `1` at the quarter
  point and `2` at the three-quarter point give `(.5,2.5)`.
- With log-linear pillars `1` and `4`, the midpoint is `2`.
  Fixing the right pillar and setting the left to `2.25` gives midpoint
  `3`. Fixing the left and setting the right to `9` also gives `3`.

## Observed choice and rationale at the audit revision

Observed choice: a request between native pillars specifies the desired move of the curve's evaluated economic quantity at that requested time. The adapter starts from a linear split, then solves against the actual interpolator for direct curves. This differs from blindly assigning the same fraction of the requested shift to surrounding knots.

## Ordered algorithm and equations

1. Convert tenor to contextual years using scenario `as_of`, no calendar, `Unadjusted`, and the curve's day count. Also compute `Tenor::to_years()` as a simple approximation. Literal `0Y` is treated as time zero.
2. `Exact` selects the first pillar within absolute `1e-6` of the contextual time, otherwise of simple years, otherwise errors. `Interpolate` uses contextual time unless only the simple time exactly matches a pillar.
3. The interpolation helper uses two bracketing linear weights `w0,w1`; outside the range it uses the nearest endpoint with weight one and emits `TenorExtrapolated` when outside by more than `1e-10`. Near-duplicate bracket widths below `1e-12` split evenly. Empty knots yield no weights.
4. Seed each pillar delta with `requested*w_i/sum_j(w_j^2)` (normalization only when the denominator exceeds `1e-12`). Add contributions from overlapping requests. This normalization gives the requested linear-interpolation move for an isolated target; it is not a global simultaneous solution.
5. For direct discount, forward, inflation, commodity and volatility-index curves, aggregate repeated requested times within `1e-12`, select the union of touched pillars, and solve all requested-time equations together. Other pillars remain zero. `LevenbergMarquardtSolver::new()` uses tolerance `1e-13`; residuals are rebuilt/evaluated curve quantity minus target. Preview evaluation errors become NaN residuals and ultimately solver failures.
6. Re-evaluate every solution target and require finite values and absolute residual at most `1e-12`; otherwise error. Emit only nonzero solved pillar deltas. Discount/inflation preview applies ordered triangular bumps, forward preview adds decimal rates to visible knots, commodity preview multiplies visible levels, and vol-index preview adds absolute levels.

Targets are `z+bp/10,000`, `f+bp/10,000`, implied inflation plus `bp/10,000`, commodity price times `1+pct/100`, or volatility-index level plus points. These residual quantities have different units despite sharing one absolute tolerance. The selected active pillar set and LM initialization resolve otherwise nonunique shape choices; there is no explicit smoothness/minimum-norm financial objective.

The live LM callback is fallible. The live preview still skips tiny rate
adjustments by epsilon, while the emitted list skips only literal zeros.

## Approximation boundaries, failure and alternatives

ParCDS operations do **not** run this native-interpolant solve. Solve-to-par passes normalized split targets to calibration quote replay and warns when the split spans multiple entries because quote snapping/rebootstrap need not deliver the requested off-pillar spread move. First-order hazard mode also uses the split without recalibrating native off-pillar intensity; piecewise hazard evaluation is not linear zero-rate interpolation.

Duplicate targets, near-coincident times, zero-time implied inflation, large percentage shocks, invalid preview domains, sparse pillars and out-of-range interpolation/extrapolation can make the system ill-conditioned or unsatisfiable. The solver controls are not exposed by the scenario specification. Existing transformed forward curves are rebuilt from visible knots, as documented in [direct rates](05-direct-rate-shocks.md).

## Evidence

Owning source: [adapters/curves.rs](../../../../finstack-quant/scenarios/src/adapters/curves.rs), [utils.rs](../../../../finstack-quant/scenarios/src/utils.rs). Shared optimizer methodology is [Levenberg–Marquardt](../core-numerics/levenberg-marquardt.md). Inspected inline tests assert +25bp at an off-pillar discount tenor, +10% commodity delivery, and arithmetic interpolation weights. Inspected [senior regression](../../../../finstack-quant/scenarios/tests/senior_review_market_regressions.rs) simultaneously requests +1 at 2Y and +2 at 4Y and checks native linear, log-linear and cubic-Hermite vol-index curves at `1e-8`. These are deterministic delivery checks, not global shape or economic-risk validation.

A pure triangular shock at the requested time or calibrated-quote bucket risk are alternatives with different economics.

## Relationship to the accepted inflation parameter boundary

[CORE-MARKET-10](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks)
owns direct annual-growth parameter changes on original positive CPI pillars.
This record owns the separately elected evaluated delivery and its final
residual gates.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios).
