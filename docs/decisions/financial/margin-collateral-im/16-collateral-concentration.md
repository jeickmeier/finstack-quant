# MARGIN-COLLATERAL-IM-016 — Collateral concentration reporting

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: concentration checking is a separate reporting helper; it neither changes haircut IM nor enforces collateral allocation. Proposed rationale: report contract-limit breaches separately from monetary valuation, but the intended aggregation level must be explicit.

## Input domain and conventions

Input is a slice of `(CollateralAssetClass,f64 amount)` without currency tags, requiring the caller to normalize units. The API does not validate nonnegative/finite amounts or aggregate repeated classes before computing fractions.

## Ordered algorithm

1. Sum all row amounts with ordinary floating addition. If total≤0, return no breaches.
2. For each original row compute fraction=amount/total.
3. Find only the first schedule entry matching its asset class. If that entry has a concentration_limit and fraction>limit, emit class, fraction, limit and excess=fraction-limit.
4. Equality does not breach. Missing class, missing limit or a later constrained entry produce no breach through this path. Default haircut eligibility does not create a concentration limit.

## Selection, controls and limitations

Limits are decimal fractions. The corporate class convenience inherits30% from embedded class defaults; the default BCBS/treasury schedule entries themselves carry no concentration limits. Output is row-level: two separately supplied20% corporate rows each pass a30% limit even though their combined class amount is40%. This is a direct consequence of the inspected loop, not an executed regression result.

## Alternatives and tradeoffs

Aggregate by class before checking when the legal constraint is class-wide; group by issuer/security when that is the actual constraint. Rejecting repeated keys would instead enforce a preaggregated input contract. Haircut adjustment or forced optimization upon breach are distinct downstream policies.

## Source and validation evidence

Inline tests cover one corporate row at 20% passing30% and one row at 40% producing a10pp breach. Those assertions do not cover repeated classes, invalid values, first-entry ambiguity or automatic enforcement.

Implementation and data: [src/types/collateral.rs](../../../../finstack-quant/margin/src/types/collateral.rs), [data/margin/collateral_schedules.v1.json](../../../../finstack-quant/margin/data/margin/collateral_schedules.v1.json).

Existing evidence: [src/types/collateral.rs](../../../../finstack-quant/margin/src/types/collateral.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
