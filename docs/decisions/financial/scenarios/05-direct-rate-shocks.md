# SCENARIOS-005: Direct discount, forward and inflation rate shocks

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: rate scenarios operate on model curve quantities. Discount shifts change continuously compounded zero rates; forward shifts change projected rates; inflation shifts change the curve's implied inflation representation. These are distinct from shifting market calibration quotes and re-solving a full rates calibration.

## Ordered calculation and selection

For finite `bp`, `CurveParallelBp` first verifies the requested typed curve exists, then emits `BumpSpec::parallel_bp(bp)` to core. The discount result is `D'(t)=D(t)*exp(-bp*1e-4*t)`, anchored to the curve's current origin through its transform implementation. Forward shifts add `bp*1e-4`. Inflation uses the core inflation bump kernel; its projection rebuild and day-count semantics are recorded in [inflation projection](../core-market/inflation-projection.md). An optional `discount_curve_id` is validated as text by the spec but has no role in these direct rate operations.

For node requests, resolve requested tenors and use [native node delivery](06-node-delivery-solve.md). Discount and inflation produce triangular key-rate effects with left/right neighboring pillars. The first/last triangular buckets extend flat outside their outer side, as defined by core. The discount target quantity is continuous zero rate; the inflation target is `inflation_rate(0,t)`, with an explicit zero return for `t<=0`.

Forward nodes take visible knots and projected rates, add per-knot decimal shifts, then `rebuild_with_knots`. Interpolation style and metadata are retained, but rebuilding replaces the prior source/continuous-adjustment representation with the visible-knot interpolant. Thus a forward-node operation can change off-pillar behavior from previous continuous shocks even when selected pillar values are preserved. This follows the rebuild boundary; the numerical size for compounded real scenarios has not been reproduced here. Discount nodes instead apply composed transforms through `Bumpable`.

## Domain, failure and alternatives

Missing/wrong-kind data errors. Negative rates are not rejected merely for being negative; curve-specific finite/DF/domain validation controls acceptance. Construction constraints after stress can differ from original curve calibration constraints; see [core rate shocks](../core-market/rate-shocks.md). Node conversion uses scenario as-of and curve day count, so a mismatched scenario date and curve base date changes tenor-to-time mapping. No calendar is used for curve node conversion.

Quote bump and full recalibration would measure a different risk. A uniform continuous-transform implementation for forward nodes would preserve prior shock shape; preserving visible rates alone is the observed method. Agree which composition invariants are required and whether mismatch between execution date and curve base must error.

## Evidence

Owning source: [adapters/curves.rs](../../../../finstack-quant/scenarios/src/adapters/curves.rs); shared kernels [rate shocks](../core-market/rate-shocks.md). Inline tests inspect exact/interpolated delivery and preservation of forward tenor/reset-lag/grid/day-count/interpolation/extrapolation/FX-policy metadata. [Curve-variant tests](../../../../finstack-quant/scenarios/tests/shocks/curve_variants_test.rs) locate integration evidence. Selected positive-rate monotonicity fixtures do not prove all stressed DFs decrease or that negative forwards are prohibited.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
