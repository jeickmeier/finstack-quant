# MARGIN-COLLATERAL-IM-007 — SIMM residual equity delta and vega

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: each equity name receives the residual weight and names combine with zero cross-correlation. Proposed rationale: preserve unrelated names instead of cancelling their signs in a scalar, while making absent bucket classification explicit.

## Input domain and conventions

Signed USD P&L per 1% relative equity-price move for delta; compatible volatility-weighted vega per name for vega. Underlier IDs must be nonempty. Exact names combine before pricing.

## Ordered algorithm

1. Delta uses `CR=max(1,sqrt(abs(delta)/370000))`, RW=50 percentage points and WS=delta×50×CR.
2. Vega first forms `raw=vega*0.60` (HVR), then concentration at CT=39m, then RW=0.45.
3. Sort names by absolute WS with name as tie breaker; aggregate `sqrt(sum WS_name²)`. Opposite signs on different underliers do not hedge each other; opposite contributions to the same key net before this step.
4. Keep per-name K=abs(WS) and signed WS in detailed diagnostics. Delta/vega margins add with residual equity curvature, then enter cross-risk-class aggregation.

## Selection, controls and limitations

No equity bucket, country, market cap, repo delta or explicit cross-name covariance input exists in this SIMM container. HVR is applied before vega concentration. All final SIMM results remain marked approximate.

The calculator has no source-volatility metadata to distinguish quoted vega, percent vega or sigma-weighted vega at runtime.

## Alternatives and tradeoffs

Full sector/country/size buckets would represent more prescribed dependence; a bespoke estimated covariance model would be a different methodology. Scalar netting is smaller but would incorrectly equate unrelated underliers within this implementation's intended identity model.

## Source and validation evidence

`audit_regressions.rs::simm_preserves_ir_direction_and_does_not_erase_unrelated_equities` checks a positive charge for opposite AAPL/MSFT amounts and the approximation flag. Parameter tests pin RW 50/VRW 0.45/HVR 0.60 and thresholds; these are selected literal and invariant checks.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs), [tests/simm_schedule_parity.rs](../../../../finstack-quant/margin/tests/simm_schedule_parity.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
