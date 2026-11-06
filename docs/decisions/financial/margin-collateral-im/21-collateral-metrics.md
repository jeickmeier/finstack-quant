# MARGIN-COLLATERAL-IM-021 — Collateral adequacy, funding spread and haircut sensitivity

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: these are deterministic reporting metrics, not pricing engines or dynamic XVA. Proposed rationale: expose transparent monetary/ratio diagnostics with explicit currency handling.

## Input domain and conventions

Money inputs carry currency. MarginUtilization requires same-currency finite nonnegative posted/required amounts. ExcessCollateral checks currency but does not impose nonnegativity. Funding/collateral rates are annual decimals; haircut is a decimal metadata value and period length a year fraction.

## Ordered algorithm

1. Utilization=posted/required for required>0; posted>0, required 0 gives+infinity; posted=required=0 gives1. Adequacy iff ratio≥1; shortfall=required-posted iff ratio<1, otherwise 0.
2. Utilization JSON serializes only posted/required and recomputes ratio on deserialization so infinity need not appear on wire. Fields are public, so a manually inconsistent struct is not validated by every accessor.
3. ExcessCollateral=collateral-required, positive/negative tests report excess/shortfall. Percentage=excess/required when required>0, else0, even if a positive collateral balance exists.
4. Funding spread=funding_rate-collateral_rate; annual cost=posted×spread; period cost=annual_cost×year_fraction. No discounting, changing exposure, accrual calendar, threshold or segregated reuse rule is applied. Negative spread gives a benefit.
5. Haircut01's pv_change=collateral_value×0.0001, independent of current haircut; current_haircut only reports haircut_bp=haircut×10000. This is a positive additional-haircut amount convention, not a full collateral portfolio PV reprice.

## Selection, controls and limitations

Money operations provide currency/finite arithmetic failure behavior, but only utilization enforces a nonnegative input domain at construction. The derived fields of ExcessCollateral, MarginFundingCost and Haircut01 serialize directly and can be inconsistent if constructed/deserialized independently of helpers.

## Alternatives and tradeoffs

Optional ratios would avoid infinity and 0/0 conventions but change reporting. Curve-discounted funding over dated margin profiles belongs to XVA/MVA; numerical haircut bumps would include nonlinear eligibility/price effects omitted from this linear diagnostic.

## Source and validation evidence

Inspected metric tests check12m/10m=1.2,8m/10m shortfall2m,105m-100m=5m,50m×20bp=100k, and 100m×1bp=10k. A zero-requirement JSON roundtrip is an invariant check; none establishes dynamic funding cost accuracy.

Implementation and data: [src/metrics/mod.rs](../../../../finstack-quant/margin/src/metrics/mod.rs), [src/schema.rs](../../../../finstack-quant/margin/src/schema.rs).

Existing evidence: [src/metrics/mod.rs](../../../../finstack-quant/margin/src/metrics/mod.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
