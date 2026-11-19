# MARGIN-REGULATORY-XVA-009: CSR, equity and commodity volatility-scaled vega aggregation

- Decision status: **Accepted 2026-11-17 for checked credit, equity and commodity vega.**
- Implementation status: Existing behavior is described below. Qualified underliers, five-point expiry admission, index and tranche-pair evidence, and checked aggregation and result paths remain pending.
- Validation status: Primary references and weight schedules checked. Independent examples covered expiry admission, equity weights, index and tranche-pair thresholds, and other-sector aggregation. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked credit, equity and commodity vega

1. **Require prepared, volatility-scaled vega.** Use signed `σ × ∂PV/∂σ`
   in the reporting currency, with matching volatility units and PV
   before CVA. Admit qualified normal or lognormal models for credit and
   lognormal models for equity and commodity. Assign `10Y` and carry
   the RRAO requirement for options without a specified maturity. A
   CTP securitisation qualifies for the vega exemption only with
   evidence that it has no implied volatility.
   [FRTB input contract](01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance) ·
   [Basel MAR21.25–30](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
2. **Use the exact underlier and five-point expiry grid.** Use issuer for
   non-sec credit, reference name or qualified whole index for CTP,
   tranche for non-CTP, equity spot underlier for equity, and commodity
   spot identity for commodity. Net only matching class, underlier,
   bucket and expiry at `0.5Y`, `1Y`, `3Y`, `5Y` or `10Y`. Require
   documented upstream mapping. Carry bond/CDS basis, underlying
   maturity and delivery location in source provenance.
   [Basel MAR21.8–13 and MAR21.94 FAQ](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Qualify index-option vega on its own pricing basis.** Admit the
   option's own implied volatility when supported, including when delta
   uses constituent exposures. For equity and non-sec credit indices, use
   a sector bucket when its weighted share exceeds `75%`. Otherwise use
   the applicable equity or non-sec credit index bucket. Keep CTP indices
   whole and use a sector bucket when its weighted share is at least
   `75%`, with bucket 16 as the fallback.
   [Credit factor classification](04-frtb-credit-delta.md#accepted-decision-for-checked-credit-spread-delta) ·
   [Equity factor classification](05-frtb-equity-delta.md#accepted-decision-for-checked-equity-spot-and-repo-delta) ·
   [Basel MAR21.33, MAR21.38 and the MAR21.58 FAQ](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
4. **Apply the vega weight once.** Use `1.00` for all three credit classes,
   commodity and equity buckets 9–11. Use the unrounded `0.55 × √2`
   for equity buckets 1–8 and 12–13, approximately `77.7817%`.
   [Basel MAR21.92, Table 13 and footnote 24](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
5. **Combine underlier and expiry correlations before applying the
   scenario.** Use `exp(−0.01 × |T_i−T_j| / min(T_i,T_j))` for the expiry
   coefficient. Matching underliers contribute `1`. Distinct names use
   `0.35` for ordinary non-sec and CTP buckets and `0.80` for non-sec
   index buckets. Use the accepted equity spot and commodity coefficients.
   For non-CTP, use `1` for a qualified same-tranche pair and `0.40`
   otherwise. The accepted same-tranche test requires strictly more than
   `80%` notional overlap, so exactly `80%` gets `0.40`. Qualifying
   pairwise overlap changes correlation while preserving distinct tranche
   identities for netting. Cap the product at one and apply the scenario once.
   [Equity coefficients](05-frtb-equity-delta.md#accepted-decision-for-checked-equity-spot-and-repo-delta) ·
   [Commodity coefficients](06-frtb-commodity-delta.md#accepted-decision-for-checked-commodity-delta) ·
   [Basel MAR21.54–55, MAR21.60, MAR21.68 and MAR21.93–94](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
6. **Apply the class-specific other-sector rules.** Sum absolute net
   weighted factors within non-sec and CTP bucket 16, non-CTP bucket 25
   and equity bucket 11. Use ordinary within-bucket correlation for
   commodity bucket 11. The non-sec, CTP, equity and commodity other
   buckets enter their cross-bucket roots with zero cross-correlation.
   For non-CTP use `√(K_1² + … + K_24²) + K_25`. The elected profile
   explicitly adopts the outside-root treatment for vega based on
   Basel's delta rule.
   [Basel MAR21.56–57, MAR21.69–71, MAR21.79–85 and MAR21.95](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
7. **Use checked aggregation and preserve coverage.** Use each class's
   prescribed cross-bucket coefficients and the conditional capped-sum
   retry where applicable. Carry the source mapping, net factors,
   applied weights, bucket charges and scenario evidence through Rust,
   wire, Python and WASM. Missing required evidence makes the affected
   charge unavailable.
   [FRTB aggregation](02-frtb-scenarios-and-aggregation.md#accepted-decision-for-checked-frtb-scenarios-and-aggregation) ·
   [Basel MAR21.4, MAR21.6 and MAR21.95](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

Non-FX/non-GIRR vega shares a `(name,bucket,option_maturity)` aggregation rule. Credit, equity and commodity classes retain distinct risk weights, name correlations, other-bucket treatment and cross-bucket rules. No bond/CDS basis or delivery-location dimension is carried in these vega keys.

## Algorithm and conventions

1. Weight CSR non-sec, CTP, non-CTP and commodity vega by `1.00`. Equity buckets 9–11 use `1.00`; equity buckets 1–8 and 12–13 use `0.55×sqrt(2)`.
2. Correlate equal names at one, otherwise use the class/bucket name correlation listed in records 04–06. Multiply by `exp(−0.01×|T_i−T_j|/max(min(T_i,T_j),1/365))`, then scenario-transform the product once.
3. For other buckets (CSR non-sec/CTP 16, non-CTP 25, equity 11), use `K=Σ|weighted_vega|` and raw S. Commodity 11 remains correlated internally.
4. Non-CTP returns root-sum-square ordinary K plus bucket 25 outside the root. All other classes use their delta-class bucket-correlation matrix with scenario scaling and record 02's conditional capped-S procedure.

## Selection and controls

Risk class selects the map and fixed weight rule. Validation checks nonblank names, supported bucket IDs, the ten standard maturity labels shared with GIRR delta, and finite amounts. The engine consumes prepared, volatility-scaled vega. Equity applies the cap to small-cap and other-sector buckets.

## Limitations and failure behavior

The runtime uses bare name equality for ordinary correlations. The current non-CTP map carries no pairwise overlap evidence. Current CTP entry points label the underlier as a tranche. The accepted inputs require qualified reference names or whole indices for CTP and qualified tranche identity and pairwise overlap for non-CTP.

## Alternatives and tradeoffs

The classes share the expiry decay and checked aggregation where their rules agree. Class-specific underlier admission and residual rules preserve the prescribed factor and bucket boundaries.

## Evidence and role

The inspected tests cover commodity and both CSR securitisation variants, and pin small-cap equity vega. Independent arithmetic checked the equity weight and cap, the index and overlap thresholds, commodity bucket 11 and the difference made by the non-CTP outside-root addition. Runtime verification remains pending.
[Runtime and vega tests](../../../../finstack-quant/margin/src/regulatory/frtb/vega.rs) ·
[Input validation](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs) ·
[Equity weights](../../../../finstack-quant/margin/src/regulatory/frtb/params/equity.rs) ·
[Credit weights](../../../../finstack-quant/margin/src/regulatory/frtb/params/csr.rs) ·
[Commodity weights](../../../../finstack-quant/margin/src/regulatory/frtb/params/commodity.rs) ·
[Correlation rules](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs) ·
[Charge examples](../../../../finstack-quant/margin/tests/frtb_sba_charges.rs) ·
[Equity regression](../../../../finstack-quant/margin/tests/audit_regressions.rs) ·
[FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

Reconciliation and qualification work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
