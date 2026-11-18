# MARGIN-REGULATORY-XVA-004: CSR non-sec, CTP and non-CTP delta weights and correlation structure

- Decision status: **Accepted 2026-11-17 for checked credit spread delta across non-sec, CTP and non-CTP.**
- Implementation status: Existing behavior is described below. Qualified factor and index admission, elective covered-bond weight, checked aggregation and caller reconciliation remain pending.
- Validation status: Primary references inspected and the three weight schedules and 55 distinct sector-pair coefficients compared with the published tables. Independent examples checked index assignment, basis correlation and residual addition. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked credit spread delta

1. **Admit the exact factor for each class.** Use issuer spread curves for
   non-securitisation, underlying reference-name curves for ordinary CTP and
   tranche spread curves for non-CTP. Require prepared signed reporting-currency
   P&L per percentage point at `0.5Y`, `1Y`, `3Y`, `5Y` or `10Y`. Keep bond and
   CDS basis distinct. Require dated evidence for the class, bucket and
   applicable sector, quality and seniority. Net only identical qualified
   factors and reject conflicting assignments.
   [FRTB input contract](01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance) ·
   [Basel MAR21.9–11](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23) ·
   [CTP eligibility, MAR20.5](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/20/inforce/2023-01-01/published/2020-03-27).
