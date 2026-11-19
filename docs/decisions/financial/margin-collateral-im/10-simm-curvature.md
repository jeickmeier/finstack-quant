# MARGIN-COLLATERAL-IM-010 — SIMM expiry scaling and curvature aggregation

**Decision status:** Accepted 2026-11-17 for checked SIMM expiry scaling and curvature aggregation.

**Implementation status:** Existing behavior is described below. Classified factors, expiry grids, calculation perspective, checked arithmetic, replay evidence and host reconciliation with the accepted contract are pending.

**Validation status:** Independent arithmetic checked expiry weighting before netting, signed charge asymmetry, squared FX correlation, commodity Other, the normal quantile and the IR adjustment. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked SIMM expiry scaling and curvature aggregation

1. Bind the elected historical USD profile, valuation cut, netting scope, product-class partition and calculation side. Admit signed `σ × vega` before expiry scaling, ordinary HVR, vega risk weights and concentration. Rates and credit use matching ATM volatility and derivative conventions. Equity, FX and commodity use the elected proxy and matching volatility units. Evaluate the selected side once. An opposite-side result requires explicit sign conversion and reevaluation. Distinguish missing applicable curvature from qualified inapplicability and evaluated zero. [Accepted sensitivity contract](03-sensitivity-inputs.md#accepted-decision-for-checked-simm-sensitivity-identity-scaling-and-netting) · [ISDA SIMM v2.6, §§B.9–11 and C.3](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf) · [ORE side handling](https://github.com/OpenSourceRisk/Engine/blob/master/OREAnalytics/orea/simm/simmcalculator.cpp#L1049-L1104).
2. Require an owner-qualified exact factor mapping and retain the applicable IR or credit risk vertex and underlying source maturity separately from option expiry. Admit credit expiry labels `1Y`, `2Y`, `3Y`, `5Y` and `10Y`, and the twelve standard labels for IR, equity, FX and commodity. Reject unsupported labels or mappings before scaling, including on explicit zero rows. This selects class-specific expiry grids while the observed validator accepts twelve credit expiry labels. [ORE v2.6 expiry configuration](https://github.com/OpenSourceRisk/Engine/blob/master/OREAnalytics/orea/simm/simmconfigurationisdav2_6.cpp#L255-L281).
3. Apply `SF(t) = 0.5 × min(1, 14/t)` to each contribution using fixed calendar days, with `2W = 14`, each month `365/12` days and each year `365` days. Net scaled contributions only by the exact qualified factor, using checked compensated sums in declared contribution order. Preserve row multiplicity and that order through export and restoration. [ISDA SIMM v2.6, §B.11(a)–(b)](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
4. Use the applicable base factor and bucket dependence from records 004–009, including classified nonqualifying credit and equity. Square the base coefficients as `ρ²` within buckets and `γ²` across buckets and retain unit diagonals. Apply ordinary concentration attenuation only in its delta and vega owners. Compute each bucket norm `K` and cap its signed sum at `±K`. The outer quadratic has `K²` diagonals and capped signed sums in its cross-terms. FX uses one bucket of distinct canonical pairs. Commodity Other stays in the numbered quadratic and its signed totals. Qualified equity bucket 12 has prescribed zero and is excluded from norms and sign totals. [ISDA SIMM v2.6, §B.11 and §§D–I](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
5. For the non-residual group and separately any applicable residual group, define `N` as the signed sum of net factor exposures and `A` as the sum of their absolute values. Exact `A = 0` gives zero. Otherwise use `θ = min(N/A, 0)`, `λ = (z²−1)(1+θ)−θ` and the financial charge floor `max(N + λK_group, 0)`. Pin `z = Φ⁻¹(0.995)` to nearest binary64 `2.575829303548901`. Add the separately evaluated residual charge. Multiply completed IR curvature by `HVR_IR⁻²`, with historical `HVR_IR = 0.47`. [ISDA SIMM v2.6, §B.11(d)](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
6. Establish PSD structurally or apply the [accepted read-only dependence gate](02-simm-parameter-validation.md#accepted-decision-for-checked-simm-parameter-profiles-and-correlation-validation) to the squared effective matrix. Use scaled checked quadratics and return typed errors for negative computed variance, invalid dependence or unusable arithmetic. Preserve the profile, side, expiry weights, factor nets, bucket norms and caps, `N`, `A`, `θ`, `λ` and any prescribed-zero reason through the common Rust path. [Record 011](11-simm-total-and-mpor.md) owns total assembly and MPOR. [Record 022](22-wire-tables-and-provenance.md) owns wire format.

Independent arithmetic on one qualified factor gave net curvature exposure of USD 480.8219 from raw `+1,000 USD` at 2W and `−1,000 USD` at 1Y. The single-factor equity example charged USD 3,190.20. A single net curvature exposure of `+100 USD` gave USD 663.49 and `−100 USD` gave zero.

## Choice and rationale

The observed calculation consumes signed volatility-weighted vega rows and applies expiry scaling before factor netting. It currently supports fewer classifications and has different summation and failure boundaries from the accepted contract.

## Input domain and conventions

Input amount is signed USD `sigma*dPV/dsigma` before SF, HVR, vega RW or concentration. IR bucket is currency, factor is subcurve and risk tenor is required. CQ requires sector and issuer; CNQ/equity require residual bucket; commodity uses a recognized bucket/name; FX requires bucket `fx` and distinct currency pair. Credit risk tenors are restricted to 1Y/2Y/3Y/5Y/10Y. Expiry must be one of the 12 SIMM tenors.

## Ordered algorithm

1. Validate and canonicalize currencies, credit/commodity buckets and sorted FX pair. Convert expiry to fixed calendar days:2W=14, months=365/12 each, years=365 each. Compute `CVR=raw_vega*0.5*min(1,14/days)`.
2. Group by risk class, bucket, factor and risk tenor (expiry is removed only after scaling). Sort contributions by total numeric order and sum; reject nonfinite factor sums.
3. Within bucket compute K with squared factor correlation. IR correlation is tenor correlation times 1 for same subcurve or0.993 otherwise; CQ uses same-name 0.93/different-name 0.46 or residual 0.50; CNQ 0.50; equity 0; commodity its per-bucket correlation; FX 0.50.
4. Store sum, absolute sum and capped signed sum `S=clamp(sum,-K,K)`. For nonresidual buckets combine K² and squared bucket correlations times capped signed sums. Bucket correlations are IR 0.32, CQ table, commodity matrix; otherwise 0.
5. Define `theta=min(sum/abs_sum,0)` and `lambda=(z²-1)*(1+theta)-theta`, z=2.5758293035489004. Charge=`max(sum+lambda*norm,0)`; abs_sum 0 returns 0. Apply this separately to residual buckets and add their charges to the nonresidual charge.
6. Divide IR curvature by HVR 0.47². Reject nonfinite class margin. Add curvature within each class before cross-class aggregation.

## Selection, controls and limitations

There is no concentration multiplier in this curvature kernel and no multiplication by ordinary vega risk weights. Detailed rows expose already-SF-scaled factor amounts. Factor sums and key order are canonicalized, but the final unguarded floating sums can still require numerical qualification for extreme data.

## Alternatives and tradeoffs

The accepted calculation preserves source option expiry until scaling and qualified risk identity until factor netting. The independent valuation mapping remains part of the input qualification. Complete portfolio and host comparison remains open.

## Source and validation evidence

Inspected hand test uses +1000 at 2W and -1000 at 1Y: expected equity charge `(500-7000/365)*z²`, invariant to input reversal; IR divides that by 0.47². Other inline tests cover class-before-cross-class aggregation and invalid expiry. Source provenance is recorded locally; no licensed comparison was run.

Implementation and data: [src/calculators/im/simm/curvature.rs](../../../../finstack-quant/margin/src/calculators/im/simm/curvature.rs), [src/types/simm_curvature.rs](../../../../finstack-quant/margin/src/types/simm_curvature.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [tests/production_simm_csa_audit.rs](../../../../finstack-quant/margin/tests/production_simm_csa_audit.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
