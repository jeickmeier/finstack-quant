# MARGIN-COLLATERAL-IM-006 — SIMM nonqualifying-credit delta and vega

**Decision status:** Accepted 2026-11-16 for checked SIMM nonqualifying-credit delta and vega.

**Implementation status:** Existing behavior is described below. Classification, factor identity, valuation adapter, aggregation, portfolio and host reconciliation with the accepted contract are pending.

**Validation status:** Independent Decimal examples checked tranche concentration, classified and residual delta, cross-bucket vega and linear residual addition. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked SIMM nonqualifying-credit delta and vega

1. Elect the historical USD profile within a qualified netting and product-class partition. Require dated authoritative classification into bucket 1 for investment-grade RMBS/CMBS, bucket 2 for high-yield or unrated RMBS/CMBS, or explicit residual with a documented basis. Net delta by qualified issuer, tranche and spread vertex on `1Y`, `2Y`, `3Y`, `5Y` or `10Y`. For a nonqualifying securitization, the valuation owner shocks the instrument or tranche spread. Preserve source, series and native currency evidence. [Accepted margin repricing contract](../valuations-framework/margin-bridge.md#accepted-decision-for-checked-margin-repricing-and-identity) · [ISDA SIMM v2.6, §§C and F](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
2. Ordinary nonqualifying-credit vega uses option expiry on the five-vertex grid and carries underlying maturity in the source evidence. Form signed `σ × ∂PV/∂σ` with matching ATM volatility and derivative units before summing qualified contributions at the same expiry. Direct index vega retains its own qualified volatility identity and documented bucket, explicitly residual for cross-sector indexes. Apply the elected vega risk weight once, `0.76` in the bundled historical profile, with a credit HVR multiplier of one. [Record 003](03-sensitivity-inputs.md#accepted-decision-for-checked-simm-sensitivity-identity-scaling-and-netting) carries the expiry clarification. [ISDA SIMM v2.6, §§B.10, C.1 and F.1](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
3. Compute `C = max(1, sqrt(abs(sum(raw)) / threshold))` independently for delta and vega on signed raw totals across vertices of the same qualified issuer and tranche or the same qualified index-volatility identity. Store nonqualifying-credit weights, thresholds and dependence in the elected profile. Historical delta uses `280 bp` and USD `9.5 million/bp` for bucket 1, and `1,300 bp` and USD `500,000/bp` for bucket 2 and residual. Vega uses a USD `70 million` threshold in all three buckets. Apply the selected weight and `C` to each retained factor. [ISDA SIMM v2.6, §§B, F and J](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
4. Require the applicable correlation group for classified buckets, such as CMBX or ABX, independently of the tranche used for concentration. Use unit diagonals and historical off-diagonals `0.83` for the same classified group, `0.32` for different classified groups and `0.50` for distinct residual factors. Apply `min(C₁,C₂)/max(C₁,C₂)` to each off-diagonal. Compute `K = sqrt(wᵀRw)` and `S = clamp(sum(w), −K, K)` per bucket. Combine buckets 1 and 2 using `K²` diagonal terms and `2 × 0.43 × S₁ × S₂` at the historical correlation. Add residual `K` after the classified square root. [ISDA SIMM v2.6, §F.2](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf) · [ORE residual aggregation](https://github.com/OpenSourceRisk/Engine/blob/master/OREAnalytics/orea/simm/simmcalculator.cpp#L1000-L1034).
5. Return checked delta and vega charges at the reference horizon. Use fixed bucket and factor order, compensated sums and scaled quadratics. Establish effective PSD structurally or apply the [accepted read-only dependence gate](02-simm-parameter-validation.md#accepted-decision-for-checked-simm-parameter-profiles-and-correlation-validation). Return typed errors for missing classification, invalid dependence, negative variance and unusable arithmetic. Preserve factor, bucket, concentration and aggregation evidence through the same Rust path for portfolio, MVA and hosts. A complete margin result requires all applicable components. [Record 010](10-simm-curvature.md) owns curvature, [record 011](11-simm-total-and-mpor.md) owns assembly and MPOR, and [record 022](22-wire-tables-and-provenance.md) owns wire format.

Independent Decimal examples gave USD 14 million using the bucket 1 weight on `50,000 USD/bp` of investment-grade RMBS/CMBS delta and USD 65 million using the residual weight. Residual `±600,000 USD/bp` at distinct vertices of the same tranche gave USD 780 million. The same exposures on separate tranches gave USD 854,447,189.71 because each tranche concentrated independently.

## Choice and rationale

The observed input lacks a nonqualifying-credit bucket, so the implementation uses one residual calculation and groups concentration by name.

## Input domain and conventions

USD credit delta per bp and raw vega, finite amounts, nonempty issuer/index IDs and supported ordinary tenor labels. Nonqualifying classification itself is explicit and cannot carry a CQ sector in its wire representation.

## Ordered algorithm

1. Sort name/tenor entries and sum raw sensitivity per name.
2. Delta uses CT=500,000 and RW=1,300 bp; vega uses CT=70m and RW=0.76. `CR_name=max(1,sqrt(abs(net_name)/CT))`.
3. Weight each original tenor entry with its name's CR. Aggregate using `rho_ij=0.50*min(CR_i,CR_j)/max(CR_i,CR_j)` off diagonal, including different tenors of the same name; diagonal is 1.
4. Floor the quadratic expression at zero before square root. The resulting delta and vega margins later add with CNQ curvature before cross-class aggregation.

## Selection, controls and limitations

This residual classification is an explicit supported limitation, reflected in the final approximate flag. There is no direct credit-spread curve construction, rating migration, default simulation or collateral valuation here. Exact-key offsets occur before concentration; multi-tenor offsets still enter the correlation calculation.

## Alternatives and tradeoffs

The accepted calculation represents all three historical buckets, with the tranche determining concentration and a separately qualified group determining classified dependence. Independent portfolio comparison remains an open qualification item.

## Source and validation evidence

Inline test `credit_non_qualifying_delta_calculation` checks 50k×1,300. Historical literal tests pin residual delta/vega weights and thresholds. This is formula/parameter evidence, not a reference implementation comparison.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/types/otc.rs](../../../../finstack-quant/margin/src/types/otc.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [tests/simm_schedule_parity.rs](../../../../finstack-quant/margin/tests/simm_schedule_parity.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
