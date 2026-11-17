# PORTFOLIO-ATTRIBUTION-010: Optimization weights, valuation features and trade universe

- ID: PORTFOLIO-ATTRIBUTION-010
- Decision status: Accepted 2026-11-16 for checked optimization decision-space construction.
- Implementation status: Observed for the stated owning source scope. Runtime, solver-caller and host reconciliation with the accepted contract are pending.
- Validation status: Independent signed-PV, held-bound, gross/net, metric scaling, zero-PV and percentage-sizing examples were checked. Production and host qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked optimization decision space

1. Freeze one qualified dated market state, effective model settings and reporting FX, and use the same canonical valuation executor for holdings and candidates. Preserve existing-position order followed by declared candidate order. Validate IDs, ownership, instrument definitions, units, filters and bounds before pricing. Candidates start at zero held size with their declared entities and attributes. [Accepted portfolio execution](../portfolio-execution/04-execution-order-and-reduction.md).
2. Require an explicit decision coordinate. For signed target holding scale `s_i`, signed base-currency PV per scale `p_i` and positive qualified deal notional per scale `n_i`, use `s_i*p_i/G` for ValueWeight and `s_i*n_i/G` for NotionalWeight. Explicitly select `G` from the applicable positive opening gross amount or positive caller-supplied base-currency Money. Candidate rows do not enter opening gross. Remove the near-zero denominator fallback. Replace mixed UnitScaling with TargetScale, where the decision is `s_i` for existing holdings and candidates. Net budget, gross exposure and turnover are separate controls under [record 011](11-linear-program-formulation.md). [Accepted holding sizing](../portfolio-execution/01-position-sizing-and-validation.md).
3. Require an explicit trade universe with filters on position IDs, entities and position attributes. Intersect applicable hard bounds with the permitted signed size under the holding unit and the caller's long or short authorization. A hold fixes exact current quantity and remains subject to hard bounds. Remove implicit per-position share caps and automatic candidate short-bound expansion. A positive candidate minimum is unconditional in this continuous model. Zero-or-minimum size requires discrete selection. [Portfolio optimization buy-in thresholds](https://docs.mosek.com/portfolio-cookbook/transaction.html#buy-in-threshold).
4. Obtain the qualified unscaled mark directly, including for zero-opening holdings. Reject a zero-PV position in ValueWeight when its effective bounds permit a size change, including a negative-only candidate. An existing position kept at exact quantity can retain zero value weight and contribute its known additive risk as a fixed term in applicable constraints. Apply the accepted percentage bounds before solving. [Record 016](16-optimization-results-and-rebalance.md#accepted-decision-for-checked-optimization-results-and-rebalance) owns checked reconstruction and application.
5. Build an exact metric menu for each row from the objective and matching constraints. Resolve standard, custom and bucketed keys to qualified producers and risk bases. Remove implicit Standard metrics and automatic Zero or Exclude handling. Missing or failed required features fail the request. Qualified zeros are admitted. Treat structural inapplicability as zero only when the measure contract allows it. Callers can explicitly filter narrower expressions.
6. Derive additive coefficients from unscaled amounts using the selected decision-to-quantity conversion exactly once. Use target PV as the value expression and retain opening held PV as baseline evidence. Require compatible units and a declared weighting basis for intensive features such as duration and yield. Preserve ordered inputs, effective settings, coefficient units and provenance through one checked Rust contract across hosts. [Accepted metric aggregation](../portfolio-execution/10-metric-aggregation-and-reporting.md).

With base reference `100` and per-scale PV `-20`, value weight `+0.10` implies signed scale `-0.5`. A position held at `0.60` subject to cap `0.50` is infeasible. The observed assembly can overwrite that cap with the held equality.

## Observed choice and domain at the audit revision

Optimization is a continuous linear program over caller-defined weights, with current valuations and metrics frozen while solving. Default spec/problem settings are ValueWeight, missing-metric policy Zero, all existing positions tradeable, no candidates and no explicit constraints. There is no covariance, transaction-cost or nonlinear repricing objective implicit in this interface.

## Decision construction

1. Validate the portfolio and declared constraints; collect explicit standard MetricIds requested by objective/bounds, then value existing positions and candidates with Standard or StandardPlus metrics. Strict missing policy sets strict risk collection. Custom-key inputs do not automatically request corresponding standard calculations.
2. Construct decision variables in existing-position then candidate order. ValueWeight's current weight is signed held base-currency PV divided by existing gross absolute PV. NotionalWeight uses signed scale times absolute deal notional converted to base currency, divided by existing gross absolute notional. Missing instrument notional errors rather than falling back to quantity. If the gross denominator is at most `1e-6`, current share weights are zero.
3. UnitScaling uses current weight one for every existing position and zero for candidates. Existing variables multiply current raw quantity; candidate variables are target raw quantity. They are not comparable dollar weights. Share reconstruction later uses `PositionUnit` scale/quantity conversion.
4. Explicitly held positions, nontradeable positions and positions excluded because a required metric is missing are fixed at current weight. Filters select by entity, position ID, attributes or Boolean combinations; empty And matches all and empty Or none. Optimization filters use position attributes, unlike factor matching's instrument attributes.
5. Existing default bounds are `[0,1]` for positive ValueWeight/NotionalWeight, `[-1,0]` for negative PV/notional, and `[0,+infinity)` for UnitScaling. Candidate constructor bounds are `[0,1]`; an allow-short flag changes a near-zero minimum to negative maximum. Explicit filtered bounds intersect these ranges. Held variable equality takes precedence at assembly.
6. Candidates are valued with raw quantity one in a temporary entity, correcting per-scale PV for percentage units; their decision entity/attributes remain caller supplied. Candidate IDs cannot collide with current positions. Gross denominators use existing holdings only, so an empty/zero-gross book does not acquire capital merely by listing candidates.
7. Metric coefficients can be standard/custom metrics, numeric attributes, indicators, constants or held `PvBase`. Candidate `PvBase` in an applicable expression errors because candidates have no held PV. Zero policy inserts zero; Exclude freezes the position and drops its missing coefficient (and its average-bound denominator membership); Strict errors.

## Failure boundaries and alternatives

Tradeable existing zero-PV holdings and zero-PV candidates are rejected under ValueWeight in the coded gates. The candidate gate checks `abs(max_weight)>1e-12`, so a negative-only candidate with maximum zero can evade that check. Reconstructed zero-price quantity then falls back to zero. Candidate bounds and metric constants also lack a general finite-value check before the solver; numerical effects need reproduction. Long/short gross weights do not generally sum to one, although the default LP budget requires net sum one.

Full nonlinear optimization, dollar-position variables and normalized factor exposure objectives avoid some frozen-feature assumptions but are different formulations. Here metric units are inherited unchanged; callers must distinguish per-unit versus amount-like measures to avoid multiplying an already scaled amount by another weight unintentionally.

## Evidence

[Decision construction](../../../../finstack-quant/portfolio/src/optimization/decision.rs), [metric and weighting contracts](../../../../finstack-quant/portfolio/src/optimization/types.rs), [universe](../../../../finstack-quant/portfolio/src/optimization/universe.rs), [problem defaults](../../../../finstack-quant/portfolio/src/optimization/problem.rs) and [spec defaults](../../../../finstack-quant/portfolio/src/optimization/spec.rs) own these choices. [Integration cases](../../../../finstack-quant/portfolio/tests/test_optimization_fixes.rs) and [basic solver examples](../../../../finstack-quant/portfolio/tests/optimization_basic.rs) are regression evidence, not executed qualification.

## Follow-up tracking

Implementation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution).
