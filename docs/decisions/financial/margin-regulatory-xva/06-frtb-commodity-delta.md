# MARGIN-REGULATORY-XVA-006: Commodity name, tenor and delivery-location delta aggregation

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

Commodity delta uses explicit commodity name, bucket, tenor and delivery-location labels. It reuses the credit-style factorized correlation kernel with commodity-specific weights and correlations, rather than modelling a commodity price process.

## Algorithm and conventions

1. Multiply P&L per 1% price move by bucket 1–11 weights `[30,35,60,80,40,45,20,35,25,35,50]`.
2. Same commodity names correlate at one. Different names use bucket correlations `[0.55,0.95,0.40,0.80,0.60,0.65,0.55,0.45,0.15,0.40,0.15]`. Multiply by one for equal tenors or `0.99` for different tenors, and one for equal locations or `0.999` for different locations.
3. Apply scenario scaling to the product, then calculate bucket K and raw S. Commodity bucket 11 does **not** use the generic `other_bucket` sum-absolute rule; its within-bucket different-name correlation is `0.15`.
4. Inter-bucket gamma is `0.20` for two buckets from 1–10 and zero if either is 11. Apply scenario scaling and record 02's conditional capped-S formula.

## Selection and controls

The commodity branch delegates to `csr_bucketed_delta` with class `Commodity`. Validation requires a nonblank delivery location and an exact recognized tenor, as well as a valid name/bucket and finite amount. Bucket identity and delivered-price normalization remain caller supplied.

## Limitations and failure behavior

The shared kernel's fourth string means delivery location here, bond/CDS basis for credit; those meanings must not be interchanged by a generic feed. Delivery-tenor basis risk is binary equal/different, not term-distance dependent. No averaging of price risk across actual delivery dates, classification logic or physical-unit conversion occurs.

## Alternatives and tradeoffs

A security/delivery mapping layer could construct risk factors from contract schedules. Retaining the current externally classified input is smaller and transparent but places completeness and unit conversion on the caller.

## Evidence and role

Runtime: [src/regulatory/frtb/delta.rs](../../../../finstack-quant/margin/src/regulatory/frtb/delta.rs); exact weights: [src/regulatory/frtb/params/commodity.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/commodity.rs); name/inter-bucket rules: [src/regulatory/frtb/params/mod.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs). [tests/frtb_sba_charges.rs](../../../../finstack-quant/margin/tests/frtb_sba_charges.rs) contains an inspected hand-written two-coal/one-power example and a high-correlation scenario maximum. Additional location/tenor and mixed-sign independent cases were not established. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
