# SCENARIOS-006: Tenor matching and simultaneous native-interpolant shock delivery

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: a request between native pillars specifies the desired move of the curve's evaluated economic quantity at that requested time. The adapter starts from a linear split, then solves against the actual interpolator for direct curves. This differs from blindly assigning the same fraction of the requested shift to surrounding knots.

## Ordered algorithm and equations

1. Convert tenor to contextual years using scenario `as_of`, no calendar, `Unadjusted`, and the curve's day count. Also compute `Tenor::to_years()` as a simple approximation. Literal `0Y` is treated as time zero.
2. `Exact` selects the first pillar within absolute `1e-6` of the contextual time, otherwise of simple years, otherwise errors. `Interpolate` uses contextual time unless only the simple time exactly matches a pillar.
3. The interpolation helper uses two bracketing linear weights `w0,w1`; outside the range it uses the nearest endpoint with weight one and emits `TenorExtrapolated` when outside by more than `1e-10`. Near-duplicate bracket widths below `1e-12` split evenly. Empty knots yield no weights.
4. Seed each pillar delta with `requested*w_i/sum_j(w_j^2)` (normalization only when the denominator exceeds `1e-12`). Add contributions from overlapping requests. This normalization gives the requested linear-interpolation move for an isolated target; it is not a global simultaneous solution.
5. For direct discount, forward, inflation, commodity and volatility-index curves, aggregate repeated requested times within `1e-12`, select the union of touched pillars, and solve all requested-time equations together. Other pillars remain zero. `LevenbergMarquardtSolver::new()` uses tolerance `1e-13`; residuals are rebuilt/evaluated curve quantity minus target. Preview evaluation errors become NaN residuals and ultimately solver failures.
6. Re-evaluate every solution target and require finite values and absolute residual at most `1e-12`; otherwise error. Emit only nonzero solved pillar deltas. Discount/inflation preview applies ordered triangular bumps, forward preview adds decimal rates to visible knots, commodity preview multiplies visible levels, and vol-index preview adds absolute levels.

Targets are `z+bp/10,000`, `f+bp/10,000`, implied inflation plus `bp/10,000`, commodity price times `1+pct/100`, or volatility-index level plus points. These residual quantities have different units despite sharing one absolute tolerance. The selected active pillar set and LM initialization resolve otherwise nonunique shape choices; there is no explicit smoothness/minimum-norm financial objective.

## Approximation boundaries, failure and alternatives

ParCDS operations do **not** run this native-interpolant solve. Solve-to-par passes normalized split targets to calibration quote replay and warns when the split spans multiple entries because quote snapping/rebootstrap need not deliver the requested off-pillar spread move. First-order hazard mode also uses the split without recalibrating native off-pillar intensity; piecewise hazard evaluation is not linear zero-rate interpolation.

Duplicate targets, near-coincident times, zero-time implied inflation, large percentage shocks, invalid preview domains, sparse pillars and out-of-range interpolation/extrapolation can make the system ill-conditioned or unsatisfiable. The solver controls are not exposed by the scenario specification. Existing transformed forward curves are rebuilt from visible knots, as documented in [direct rates](05-direct-rate-shocks.md).

## Evidence

Owning source: [adapters/curves.rs](../../../../finstack-quant/scenarios/src/adapters/curves.rs), [utils.rs](../../../../finstack-quant/scenarios/src/utils.rs). Shared optimizer methodology is [Levenberg–Marquardt](../core-numerics/levenberg-marquardt.md). Inspected inline tests assert +25bp at an off-pillar discount tenor, +10% commodity delivery, and arithmetic interpolation weights. Inspected [senior regression](../../../../finstack-quant/scenarios/tests/senior_review_market_regressions.rs) simultaneously requests +1 at 2Y and +2 at 4Y and checks native linear, log-linear and cubic-Hermite vol-index curves at `1e-8`. These are deterministic delivery checks, not global shape or economic-risk validation.

A pure triangular shock at the requested time or calibrated-quote bucket risk are alternatives with different economics.

## Relationship to the accepted inflation parameter boundary

[CORE-MARKET-10's complete accepted package](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks) selects exact original
positive source-pillar annual-growth parameters. Its direct shock is distinct
from this record's inverse solve to force an evaluated off-pillar quantity.
Reconcile the native inflation adapter and remove zero-time growth sentinels
without treating inverse delivery or tolerance matching as an implicit core
parameter operation. Record10 does not adopt this Draft solve or its objective/
acceptance rules. Record10 was accepted on **2026-11-08**; its runtime
reconciliation and independent qualification remain pending.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
