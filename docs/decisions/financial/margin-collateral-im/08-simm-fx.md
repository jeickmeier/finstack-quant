# MARGIN-COLLATERAL-IM-008 — SIMM FX delta and pair-vega aggregation

**Decision status:** Accepted 2026-11-16 for checked SIMM FX delta and pair vega.

**Implementation status:** Existing behavior is described below. Qualified factor inputs, complete proxy parameters, checked dependence, portfolio and host reconciliation with the accepted contract are pending.

**Validation status:** Independent arithmetic examples checked directed shocks, signed high-volatility delta, pair-specific proxy weighting, vega concentration and indefinite custom dependence. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked SIMM FX delta and pair vega

1. Elect the historical USD profile within a qualified netting and product-class partition. Key delta by non-USD risk currency and use signed USD P&L from an upward 1% shock to USD per unit of that currency, including translation exposure. The valuation owner reprices affected crosses coherently. A 1% rise in a directed quote changes the reciprocal by about `−0.990099%`. Reject every USD self-delta row at admission, including an explicit zero. [Accepted sensitivity contract](03-sensitivity-inputs.md#accepted-decision-for-checked-simm-sensitivity-identity-scaling-and-netting) · [ISDA SIMM v2.6, §§C.1–C.2](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
2. Use the historical high-volatility grouping for BRL, RUB and TRY and regular grouping for other admitted currencies. In the USD delta calculation the historical weights are `14.7` and `7.4` respectively. Apply the separate published three-category concentration map and thresholds of USD `3.3 billion`, `880 million` and `170 million` per 1% move. Compute `C = max(1, sqrt(abs(signed net) / threshold))` per currency before applying its risk weight. [ISDA SIMM v2.6, §§I.1 and J.5](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
3. Canonicalize vega to an alphabetically ordered unordered pair of distinct currencies at admission. Pairs containing USD are eligible. Retain the directed source quote, volatility convention and expiry as evidence. Qualified contributions on the same volatility basis keep their signs when the pair key is reordered. Form each signed expiry contribution with `σ_proxy × vega` and matching volatility units, using `σ_proxy = RW_pair × sqrt(365/14) / Φ⁻¹(0.99)` in volatility percentage points. The two pair members select `7.4`, `14.7` or `21.4` for regular/regular, mixed or high/high. [ISDA SIMM v2.6, §§B.10, C.3 and I.1](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
4. Sum qualified signed expiry products for each canonical pair. Apply HVR `0.57` once before concentration. Use historical thresholds of USD `2.8 billion`, `1.4 billion`, `590 million`, `520 million`, `340 million` and `210 million` for `(1,1)`, `(1,2)`, `(1,3)`, `(2,2)`, `(2,3)` and `(3,3)` respectively. Apply vega risk weight `0.48` after computing concentration. Preserve expiry-resolved contributions for curvature under [record 010](10-simm-curvature.md). [ISDA SIMM v2.6, §§B.10, I.1 and J.10](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
5. Use one signed quadratic per component with unit diagonal and concentration attenuation `min(C₁,C₂)/max(C₁,C₂)` on distinct-factor coefficients. Historical delta uses `0.50`, `0.25` and `−0.05` for regular/regular, mixed and high/high. Historical ordinary vega uses `0.50` for every distinct pair. Require complete weights, thresholds and dependence in the elected profile. The three-member historical high-volatility group supports a structural PSD guarantee. Apply the [accepted read-only effective dependence gate](02-simm-parameter-validation.md#accepted-decision-for-checked-simm-parameter-profiles-and-correlation-validation) where a structural guarantee is unavailable. [ISDA SIMM v2.6, §§B and I.2](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
6. Return separate checked delta and vega charges at the reference horizon. Use fixed factor order, compensated sums and scaled quadratics. Return typed errors for invalid dependence, negative variance and unusable arithmetic. Carry the elected profile, shock or volatility basis, concentration and weighted-factor evidence through the common Rust path for portfolio, MVA and hosts. [Record 011](11-simm-total-and-mpor.md) owns assembly and MPOR. [Record 022](22-wire-tables-and-provenance.md) owns wire format.

Independent arithmetic gave USD 2,130.23 for opposing BRL and RUB deltas of `±100 USD` per 1% and USD 2,026.26 for equal signs. Opposing EUR/USD and BRL/RUB vegas of `±20,000 USD` per volatility point gave USD 226,087.31 using the two published proxies.

## Choice and rationale

The observed calculator drops USD self-delta rows and keeps signed exposure for other currencies. It canonicalizes opposite vega pair spellings during aggregation without changing amount signs.

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

The accepted calculation requires qualification of the directed quote and volatility basis before pair canonicalization. Expiry remains traceable through the ordinary vega reduction. The historical profile uses USD as its calculation currency.

## Source and validation evidence

Inspected independent formula vectors check EUR×7.4, BRL×14.7 and zero USD delta. Inline test checks EUR/+100, JPY/-100 partial offset at rho0.5. Table tests pin category thresholds and HVR/weights; no complete cross-pair benchmark was established.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [src/types/simm_curvature.rs](../../../../finstack-quant/margin/src/types/simm_curvature.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json).

Existing evidence: [tests/production_simm_csa_audit.rs](../../../../finstack-quant/margin/tests/production_simm_csa_audit.rs), [tests/simm_schedule_parity.rs](../../../../finstack-quant/margin/tests/simm_schedule_parity.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
