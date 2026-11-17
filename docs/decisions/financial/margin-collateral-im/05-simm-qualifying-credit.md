# MARGIN-COLLATERAL-IM-005 — SIMM qualifying-credit issuer and sector aggregation

**Decision status:** Accepted 2026-11-16 for checked SIMM qualifying-credit delta and vega.

**Implementation status:** Existing behavior is described below. Classification, factor identity, valuation adapter, aggregation, portfolio and host reconciliation with the accepted contract are pending.

**Validation status:** Independent Decimal examples checked raw concentration, payment currency and seniority dependence, vega weighting, residual aggregation and effective correlation failure. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked SIMM qualifying-credit delta and vega

1. Elect the historical USD profile and a qualified netting and product-class partition. Require authoritative dated classification into one of the 12 classified buckets or explicit residual. Reject missing or conflicting assignments. For spread delta net only qualified exact factors with the same issuer, seniority, contractual payment currency and one of `1Y`, `2Y`, `3Y`, `5Y` or `10Y` spread vertices. Keep contractual payment currency distinct from USD amount denomination. [Accepted margin repricing contract](../valuations-framework/margin-bridge.md#accepted-decision-for-checked-margin-repricing-and-identity) · [ISDA SIMM v2.6, §§C.1 and E.1](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
2. Require attributable constituent repricing for qualifying index and basket delta and preserve index family, series and version in source evidence. Admit directly qualified index vega with a distinct typed index-volatility identity and a documented bucket, explicitly residual for cross-sector indexes. Use same-group dependence for the same qualified index identity. Required base-correlation risk produces a typed unsupported error for this spread calculation. [Accepted CDS and index risk](../models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk) · [ISDA SIMM v2.6, §§B and C.1](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
3. Use option expiry on the five-vertex grid for ordinary qualifying-credit vega and preserve underlying maturity as a separate source coordinate. Form each signed `σ × ∂PV/∂σ` using the matching ATM convention before summing qualified contributions at a common expiry. Apply the elected vega risk weight once, `0.76` for the bundled profile, with an ordinary credit HVR multiplier of one. [Record 003](03-sensitivity-inputs.md#accepted-decision-for-checked-simm-sensitivity-identity-scaling-and-netting) carries this clarification. [Record 010](10-simm-curvature.md) owns curvature expiry weighting. [ISDA SIMM v2.6, §§B.10 and E.1](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf) · [ISDA Risk Data Standards, §2.8](https://www.isda.org/a/owEDE/risk-data-standards-v1-36-public.pdf).
4. Calculate `C = max(1, sqrt(abs(sum(raw)) / threshold))` independently for delta and vega by issuer and seniority across vertices and payment currencies, or by qualified index-volatility identity for index vega. Apply `C` and the selected risk weight to every separately retained factor. The bundled delta thresholds are USD 1 million/bp for the two sovereign buckets and USD 170,000/bp elsewhere, including residual. The vega threshold is USD 360 million. [ISDA SIMM v2.6, §§B and J](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
5. Use unit diagonals and, for historical off-diagonals, `0.93` for distinct factors in the same group, `0.46` for different groups and `0.50` for distinct residual factors. Multiply each off-diagonal by `min(C₁,C₂)/max(C₁,C₂)`. Compute every bucket norm `K = sqrt(wᵀRw)` and signed capped sum `S = clamp(sum(w), −K, K)`. Across non-residual buckets use the 66 admitted pair coefficients, with `K²` on the diagonal and signed `S` in cross-terms. Add the residual bucket norm linearly after the non-residual square root. [ISDA SIMM v2.6, §E.2](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf) · [ORE residual aggregation](https://github.com/OpenSourceRisk/Engine/blob/master/OREAnalytics/orea/simm/simmcalculator.cpp#L1000-L1034).
6. Return separate checked delta and vega charges at the reference horizon. Use fixed bucket and factor order, checked compensated sums and scaled quadratics. Require a structural PSD guarantee or apply the [accepted read-only effective dependence gate](02-simm-parameter-validation.md#accepted-decision-for-checked-simm-parameter-profiles-and-correlation-validation). Return typed errors for invalid dependence, negative variance and unusable arithmetic. Carry classification and factor evidence, raw concentration totals, weights, bucket norms and the residual charge through the same Rust path for portfolio and hosts. [Record 011](11-simm-total-and-mpor.md) owns assembly and MPOR. [Record 022](22-wire-tables-and-provenance.md) owns wire format.

Independent Decimal examples gave USD 13,469,966.59 on `±400,000 USD/bp` of Financial delta for one issuer and seniority across distinct payment currencies, with an exact zero raw concentration net. A Financial, Sovereign and Residual example gave USD 16,881,975.85 with the residual charge added after the square root.

## Choice and rationale

The observed calculation uses 12 classified buckets plus residual and retains a name across tenor entries. Its name-and-tenor key cannot distinguish seniority or contractual payment currency.

## Input domain and conventions

Signed USD CS01 per bp for delta and compatible raw volatility-weighted vega for vega. Sector assignment is caller supplied, including investment-grade/high-yield distinctions. Missing sector is not inferred by the calculator.

## Ordered algorithm

1. Group and sort `(name,tenor)` entries within each sector. Sum raw amounts per name across tenors.
2. For delta, `CR_name=max(1,sqrt(abs(raw_name)/CT_sector))`: sovereign/high-yield-sovereign CT=1m, other sectors/residual CT=170k. Delta weights by the enum order are 75,90,84,54,62,48,185,343,255,250,214,173,343 bp. For vega, use CQ CT=360m and RW=0.76.
3. Form entry `WS=raw*RW*CR`. Within a nonresidual sector use correlation 0.93 for same issuer, 0.46 for different issuers, multiplied by `min(CR_i,CR_j)/max(CR_i,CR_j)`; residual uses 0.50 regardless of name. Quadratic diagonal is always 1.
4. Compute sector K and cap the signed sum to ±K. Combine nonresidual sectors with the registry's pairwise correlations and floored quadratic norm.
5. Add residual K linearly outside sector diversification. Delta and vega produce independent component margins that add within the credit-qualifying risk class.

## Selection, controls and limitations

Default same-issuer correlation is below one even for separate tenors of a single issuer. Exact identical keys already net during input construction. The public ordinary credit maps admit every general SIMM tenor; the numerical formula has no separate tenor-correlation term. OtcMarginSpec::validate_for_credit requires an explicit qualifying sector or nonqualifying choice when SIMM is selected.

## Alternatives and tradeoffs

The accepted calculation preserves exact single-name factors and separately qualified index volatility factors. The classification source determines the bucket before margin aggregation.

## Source and validation evidence

The inspected vector `credit_concentration_uses_raw_name_sensitivity` gives 100k Financial CS01 × 90 = 9m below its raw threshold. Inline SIMM tests exercise CQ issuer, sector, residual, vega and concentration behavior. Historical primary-source comparison and the independent examples above cover specified formulas and literals. Runtime and host execution remain open.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/types/otc.rs](../../../../finstack-quant/margin/src/types/otc.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [tests/production_simm_csa_audit.rs](../../../../finstack-quant/margin/tests/production_simm_csa_audit.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
