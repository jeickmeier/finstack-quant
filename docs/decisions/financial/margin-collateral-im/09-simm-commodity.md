# MARGIN-COLLATERAL-IM-009 — SIMM commodity bucket netting and dependence

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: delta/vega amounts are reduced to one signed total per commodity bucket before weighting. Proposed rationale: the supplied input shape supports bucket-level exposure, while named commodity detail is available only in curvature.

## Input domain and conventions

USD delta P&L per 1% commodity-price increase and raw vega. Buckets 1–17 or recognized names canonicalize to numeric IDs; distinct labels for the same bucket combine. No location, grade, basis, tenor or individual commodity survives the ordinary input grid.

## Ordered algorithm

1. Canonicalize labels, sort amounts inside each bucket and sum to net exposure. Public validation rejects unknown labels before the private grouping could skip them.
2. Delta uses net directly; vega uses net×HVR 0.74. Apply bucket-specific `CR=max(1,sqrt(abs(raw)/CT_bucket))`.
3. Delta RWs for buckets 1–17 are [48,29,33,25,35,30,60,52,68,63,21,21,15,16,13,68,17] percentage points; vega uses 0.55. The JSON supplies all 17 delta and vega CTs in absolute USD input units.
4. Form WS and aggregate using the validated 17×17 inter-bucket matrix; no additional concentration-ratio factor is applied to this ordinary inter-bucket calculation. Floor the quadratic expression before square root.
5. Delta/vega margins later add with commodity curvature. Detail rows preserve net pre-HVR sensitivity, weight, CR and weighted value; replay must include the component-level HVR.

## Selection, controls and limitations

The JSON's `other` weight alias is not a separate priced bucket; labels normalize to numeric bucket 16. Commodity within-bucket correlations from the registry apply to named factors in curvature, not to ordinary delta/vega already netted to one bucket amount. The 17×17 matrix includes negative and zero correlations, so signed bucket exposures matter.

## Alternatives and tradeoffs

A name/tenor/basis grid with within-bucket dependence would avoid treating all ordinary risk in one bucket as perfectly nettable. Retaining the current shape is simpler but must remain an explicit approximation with measured limits.

## Source and validation evidence

Historical literal tests pin all 17 delta weights, within-bucket curvature correlations and both sets of concentration thresholds. Registry tests verify matrix shape/PSD, not transcription of all 289 matrix cells. Inline tests locate commodity delta/vega and alias behavior.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [tests/simm_schedule_parity.rs](../../../../finstack-quant/margin/tests/simm_schedule_parity.rs), [src/registry/mod.rs](../../../../finstack-quant/margin/src/registry/mod.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
