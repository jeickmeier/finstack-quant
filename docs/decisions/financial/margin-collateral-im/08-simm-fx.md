# MARGIN-COLLATERAL-IM-008 — SIMM FX delta and pair-vega aggregation

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: USD FX delta is excluded as calculation-currency risk; other currencies retain signed exposures. Vega canonicalizes opposite pair order without flipping the amount sign. Proposed rationale: aggregate a volatility factor independently of quoting order, while preserving directional delta.

## Input domain and conventions

Delta is USD P&L per 1% relative FX-price rise. Vega is a USD volatility-weighted amount. The calculator performs no FX conversion. BRL/RUB/TRY form the high-volatility delta set; all other non-USD currencies use the regular weight.

## Ordered algorithm

1. Drop USD delta rows. Select delta RW=14.7 for high-volatility currencies, otherwise 7.4.
2. Select concentration category: 1={USD, EUR, JPY, GBP, AUD, CHF, CAD}; 2={BRL, CNY, HKD, INR, KRW, MXN, NOK, NZD, RUB, SEK, SGD, TRY, ZAR}; all others category3. Delta CTs are 3.3bn,880m,170m. Apply `CR=max(1,sqrt(abs(raw)/CT))` and WS=raw×RW×CR.
3. Aggregate delta with regular/regular correlation 0.50, regular/high 0.25, high/high -0.05, each multiplied by concentration min/max ratio. Floor variance at zero.
4. For vega, sort each currency pair and sum any reversed-pair entries in deterministic amount order. Multiply net vega by HVR 0.57, apply category-pair CT (1_1 2.8bn;1_2 1.4bn;1_3 590m;2_2 520m;2_3 340m;3_3 210m) and VRW 0.48.
5. Pair vegas aggregate with hard-coded 0.50 off-diagonal correlation times concentration min/max ratio. Delta and vega margins add later within FX.

## Selection, controls and limitations

FX category and high-volatility membership are code, not overlayable data. The vega 0.5 pair correlation is also code. Ordinary FX vega allows same-currency pairs, while curvature's FX factor parser requires two distinct currencies; these entry-point domains differ.

Historical currency-group membership does not establish present-market classification.

## Alternatives and tradeoffs

A quote-aware pair-vega normalization layer could verify how reversed quotes affect the supplied sensitivity before aggregation. Expiry-resolved ordinary vega would retain more information. A non-USD calculation basis would require corresponding threshold/numeraire rules, not merely changing the output currency.

## Source and validation evidence

Inspected independent formula vectors check EUR×7.4, BRL×14.7 and zero USD delta. Inline test checks EUR/+100, JPY/-100 partial offset at rho0.5. Table tests pin category thresholds and HVR/weights; no complete cross-pair benchmark was established.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [src/types/simm_curvature.rs](../../../../finstack-quant/margin/src/types/simm_curvature.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [tests/production_simm_csa_audit.rs](../../../../finstack-quant/margin/tests/production_simm_csa_audit.rs), [tests/simm_schedule_parity.rs](../../../../finstack-quant/margin/tests/simm_schedule_parity.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
