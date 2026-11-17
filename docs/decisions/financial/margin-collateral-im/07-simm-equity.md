# MARGIN-COLLATERAL-IM-007 — SIMM equity delta and vega

**Decision status:** Accepted 2026-11-16 for checked SIMM equity delta and vega.

**Implementation status:** Existing behavior is described below. Bucket data, factor identity, valuation adapters, aggregation, portfolio and host reconciliation with the accepted contract are pending.

**Validation status:** Exact-rational checks found positive pivots in the published 12×12 classified correlation matrix. Independent Decimal examples checked classified and residual delta, equity HVR before concentration and bucket 12 vega. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked SIMM equity delta and vega

1. Use one historical USD calculation covering all 12 numbered buckets and explicit residual within the elected profile, qualified netting scope and product-class partition. Require dated authoritative factor classification. Ordinary equities use the historical region and sector rules and the inclusive USD 2 billion large-cap threshold on global capitalization of the entity or group. Residual requires a documented basis. Net only matching qualified spot or volatility factors. Reject missing or conflicting classifications. [ISDA SIMM v2.6, §§C.1 and G.1](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
2. For index, fund and ETF delta, elect direct bucket 11 treatment or a bilaterally agreed portfolio-wide allocation backed by qualified constituent sensitivities. Their ordinary vega stays in bucket 11. Require attributable constituent delta and vega for bespoke baskets. Route qualified volatility-index exposure to bucket 12 on its equity-volatility basis. [Record 010](10-simm-curvature.md) owns the published bucket 12 zero-curvature rule. [ISDA SIMM v2.6, §§B.11 and C.1](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
3. Use signed USD P&L from an upward 1% relative spot move for delta. For each vega expiry contribution use the historical proxy `RW_delta × sqrt(365/14) / Φ⁻¹(0.99)` in volatility percentage points and the matching signed vega per volatility point. Bind the proxy to the elected bucket and profile. Sum qualified signed products across expiries of the same factor for ordinary vega and retain expiry-resolved inputs for curvature. Equity TRS financing remains a separately qualified rate exposure. [Accepted equity TRS contract](../valuations-equity-and-composition/16-equity-trs.md#accepted-decision-for-checked-dated-equity-total-return-swaps) · [ISDA SIMM v2.6, §§B.10 and C](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
4. Use signed net delta as its concentration basis. For vega, apply HVR `0.60` to the signed proxy-weighted net before concentration. Compute `C = max(1, sqrt(abs(basis) / threshold))` and multiply the basis by `C` and the selected risk weight. Require complete bucket weights and thresholds, with historical vega weight `0.45` generally and `0.96` for bucket 12. [ISDA SIMM v2.6, §§B, G and J](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
5. Within each bucket use unit diagonals and apply `min(C₁,C₂)/max(C₁,C₂)` to the selected off-diagonal coefficient. Compute the bucket norm `K = sqrt(wᵀRw)` and `S = clamp(sum(w), −K, K)`. Across numbered buckets use the 66 admitted pair coefficients with `K²` diagonal terms and signed `S` cross-terms. Add residual `K` after the classified square root. Zero historical residual off-diagonals give `sqrt(sum(w²))` within residual. [ISDA SIMM v2.6, §G.2](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf) · [ORE residual aggregation](https://github.com/OpenSourceRisk/Engine/blob/master/OREAnalytics/orea/simm/simmcalculator.cpp#L1000-L1034).
6. Return separate checked reference-horizon delta and vega charges. Use fixed bucket and factor order, compensated sums and scaled quadratics. Establish effective PSD structurally or apply the [accepted read-only dependence gate](02-simm-parameter-validation.md#accepted-decision-for-checked-simm-parameter-profiles-and-correlation-validation). Return typed failures for invalid dependence, negative variance and unusable arithmetic. Preserve classification, source volatility, weighting, concentration and bucket evidence through the common Rust path for portfolio, MVA and hosts. [Record 010](10-simm-curvature.md) owns curvature, [record 011](11-simm-total-and-mpor.md) owns total assembly and MPOR, and [record 022](22-wire-tables-and-provenance.md) owns wire format.

Independent Decimal examples gave USD 3,142,992.20 for opposing `±100,000 USD` delta amounts per 1% spot move on distinct bucket 8 factors. Applying the residual parameters to those inputs gives USD 7,071,067.81. A raw residual vega of USD 260 million gives USD 156 million after HVR, a concentration multiplier of two and a USD 140.4 million charge.

## Choice and rationale

The observed calculation applies the residual weight to every equity name and zero cross-correlation between distinct names.

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

The accepted calculation uses the published classified-bucket dependence and a separately qualified residual. Independent portfolio comparison remains an open qualification item.

## Source and validation evidence

`audit_regressions.rs::simm_preserves_ir_direction_and_does_not_erase_unrelated_equities` checks a positive charge for opposite AAPL/MSFT amounts and the approximation flag. Parameter tests pin RW 50/VRW 0.45/HVR 0.60 and thresholds; these are selected literal and invariant checks.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs), [tests/simm_schedule_parity.rs](../../../../finstack-quant/margin/tests/simm_schedule_parity.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
