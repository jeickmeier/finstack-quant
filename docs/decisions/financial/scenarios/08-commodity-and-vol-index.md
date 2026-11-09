# SCENARIOS-008: Commodity percentage curves and volatility-index point shocks

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Relationship to accepted level-source boundaries

[CORE-MARKET-08](../core-market/price-curves.md#accepted-decision-explicit-level-pillars-and-parameter-shocks)
was accepted on 2026-11-08: no automatic spot anchor, explicit native/relative
source-parameter shocks with original-coordinate weights, scalar domain checks
and frozen dated reuse. Reconcile current bp-as-percent, additive-percent,
zero-level and near-origin synchronization paths to that contract. Separately
selected off-pillar native-level solving remains this scenario owner's
operation; this dependency does not adopt its remaining delivery/diagnostic choices.

## Choice and rationale

Observed choice: commodity stress scales forward prices relatively, while volatility-index stress adds index points. The same price-curve storage supports both, but typed getters and distinct operation variants prevent their units from being conflated.

## Domain and ordered calculation

Commodity `CurveParallelBp` treats its `bp` field as a percentage, so `10` means `P'(t)=1.10*P(t)`, not ten basis points. It verifies a commodity price curve, emits an additive/Percent `BumpSpec`, and warns outside [-80%,+200%]. The range is a configured diagnostic threshold, not a fitted stress distribution or validated market bound. Node requests use each pair's numeric value as percent, [resolve and solve](06-node-delivery-solve.md) native prices, multiply selected pillar levels and rebuild with preserved metadata. If the first pillar is within `1e-12` of zero, its new level also becomes spot; otherwise existing spot is retained. The final builder validates the resulting curve. There is no separate commodity percentage floor in scenario validation.

Volatility-index parallel shocks require a `PriceCurveKind::VolIndex` curve and finite points. Preflight the minimum among spot and pillars: a finite minimum plus shock must be strictly positive. Add the same absolute points to every knot and spot, rebuilding under the original ID.

Volatility-index node shocks resolve tenor coordinates, solve native-interpolant level delivery, add solved point changes, and reject each nonpositive resulting level. `rebuild_price_levels` synchronizes spot only when the first knot is effectively zero. Outside the knot range, tenor resolution can warn and target the nearest pillar; native evaluation/extrapolation must still permit the requested target evaluation.

## Failure, alternatives and evidence

No volatility-futures convexity, commodity carry re-estimation, dynamic convenience yield, or stressed storage cost is computed. Rate and commodity shocks therefore need separate specification if a joint macro story requires both. Interpolator reconstruction can move unrequested off-pillar levels; solving requested values alone is not a no-arbitrage proof. Price positivity/negative-price support is inherited from the curve kind's own builder.

Owning source: [adapters/curves.rs](../../../../finstack-quant/scenarios/src/adapters/curves.rs); shared price-curve economics in [price curves](../core-market/price-curves.md). Inspected inline tests assert commodity +10% at all knots and spot, isolated exact-node movement, off-pillar +10% delivery, preserved vol-index metadata and rejection of zero index levels. The joint native-interpolant fixture in [senior regressions](../../../../finstack-quant/scenarios/tests/senior_review_market_regressions.rs) covers linear/log-linear/cubic-Hermite index interpolation. These are formula/contract evidence, not independent market calibration.

Absolute commodity price changes, relative volatility-index changes, or shifts of a calibrated commodity process are alternate shock definitions. No runtime choice changes here.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
