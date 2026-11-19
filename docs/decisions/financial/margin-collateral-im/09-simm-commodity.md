# MARGIN-COLLATERAL-IM-009 — SIMM commodity delta and vega

**Decision status:** Accepted 2026-11-16 for checked SIMM commodity delta and vega.

**Implementation status:** Existing behavior is described below. Exact factor inputs, qualified source mappings, within-bucket aggregation, portfolio and host reconciliation with the accepted contract are pending.

**Validation status:** Independent comparisons matched the 272 published off-diagonal entries, confirmed the 17 unit diagonals and found positive exact-rational pivots. Decimal examples checked exact-factor delta, the signed cap, bucket 16's position in the outer quadratic and vega concentration. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked SIMM commodity delta and vega

1. Elect the historical USD profile within the qualified netting and product-class partition. Admit a dated, qualified spot or volatility factor with numeric bucket `1–17`. Preserve benchmark, grade and location where they distinguish economic factors. Delta is signed USD P&L for `P → 1.01P`. Map forward exposures to the same spot factor only through a qualified parallel-curve shock or an established equivalent. Retain the source curve and delivery evidence. [Accepted commodity delivery contract](../valuations-fx-and-exotics/commodity-delivery-listed.md#accepted-decision-for-commodity-delivery-and-listed-contracts) · [ISDA SIMM v2.6, §§C.1–C.2](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
2. Elect direct bucket 17 index delta or qualified constituent delta consistently across the portfolio. Index vega stays direct in bucket 17. Bespoke basket delta requires constituent attribution. Bespoke basket vega requires qualified constituent volatility factors and returns a typed unsupported error when the required allocation cannot be established. [Accepted commodity spread contract](../valuations-fx-and-exotics/commodity-spread.md#accepted-decision-for-commodity-spread-pricing-and-settlement) · [ISDA SIMM v2.6, §C.1 ¶18](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
3. Form each expiry vega contribution with the elected proxy `RW_delta × sqrt(365/14) / Φ⁻¹(0.99)` in volatility percentage points and the matching signed vega per volatility point. Use the bucket's delta weight for the proxy, including `17` for index vega. Sum signed products across qualified expiries of the same factor and apply HVR `0.74` once. Compute `C = max(1, sqrt(abs(basis) / threshold))` separately per factor on signed net delta or HVR-adjusted vega. Apply `C` with the elected delta weight or vega weight `0.55`. Preserve expiry-resolved inputs for curvature under [record 010](10-simm-curvature.md). [ISDA SIMM v2.6, §§B, H and J](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
4. Use unit diagonals and multiply each within-bucket off-diagonal by `min(C₁,C₂)/max(C₁,C₂)`. Compute each bucket norm `K = sqrt(wᵀRw)` and signed capped sum `S = clamp(sum(w), −K, K)`. Across buckets use the admitted 17×17 matrix with `K²` diagonal terms and signed `S` cross-terms. Concentration attenuation applies only within buckets. Bucket 16, Other, contributes `K₁₆²` inside the root and has zero historical cross-links. [ISDA SIMM v2.6, §§B and H.2](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
5. Return separate checked delta and vega charges at the reference horizon. Require complete bucket parameters, fixed bucket and factor order, compensated sums and scaled quadratics. Establish effective PSD structurally or apply the [accepted read-only dependence gate](02-simm-parameter-validation.md#accepted-decision-for-checked-simm-parameter-profiles-and-correlation-validation). Return typed errors for missing required factors, invalid dependence, negative variance and unusable arithmetic. Carry source mapping, proxy, HVR, concentration, weights and signed bucket evidence through the common Rust path for portfolio, MVA and hosts. [Record 011](11-simm-total-and-mpor.md) owns assembly and MPOR. [Record 022](22-wire-tables-and-provenance.md) owns wire format.

Independent Decimal examples gave USD 710.35 for distinct WTI and Brent factors with opposing `±100 USD` delta amounts per 1% move using bucket 2 parameters. Single Crude and Other factors each with `+100 USD` per 1% move gave USD 7,392.56 in quadrature.

## Choice and rationale

The observed calculation reduces ordinary delta and vega to one signed total per bucket before weighting. The observed curvature input can carry a named commodity factor.

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

The accepted calculation retains distinct qualified commodity factors and applies within-bucket dependence. Source delivery and curve evidence support the qualified parallel-shock projection. Independent portfolio comparison remains an open qualification item.

## Source and validation evidence

Historical literal tests pin all 17 delta weights, within-bucket correlations and both sets of concentration thresholds. Registry tests check matrix shape and numerical PSD admission. A separate primary-source comparison matched the full inter-bucket matrix with the implied unit diagonals. Inline tests cover commodity delta, vega and alias behavior. Runtime and host execution remain open.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [tests/simm_schedule_parity.rs](../../../../finstack-quant/margin/tests/simm_schedule_parity.rs), [src/registry/mod.rs](../../../../finstack-quant/margin/src/registry/mod.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
