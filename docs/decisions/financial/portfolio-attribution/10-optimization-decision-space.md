# Optimization weights, valuation features and trade universe

- ID: PORTFOLIO-ATTRIBUTION-010
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

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

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
