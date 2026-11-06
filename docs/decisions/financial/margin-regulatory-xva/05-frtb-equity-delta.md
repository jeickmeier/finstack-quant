# MARGIN-REGULATORY-XVA-005: Equity spot and repo delta aggregation

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

Equity spot and repo-rate risks share issuer/bucket grouping but retain separate risk-factor types. Spot sensitivity is P&L per 1% equity-price move; repo sensitivity is P&L per 1pp repo-rate move. The engine applies different weight scales to those inputs.

## Algorithm and conventions

1. Spot weights for buckets 1–13 are `[55,60,45,55,30,35,40,50,70,50,70,15,25]`. Repo uses the same numeric weight divided by 100; e.g. bucket 1 repo multiplies by `0.55`, versus `55` for spot.
2. Within each bucket, equal names have base name correlation one; unequal names use `0.15` for 1–4, `0.25` for 5–8, `0.075` for 9, `0.125` for 10, and `0.80` for 12–13. Multiply by one for two spot or two repo factors and by `0.999` for mixed spot/repo factors. Apply scenario scaling to that product.
3. Bucket 11 (“other”) bypasses correlation and uses sum absolute weighted sensitivities, keeping raw signed S.
4. Cross-bucket base gamma is zero if either is bucket 11; `0.15` when both are 1–10; `0.75` when both are index buckets 12–13; otherwise `0.45`. Apply scenario scaling and ordinary conditional capped-S aggregation.

## Selection and controls

Selected by the equity delta branch. Supported bucket IDs are validated before lookup. Matching issuer strings link spot and repo risk, but no security master verifies that identity. Empty maps contribute zero. There is no equity-repo curvature or separately typed repo-vega map in this representation.

## Limitations and failure behavior

The numeric spot and repo weight scales differ by a factor of 100. Bucket 11 permits no constituent diversification before a zero-gamma cross-bucket root, which differs from simply summing all other and ordinary bucket capital.

## Alternatives and tradeoffs

Combining spot and repo sensitivities before risk weighting would erase a material unit and correlation distinction. Separate issuer-normalized factor records would make identity more explicit but do not change the adopted aggregation by themselves.

## Evidence and role

Runtime: [src/regulatory/frtb/delta.rs](../../../../finstack-quant/margin/src/regulatory/frtb/delta.rs); weights: [src/regulatory/frtb/params/equity.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/equity.rs); correlations: [src/regulatory/frtb/params/mod.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs). [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) pins the direct bucket-1 repo calculation `100×0.55=55`; [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) locates spot weight examples. Parameter tests pin literal tables; they do not certify input normalization. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
