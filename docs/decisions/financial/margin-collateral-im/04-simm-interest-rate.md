# MARGIN-COLLATERAL-IM-004 — SIMM interest-rate delta and vega aggregation

**Decision status:** Accepted 2026-11-16 for checked SIMM interest-rate delta and vega aggregation.

**Implementation status:** Existing behavior is described below. Subcurve identity, checked aggregation, valuation adapters, portfolio and host reconciliation with the accepted contract are pending.

**Validation status:** Independent Decimal examples checked distinct subcurves, signed tenor and currency offsets, the currency sum cap, and vega weighting and concentration. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked SIMM interest-rate delta and vega aggregation

1. Elect the historical USD profile and qualified netting and product-class partition. Support multiple ordinary yield subcurves and retain `(risk currency, subcurve, vertex)` for delta. Net only matching economic factors. Required inflation or cross-currency basis risk produces a typed unsupported error under this ordinary yield profile. The valuation adapter supplies qualified shocks and exact vertices. [Accepted sensitivity contract](03-sensitivity-inputs.md#accepted-decision-for-checked-simm-sensitivity-identity-scaling-and-netting) · [Accepted margin repricing contract](../valuations-framework/margin-bridge.md#accepted-decision-for-checked-margin-repricing-and-identity) · [ISDA SIMM v2.6, §B.7](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
2. Form ordinary vega by multiplying each source vega by its matching ATM volatility, then sum underlying maturities at the same risk currency and option expiry. Preserve source maturity and volatility convention in the evidence. Apply the elected vega risk weight once. The bundled value is `0.23` and the ordinary IR vega HVR multiplier is one. Preserve curvature inputs separately under [record 010](10-simm-curvature.md). [ISDA SIMM v2.6, §§B.10 and D.1](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
3. Compute `C = max(1, sqrt(abs(sum(raw)) / threshold))` separately for each risk currency and component on the signed raw net before weighting. Use the selected profile's currency groups, thresholds and risk weights. For regular well-traded, regular less-traded, low and high volatility groups respectively, the historical delta thresholds are `330m`, `130m`, `61m`, `30m` USD/bp and vega thresholds are `4,900m`, `520m`, `970m`, `74m` USD. Form each weighted sensitivity `w = raw × risk_weight × C`. [ISDA SIMM v2.6, §§B.7, B.10 and J](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
4. For each currency, compute `K = sqrt(wᵀRw)` and `S = clamp(sum(w), −K, K)`. Delta uses the tenor correlation multiplied by the selected subcurve coefficient for distinct subcurves and by one for the same subcurve. Vega uses the expiry correlation table. For each component, take the square root of the sum of currency `K²` plus `2 × γ × min(C₁,C₂)/max(C₁,C₂) × S₁ × S₂` for every unordered currency pair. The historical intercurrency coefficient is `0.32`. [ISDA SIMM v2.6, §§B and D.2](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
5. Use fixed currency, factor and semantic tenor order, checked compensated sums, and scaled products and quadratics. Admit effective matrices with the [accepted read-only dependence gate](02-simm-parameter-validation.md#accepted-decision-for-checked-simm-parameter-profiles-and-correlation-validation) or a structural guarantee. Return typed errors for missing coefficients, invalid dependence, negative variance and unusable arithmetic. Preserve qualified zero and distinguish it from missing required inputs.
6. Publish separate reference-horizon delta and vega charges through one checked Rust path for direct, portfolio, MVA, Python and WASM callers. Retain the profile, calculation cut, concentration basis, weights, currency `K` and signed `S` in calculation evidence. [Record 011](11-simm-total-and-mpor.md) owns assembly and MPOR treatment. [Record 022](22-wire-tables-and-provenance.md) owns wire format.

Independent examples using the bundled historical inputs gave USD 709.93 for opposite USD 5Y sensitivities of `±100 USD/bp` on distinct subcurves, USD 6,997.14 for opposite USD and EUR 5Y sensitivities of `±100 USD/bp`, and USD 230 for USD 1,000 of raw volatility-weighted IR vega below its concentration threshold.

## Choice and rationale

The observed path aggregates rates within each currency and then across currencies using signed bucket sums and concentration differences. Its two-dimensional delta and vega inputs do not preserve subcurves or both volatility coordinates.

## Input domain and conventions

Inputs are finite USD amounts: delta per bp, vega before risk weighting. Tenors are 2W,1M,3M,6M,1Y,2Y,3Y,5Y,10Y,15Y,20Y,30Y. Risk currency identifies the rate factor, not the amount's denomination.

## Ordered algorithm

1. Group by currency and sort tenor labels. Select group: USD/EUR/GBP regular well traded; AUD/CAD/CHF/DKK/HKD/KRW/NOK/NZD/SEK/SGD/TWD regular less traded; JPY low; every other currency high.
2. Compute a currency concentration multiplier `CR=max(1,sqrt(abs(sum(raw))/CT))`. Delta CTs are 330m/130m/61m/30m for regular well/less/low/high; vega CTs are 4.9bn/520m/970m/74m, in USD input units.
3. Delta uses the group's tenor weights (e.g. 5Y regular 60, low 23, high 97 bp). Vega uses a flat weight 0.23. Form `WS=raw*RW*CR`.
4. `K_c=sqrt(max(0,sum_i,j rho_ij*WS_i*WS_j))`, using the 12-tenor table. Cap `S_c=clamp(sum WS,-K_c,K_c)`.
5. Cross-currency margin is `sqrt(max(0,sum K_c²+2 sum_(c<d) 0.32*(min(CR_c,CR_d)/max(CR_c,CR_d))*S_c*S_d))`. Delta and vega run independently. Their nonnegative margins are added later within IR risk class.

## Selection, controls and limitations

All group weights and thresholds are registry-backed. The IR subcurve correlation 0.993 is used by curvature, not this collapsed delta/vega path. IR HVR 0.47 is not multiplied into these vega inputs; the curvature adjustment is separate. Unknown tenors are blocked by public validation before the private filter-map could omit them.

## Alternatives and tradeoffs

The accepted delta calculation preserves distinct rate subcurves. The accepted ordinary vega reduction retains source maturity in the evidence and aggregates each expiry after source volatility weighting. Independent portfolio comparison remains an open qualification item.

## Source and validation evidence

`production_simm_csa_audit.rs` checks the 60/23/97 single-tenor weights and concentration at four times the USD threshold using hand formulas. `simm_schedule_parity.rs` pins all three 12-weight tables and 66 tenor pairs. `audit_regressions.rs` checks that opposite currency exposures reduce the charge. Tests were inspected, not run.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [tests/production_simm_csa_audit.rs](../../../../finstack-quant/margin/tests/production_simm_csa_audit.rs), [tests/simm_schedule_parity.rs](../../../../finstack-quant/margin/tests/simm_schedule_parity.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
