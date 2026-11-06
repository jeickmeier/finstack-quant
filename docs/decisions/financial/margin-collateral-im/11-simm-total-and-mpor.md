# MARGIN-COLLATERAL-IM-011 — SIMM component assembly, MPOR scaling and diagnostics

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: independently computed delta, vega and curvature charges are added within each risk class, then combined with historical cross-class dependence. Proposed rationale: make component ordering and time scaling explicit and replayable.

## Input domain and conventions

All sensitivity amounts and requested output must be USD; any other output or base-currency mismatch errors. MPOR is a positive number of business days. A caller-provided as-of date labels the result; this stage does not project future exposure or inspect market-data age.

## Ordered algorithm

1. Validate the complete sensitivity set, including otherwise-unused zero entries, then enforce USD normalization.
2. Compute each populated delta/vega class and all curvature classes. Only strictly positive components are included in breakdown/detail and added to the class subtotal.
3. Sort class subtotals and compute `sqrt(max(0,sum_r,s rho_rs*M_r*M_s))`. Missing cross-class lookup uses 1.0, subject to parameter construction checks; the embedded table provides all 15 distinct pairs.
4. Require MPOR>0; multiply total and monetary component breakdown by `sqrt(mpor_days/10)`. No extra horizon evolution, VaR simulation or calibration occurs.
5. Detailed weighted sensitivities, bucket K/S and class/component margins remain at the 10-day basis; detail carries `mpor_scale` separately. Component breakdowns are standalone charges and do not sum to the diversified total.
6. The full result sets `methodology=Simm`, `approximation=true`, ordered string breakdown and optional detailed replay data. The `parts` helper returns only total and breakdown. Trait calculation first calls instrument MTM to select its currency, then requests its sensitivities.

## Selection, controls and limitations

Default MPOR 10 is registry-backed; `with_mpor` mutates it without validation until pricing. Empty valid USD sensitivities produce zero. The ordinary trait path therefore rejects a non-USD MTM even if a supplied sensitivity container is normalized to USD; direct normalized APIs are the supported route for that case. No CSA threshold/MTA/segregation is applied here.

## Alternatives and tradeoffs

A separate product-class aggregation layer and model-specific MPOR recalculation would be broader than square-root scaling. A strict full-model-only API would reject approximations; the current implementation instead exposes the approximate flag. Consumers of the parts helper must carry that provenance themselves.

## Source and validation evidence

Inline formula tests check IR/CQ correlation 0.04 and IR/equity 0.07, horizon scaling and detailed replay behavior. `audit_regressions.rs` checks approximate marking and rejected non-USD output. These are formula/invariant tests, not regulatory approval or a stochastic horizon backtest.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/calculators/traits.rs](../../../../finstack-quant/margin/src/calculators/traits.rs), [src/traits.rs](../../../../finstack-quant/margin/src/traits.rs), [src/constants.rs](../../../../finstack-quant/margin/src/constants.rs).

Existing evidence: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