2. **Make index treatment explicit.** For an eligible ordinary whole-index
   election, assign a sector bucket when its weighted share exceeds 75%.
   Otherwise use bucket 17 when at least 75% is investment grade, or bucket
   18. Require qualified constituent sensitivities where look-through applies.
   Keep a CTP index as one factor. Assign it to a sector bucket at a weighted
   share of at least 75%, or to bucket 16 if no sector qualifies. Carry the
   index election and classification evidence.
   [Basel MAR21.31–34 and the CTP index FAQ](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Use the prescribed weights with an explicit covered-bond election.** Use
   Table 4 for non-sec buckets 1–18 and Table 6 for CTP buckets 1–16. For
   non-CTP, use Table 8 for 1–8, multiply that row by `1.25` for 9–16 and
   `1.75` for 17–24, and use `3.5` for bucket 25. Retain `2.5` for non-sec
   bucket 8 under the standard election. An explicitly elected `1.5` requires
   qualifying covered bonds rated AA− or higher and a profile that permits
   the relief.
   [Basel MAR21.53, MAR21.59 and MAR21.64–67](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
4. **Build the ordinary within-bucket correlation from three coefficients.**
   A qualified match contributes `1`. Multiply the name or tranche, tenor
   and curve-basis coefficients, then apply the scenario transform once to
   the product. Use these coefficients for differences.

   | Class | Different name or tranche | Different tenor | Different curve basis |
   | --- | ---: | ---: | ---: |
   | Non-sec | `0.35`, or `0.80` in buckets 17–18 | `0.65` | `0.999` |
   | CTP | `0.35` | `0.65` | `0.99` |
   | Non-CTP | `0.40` | `0.80` | `0.999` |

   For non-CTP, require qualified evidence for the same-tranche relationship,
   including strictly more than 80% notional overlap under the prescribed
   test. Distinct tranches retain their identities even when they share an issuer.
   [Basel MAR21.54–55, MAR21.60 and MAR21.68](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
5. **Apply the specific other-sector rules.** Set `K = Σ|WS|` after factor
   netting and retain raw `S = ΣWS`. Non-sec and CTP bucket 16 enter their
   cross-bucket quadratic with zero cross-correlation. For non-CTP, combine
   buckets 1–24 by root-sum-square and add bucket 25 outside the root.
   [Basel MAR21.56–57 and MAR21.69–71](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
6. **Use the prescribed sector matrix across non-sec and CTP buckets.**
   Apply Table 5 with a `0.50` rating multiplier only when both buckets are
   in 1–15 and their credit-quality groups differ. Apply the scenario
   transformation to the resulting correlation, then use record 002's checked
   quadratic and conditional capped retry.
   [Basel MAR21.57 and MAR21.61](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
7. **Preserve evidence through existing result paths.** Carry factor identity,
   classification, effective weight, bucket charges and scenario diagnostics
   through checked Rust state, wire restoration, Python and WASM. Report
   securitisation SBA on its qualified scope. Full capital requires applicable
   DRC and RRAO coverage under record 002.
   [FRTB aggregation contract](02-frtb-scenarios-and-aggregation.md#accepted-decision-for-checked-frtb-scenarios-and-aggregation).

## Choice and domain

The observations below describe the audited design. The accepted method binds the class, factor and bucket before netting and preserves separate sector aggregation rules.

Credit-spread delta separates non-securitisation, correlation-trading-portfolio securitisation (CTP) and non-CTP securitisation. Caller labels determine name/tranche, bucket, tenor and bond/CDS basis identity. All values are P&L per 1pp spread shift in the reporting currency.

## Algorithm and conventions

1. Apply the bucket's numeric-percent risk weight. Non-sec buckets 1–18 use `[0.5,1,5,3,3,2,1.5,2.5,2,4,12,7,8.5,5.5,5,12,1.5,5]`. CTP buckets 1–16 use `[4,4,8,5,4,3,2,6,13,13,16,10,12,12,12,13]`. Non-CTP buckets 1–8 use `[0.9,1.5,2,2,0.8,1.2,1.2,1.4]`; 9–16 are 1.25 times that row, 17–24 are 1.75 times it, and 25 is `3.5`.
2. For ordinary buckets, correlation before scenario scaling is `r_name×r_tenor×r_basis`. Same names use one; different names use `0.35` for non-sec/CTP buckets below 17, `0.80` for non-sec index buckets 17–18, and `0.40` for non-CTP. Same tenor/basis each use one. Different tenors use `0.65` for non-sec/CTP and `0.80` for non-CTP. Different basis labels use `0.99` in CTP and `0.999` otherwise. Scenario-transform the product once.
3. Other-sector buckets (16 for non-sec/CTP, 25 for non-CTP) use `K=Σ|WS|` with no intra-bucket diversification and raw `S=ΣWS`.
4. Non-sec/CTP cross-bucket gamma comes from the fixed 11×11 sector matrix in `params::credit_inter_correlation`. Map buckets 1–8 to sectors 0–7, buckets 9–15 to sectors 0–6, and buckets 16–18 to sectors 8–10. Multiply by `0.5` when both buckets are ≤15 and one is IG (≤8) and the other HY (>8); otherwise multiply by one. The matrix gives zero cross-correlations to sector 8 (“other”), 0.45 between indices and ordinary sectors, and 0.75 between the two index sectors. Apply the scenario transformation, then record 02's conditional capped-S aggregation.
5. Non-CTP has zero cross-bucket diversification terms: root-sum-square ordinary bucket K values and add bucket 25's K outside the root.

## Selection and controls

Risk-class dispatch selects the corresponding fixed map/table; validation accepts only each table's bucket set and the shared standard tenor strings. Unknown public risk-weight lookups yield NaN. The standard covered-bond weight is 2.5; the source identifies a discretionary lower alternative but this engine does not select it.

## Limitations and failure behavior

Bucket/name/basis assignment and credit quality are external. A name placed in multiple buckets is not globally rejected. Tenor correlations depend on label equality, not numerical distance. CTP and non-CTP portfolios are supported for SBA sensitivity aggregation; the separate DRC implementation is non-securitisation only, so CSR securitisation support does not establish a complete securitisation capital framework.

## Alternatives and tradeoffs

The accepted method uses qualified bucket and factor evidence with the prescribed products of name or tranche, tenor and basis correlations. The inputs retain source classification and route selection for replay.

## Evidence and role

Runtime grouping/correlations: [src/regulatory/frtb/delta.rs](../../../../finstack-quant/margin/src/regulatory/frtb/delta.rs); weights: [src/regulatory/frtb/params/csr.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/csr.rs); exact sector matrix: [src/regulatory/frtb/params/mod.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs). [tests/frtb_sba_charges.rs](../../../../finstack-quant/margin/tests/frtb_sba_charges.rs) contains an inspected hand-written non-CTP aggregate example and located CTP/table-boundary tests; [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) explicitly derives a bond/CDS `0.999` offset case. These are formula evidence within their cases, not external certification. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

Implementation reconciliation and independent qualification are tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
