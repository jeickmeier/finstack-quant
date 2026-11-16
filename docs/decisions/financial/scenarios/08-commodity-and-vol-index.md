# SCENARIOS-008: Commodity percentage curves and volatility-index point shocks

- Decision status: **Accepted 2026-11-15 — complete commodity relative-percent and volatility-index point shock package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent signed commodity, zero index level and interpolated delivery calculations verified.** Production execution, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for typed commodity and volatility-index scenario shocks

1. **Use dedicated operations and exact source bindings.** Commodity
   shocks use relative percent. Volatility-index shocks use absolute
   index points. Retire commodity `bp` and additive-percent dispatch.
   Bind the exact kind, ID, asset or index series, quotation basis,
   units and delivery convention. Use the same checked Rust operation
   for direct and hierarchy routes. Validate active targets even
   for zero requests.
   [Accepted level sources](../core-market/price-curves.md#accepted-decision-explicit-level-pillars-and-parameter-shocks) ·
   [Accepted scenario hierarchy](03-hierarchy-resolution.md#accepted-decision-for-typed-scenario-hierarchy-expansion-and-overlap).

2. **Scale commodity prices algebraically.** Apply `P'=P(1+p/100)`
   at a selected direct pillar. Admit zero and signed or sign-reversing
   results where the retained interpolation and source domain support
   them. Literal `−100%` produces zero under that same admission.
   Log-linear interpolation requires strictly positive pillars.
   Use checked arithmetic and refuse a required nonzero result lost
   to zero.
   [EIA evidence of negative WTI futures prices](https://www.eia.gov/TodayinEnergy/detail.php?id=43495).

3. **Add volatility-index points and admit eligible zero levels.** Apply
   the signed point increment to the selected level. Reject negative
   and nonfinite levels. A zero level is eligible on a source whose
   interpolation admits it. A log-linear source rejects a zero pillar.
   The futures product retains its positive live-mark requirement
   and exact official settlement authority.
   [Accepted futures marks and settlement](../valuations-equity-and-composition/05-vol-index-and-listed-options.md#accepted-decision-for-checked-volatility-index-futures-and-listed-futures-options) ·
   [Cboe VX specifications](https://www.cboe.com/tradable_products/vix/vix_futures/specifications).

4. **Elect direct parameter changes or evaluated delivery.** Parallel
   acts on all original pillars. Exact-node targets select their
   original pillars. Evaluated delivery uses the requested move of
   the pre-action interpolated level and the selected adjustable
   pillars, local rank and native residual check of record 006.
   Preserve the original grid, clock, interpolation, tails and source
   identity. A supplied time-zero pillar is an ordinary parameter.
   Retire near-zero spot synchronization. Observed spot, fixings and
   settlement retain their respective authorities.
   [Accepted scenario node delivery](06-node-delivery-solve.md#accepted-decision-for-exact-scenario-coordinates-and-checked-node-delivery).

5. **Apply source admission and report actual delivery.** Retire the
   hardcoded commodity warning outside `[−80%,+200%]`. Report requested
   units, exact targets, actual before-and-after levels, valid no-effects
   and applicable inverse residuals. Publish through the checked
   scenario transaction. Retain applicable construction history and
   require fresh qualification before replaying a changed source.
   Record 017 owns the report representation.
   [Accepted scenario execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution) ·
   [Reporting record 017](17-reports-and-replay.md).

## Independent calculation checks

- A commodity level of `−10` under a `+10%` shock becomes `−11`.
  Under `−150%` it becomes `5`. A `−100%` shock on `17`
  produces exact zero where the source admits it.
- Linear volatility-index pillars `15,20` become `0,5` under a
  `−15` point shock. A log-linear rebuild rejects that zero pillar.
- Log-linear volatility-index pillars `1,4` have midpoint `2`.
  A parallel one-point pillar move gives midpoint `sqrt(10)`.
  An evaluated midpoint move of one point gives target `3`; with the
  left pillar fixed, the admitted solution sets the right pillar to `9`.

## Relationship to accepted level-source boundaries

[CORE-MARKET-08](../core-market/price-curves.md#accepted-decision-explicit-level-pillars-and-parameter-shocks)
was accepted on 2026-11-08: no automatic spot anchor, explicit native/relative
source-parameter shocks with original-coordinate weights, scalar domain checks
and frozen dated reuse. This scenario record carries the selected commodity
and volatility-index units through that source boundary. Evaluated node
delivery follows [record 006](06-node-delivery-solve.md#accepted-decision-for-exact-scenario-coordinates-and-checked-node-delivery).

## Observed choice and rationale at the audit revision

Observed choice: commodity stress scales forward prices relatively, while volatility-index stress adds index points. The same price-curve storage supports both, but typed getters and distinct operation variants prevent their units from being conflated.

## Domain and ordered calculation

Commodity `CurveParallelBp` treats its `bp` field as a percentage, so `10` means `P'(t)=1.10*P(t)`, not ten basis points. It verifies a commodity price curve, emits an additive/Percent `BumpSpec`, and warns outside [-80%,+200%]. The range is hardcoded and checks authored percentages alone. Node requests use each pair's numeric value as percent, [resolve and solve](06-node-delivery-solve.md) native prices, multiply selected pillar levels and rebuild with preserved metadata. If the first pillar is within `1e-12` of zero, its new level also becomes spot; otherwise existing spot is retained. The final builder validates the resulting curve. There is no separate commodity percentage floor in scenario validation.

Volatility-index parallel shocks require a `PriceCurveKind::VolIndex` curve and finite points. Preflight the minimum among spot and pillars: a finite minimum plus shock must be strictly positive. Add the same absolute points to every knot and spot, rebuilding under the original ID.

Volatility-index node shocks resolve tenor coordinates, solve native-interpolant level delivery, add solved point changes, and reject each nonpositive resulting level. `rebuild_price_levels` synchronizes spot only when the first knot is effectively zero. Outside the knot range, tenor resolution can warn and target the nearest pillar; native evaluation/extrapolation must still permit the requested target evaluation.

## Failure, alternatives and evidence

No volatility-futures convexity, commodity carry re-estimation, dynamic convenience yield, or stressed storage cost is computed. Rate and commodity shocks therefore need separate specification if a joint macro story requires both. Interpolator reconstruction can move unrequested off-pillar levels; solving requested values alone is not a no-arbitrage proof. Price positivity/negative-price support is inherited from the curve kind's own builder.

Owning source: [adapters/curves.rs](../../../../finstack-quant/scenarios/src/adapters/curves.rs); shared price-curve economics in [price curves](../core-market/price-curves.md). Inspected inline tests assert commodity +10% at all knots and spot, isolated exact-node movement, off-pillar +10% delivery, preserved vol-index metadata and rejection of zero index levels. The joint native-interpolant fixture in [senior regressions](../../../../finstack-quant/scenarios/tests/senior_review_market_regressions.rs) covers linear/log-linear/cubic-Hermite index interpolation. These are formula/contract evidence, not independent market calibration.

Absolute commodity price changes, relative volatility-index changes, or shifts of a calibrated commodity process are alternate shock definitions. No runtime choice changes here.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios).
