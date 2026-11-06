# MARGIN-REGULATORY-XVA-009: CSR, equity and commodity volatility-scaled vega aggregation

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

Non-FX/non-GIRR vega shares a `(name,bucket,option_maturity)` aggregation rule. Credit, equity and commodity classes retain distinct risk weights, name correlations, other-bucket treatment and cross-bucket rules. No bond/CDS basis or delivery-location dimension is carried in these vega keys.

## Algorithm and conventions

1. Weight CSR non-sec, CTP, non-CTP and commodity vega by `1.00`. Equity buckets 9–11 use `1.00`; equity buckets 1–8 and 12–13 use `0.55×sqrt(2)`.
2. Correlate equal names at one, otherwise use the class/bucket name correlation listed in records 04–06. Multiply by `exp(−0.01×|T_i−T_j|/max(min(T_i,T_j),1/365))`, then scenario-transform the product once.
3. For other buckets (CSR non-sec/CTP 16, non-CTP 25, equity 11), use `K=Σ|weighted_vega|` and raw S. Commodity 11 remains correlated internally.
4. Non-CTP returns root-sum-square ordinary K plus bucket 25 outside the root. All other classes use their delta-class bucket-correlation matrix with scenario scaling and record 02's conditional capped-S procedure.

## Selection and controls

Risk class selects the map and fixed weight rule. Names, bucket IDs and exact standard maturity labels are validated. The engine receives already volatility-scaled vega, so a missing market-volatility multiplier is not repaired here. Equity's small-cap/other cap is an actual runtime branch, not merely a provenance note.

## Limitations and failure behavior

Input dimensions omit basis/location axes available in delta. Current runtime applies the 9–11 cap and other-bucket formula described above.

## Alternatives and tradeoffs

Separate class kernels could expose more regulatory exceptions but duplicate a genuinely shared current formula. Extending vega factor identity is a new financial representation choice, not a harmless refactor.

## Evidence and role

Runtime and weight dispatch: [src/regulatory/frtb/vega.rs](../../../../finstack-quant/margin/src/regulatory/frtb/vega.rs); [src/regulatory/frtb/params/equity.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/equity.rs), [src/regulatory/frtb/params/csr.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/csr.rs), [src/regulatory/frtb/params/commodity.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/commodity.rs) and [src/regulatory/frtb/params/mod.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs). Inspected hand derivations in [tests/frtb_sba_charges.rs](../../../../finstack-quant/margin/tests/frtb_sba_charges.rs) cover commodity and CSR securitisation variants; [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) pins small-cap equity vega. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
