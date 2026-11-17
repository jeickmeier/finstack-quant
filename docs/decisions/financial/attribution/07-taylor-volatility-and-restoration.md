# ATTRIBUTION-007: Taylor volatility, restored families and cash theta

- Decision status: Accepted 2026-11-16 for checked Taylor volatility, restored factors and cash theta.
- Implementation status: Observed for the source scope listed below. Runtime, pricing, cash lineage and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked directional volatility curvature, disjoint restoration, cash realization and mixed-anchor residual. Production attribution and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked Taylor volatility, restored factors and cash theta

1. Keep Taylor as an explicit hybrid. Use qualified opening-state sensitivities for the curve experiments in [record 006](06-taylor-curve-risk.md#accepted-decision-for-checked-taylor-curve-sensitivities-and-directional-gamma) and elected ordinary volatility grids. Use closing-state restoration for the remaining supported effects. Resolve actual source use at both endpoints and in each counterfactual. Assign each changed active source once and retain its economic role. Rate fixings and independent credit-index inputs beyond the curve sensitivity require a qualified restore under their own factor or a typed failure. Derived state rebuilt by replay belongs to that experiment. [Accepted restoration contract](02-market-factor-restoration.md#accepted-decision-for-checked-economic-factor-families-and-market-restoration).
2. Measure the complete ordinary-grid move in native annual sigma. Pair each opening node with a qualified closing observation of the same economic quote. Bind the underlying, expiry, strike or reference tenor, quote type, units, displacement and source clocks. Require a product-owned mapping where clocks or grid coordinates differ. Keep source aging on its qualified carry basis. Retire the generic Act/365 reference point and surface-average fallback. Black and normal volatility retain their own native units. [Accepted volatility grid and clock contract](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks).
3. Reprice the whole volatility move direction. For nonzero vector `d` require an explicit positive finite calculation step `h` in the source's native units and set `a = min(1, h / max |d_i|)`. Verify both `+a·d` and `−a·d` from one qualified opening state. Use checked holder-currency marks and verified common draws where required. Compute first order as `(V+ − V−) / (2a)`. When curvature is elected add `[(V+ − V0) + (V− − V0)] / (2a²)`. Retire fixed bump ranges and defaults, flooring and automatic step shrinking. A verified zero on the mapped grid requires zero shock reprices. The stencil captures within-grid cross-node curvature and establishes no accuracy bound. [Accepted option sensitivity controls](../valuations-framework/option-sensitivities.md#accepted-decision-for-checked-option-and-fx-sensitivities).
4. Restore remaining volatility sources together. Elect the route up front. Restore active scalar vols, SABR cubes or slices, FX-delta surfaces, volatility-index curves and any ordinary grid explicitly elected for restoration. Keep Taylor-expanded grids at their closing values in that restore. Require an owner-qualified comparison for any source-kind or precedence switch and a valid dated view for aged volatility sources. Failed elected expansion is a typed failure.
5. Use one qualified closing anchor for restored effects. Restore each exact scope atomically and calculate `V1 − V_restored` in checked native Money, including that scope's own nonlinear response. Apply this to remaining volatility, native pricing FX, inflation and releases, correlations, market scalars, model parameters and other qualified inputs under their owning factors. FX uses frozen eligible quote and route authority. Reporting FX follows [record 013](13-target-currency-and-scaling.md). Restore qualified opening pricing parameters while preserving contract terms, entitlement and lifecycle facts.
6. Apply the accepted carry and cash rules. Publish price theta `V_H − V0`, held cash `C_held` and gross carry `V_H − V0 + C_held`. Use the independently qualified closing ledger for headline P&L `V1 − V0 + C_close`. Sensitivity and restore amounts are price-only. Reconcile changes by cash event and allocate each once to a qualified effect, including offsetting changes that net to zero. Unsupported allocation fails complete attribution. Apply the elected carry partition and funding treatment under [record 003](03-total-return-carry.md#accepted-decision-for-checked-period-cash-gross-carry-and-funding).
7. Use checked results and disclose the residual. Required FX, cash, restore, pricing and arithmetic failures fail complete attribution. Distinguish evaluated zero, proven inapplicability and elected advisory unavailability. Retire the implicit Taylor thresholds of 10 currency units and 5%. Use qualified shared tolerances and leave their broader semantics with record 013. Mixed-anchor interactions, cross-factor effects and approximation error can remain in the price residual. Publish source and cut identities, elected routes, delivered shocks, marks, cash, actual pricing attempts and reuse through checked Rust results that Python and WASM project. [Accepted execution contract](01-execution-and-endpoints.md#accepted-decision-for-checked-instrument-attribution-execution-and-endpoints).

Exact examples verified that a two-node `(+2, −2)` vol-point twist produces first-order P&L of `+4` and curvature of `−8` even though its average move is zero. An independent cash example reconciled `+4` headline P&L to `+1` gross carry, `+2` market price effect and `+1` of changed cash.

## Observed choice and algorithm

The Taylor route is a hybrid: rates/credit/ordinary vol surfaces use sensitivity expansions at opening state; scalar volatility, FX, inflation, correlation, ordinary market scalars and model parameters use closing-state restore-and-reprice differences. Therefore it is not one uniform multivariate Taylor series.

For each declared volatility dependency that resolves to an ordinary surface in either snapshot, bump all surface volatility absolutely by ±h, default h=0.01 (one vol point), permitted finite range `[0.0001,0.20]`. Vega per vol point is `[Vup-Vdown]/(2*h*100)`. Observed move is measured at opening-date Act/365 expiry and dependency reference strike when both exist; otherwise use the surface-average helper and emit a note. Add vega × move and, when gamma enabled, `0.5*[Vup-2V0+Vdown]/(100h)² * move_points²`.

Scalar-vol quotes are separately restored to opening values in an otherwise closing volatility-family snapshot. This keeps closing surfaces/cubes in that restore, and measures scalar-vol pricing effect exactly. Ordinary surface expansion does not cover non-scalar SABR cubes, FX-delta surfaces or volatility-index curves merely because the restoration layer recognizes them; there is no general restored-volatility remainder here.

FX restores the opening matrix whenever either snapshot has a matrix, including a closing-only introduction (with a note). Inflation, correlations and market scalars restore full opening families whenever either snapshot contains family data. Each contribution is closing PV minus closing-date PV with that family restored. Model parameters similarly restore the opening snapshot, when a non-None snapshot exists. These full-family effects already include own-family nonlinearities; no additional gamma is applied.

Theta uses opening parameters/market at closing date and adds shared period economic cash. It is then routed through total-return carry with income cash but no accrued, flat-yield or funding detail; the remainder is roll-down. Carry collection failure sets invalid, leaves the successful theta absent, and notes that total excludes period cash receipts.

## Controls, limitations and failure

Most factor errors set invalid and add notes; the explicit FX catch adds a note but does not set invalid, a real method-specific asymmetry. Nonfinite aggregate factor amounts become zero sentinels with invalid status; sensitivity working can be omitted when component values are nonfinite. Default residual thresholds are 10 currency units and 5 percent. No mixed-factor cross terms are calculated. Repeated dependencies can generate repeated surface factors; dependency uniqueness is not enforced in this loop.

## Rationale and alternatives

The hybrid supplies full-family attribution where a scalar sensitivity contract is absent, but observation point and interaction conventions differ across factors. Parallel/waterfall are supported alternatives. Located tests cover reference-point/average selection, all surface dependencies, failed-factor flags and restored FX/scalar/inflation/correlation/model factors. They are implementation and synthetic response checks, not a general accuracy certification.

## Source and evidence

- [taylor.rs](../../../../finstack-quant/attribution/src/taylor.rs) — owning implementation.

## Relationship to the accepted context lifecycle boundary

[CORE-MARKET-20's accepted package](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle)
requires each factor restore to declare its typed namespace scope and build one
validated final context before publication. Preserve unselected data and canonical
composite dependencies; propagate construction failure instead of warning away
invalidated credit indices. Reconcile actual restore_market callers, guarded
scratch and required FX quote/query coverage rather than treating clone/JSON as
complete immutable replay. Accepted record 20 does not adopt this
record's Taylor/hybrid allocation, factor-error reporting, residual or theta model.

## Follow-up tracking

Runtime reconciliation and validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). Production and host
qualification remain pending.
