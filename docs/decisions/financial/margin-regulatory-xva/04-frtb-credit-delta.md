# MARGIN-REGULATORY-XVA-004: CSR non-sec, CTP and non-CTP delta weights and correlation structure

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

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

A separately validated classification feed would make bucket assignments auditable. Additional security-level dimensions or eligibility treatments require distinct design, rather than changing the same-name netting rule silently. A full correlation matrix per factor provides flexibility but adds validation burden; this implementation fixes the factorized rules.

## Evidence and role

Runtime grouping/correlations: [src/regulatory/frtb/delta.rs](../../../../finstack-quant/margin/src/regulatory/frtb/delta.rs); weights: [src/regulatory/frtb/params/csr.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/csr.rs); exact sector matrix: [src/regulatory/frtb/params/mod.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs). [tests/frtb_sba_charges.rs](../../../../finstack-quant/margin/tests/frtb_sba_charges.rs) contains an inspected hand-written non-CTP aggregate example and located CTP/table-boundary tests; [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) explicitly derives a bond/CDS `0.999` offset case. These are formula evidence within their cases, not external certification. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
