# MARGIN-COLLATERAL-IM-004 — SIMM interest-rate delta and vega aggregation

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: rates are aggregated within each currency, then across currencies while preserving the sign of the bucket sum. Proposed rationale: retain partial offsets and adjust dependence when concentration differs. The two-dimensional delta/vega shape does not resolve subcurves or both option and underlying maturity.

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

A full subcurve and expiry/underlying grid would retain more risk distinctions. Netted scalar duration would be smaller to implement but would discard the term-structure and currency dependence retained here. A licensed historical/full-model comparison would measure either approximation.

## Source and validation evidence

`production_simm_csa_audit.rs` checks the 60/23/97 single-tenor weights and concentration at four times the USD threshold using hand formulas. `simm_schedule_parity.rs` pins all three 12-weight tables and 66 tenor pairs. `audit_regressions.rs` checks that opposite currency exposures reduce the charge. Tests were inspected, not run.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [tests/production_simm_csa_audit.rs](../../../../finstack-quant/margin/tests/production_simm_csa_audit.rs), [tests/simm_schedule_parity.rs](../../../../finstack-quant/margin/tests/simm_schedule_parity.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
