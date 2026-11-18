# MARGIN-REGULATORY-XVA-005: Equity spot and repo delta aggregation

- Decision status: **Accepted 2026-11-17 for checked equity spot and repo delta.**
- Implementation status: Existing behavior is described below. Qualified factor and index admission, checked aggregation and caller reconciliation remain pending.
- Validation status: Primary source, Table 10 weights and independent examples checked. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked equity spot and repo delta

1. **Require exact equity or approved index identity and a qualified name
   association.** Require the bucket and explicit spot or repo kind. Net
   only identical qualified factors. Admit spot sensitivity as signed
   reporting-currency P&L for a `1%` relative price increase. Admit repo
   sensitivity as signed P&L per percentage point of the repo rate, obtained
   by multiplying the change for an upward `1 bp` parallel repo curve shift
   by `100`. Equity repo enters delta. Equity vega and curvature use the
   equity spot basis. Applicable fixed-income repo curve risk belongs to
   GIRR.
   [FRTB input contract](01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance) ·
   [GIRR delta](03-frtb-girr-delta.md#accepted-decision-for-checked-girr-delta) ·
   [Basel MAR21.12 and MAR21.21–22](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
2. **Admit all 13 Table 9 buckets with dated classification evidence.** Use
   the qualified global market cap, applicable economy and sector, and the
   issuer's most material operations where required. Exactly USD 2 billion
   qualifies as large cap. Bucket 11 requires an evidenced other-sector
   assignment. Missing or conflicting evidence makes the affected result
   unavailable.
   [Basel MAR21.72–76](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Require an explicit index election.** Use qualified constituent
   sensitivities when look-through applies. For an eligible whole-index
   election, use a bucket from 1 to 11 when its constituent weight exceeds
   `75%`. Otherwise use bucket 12 when at least `75%` of constituent weight
   meets both the large-cap and advanced-economy definitions, or bucket 13.
   Preserve the election and use it consistently for delta and curvature.
   [Basel MAR21.31–34](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
4. **Apply the matching weight once.** The spot multipliers for the
   prescribed preparation units in buckets 1–13 are
   `[55, 60, 45, 55, 30, 35, 40, 50, 70, 50, 70, 15, 25]`.
   The repo multiplier is the spot multiplier divided by `100`. In
   bucket 1 a repo change of `+1` reporting-currency unit per basis point
   becomes `+100` per percentage point, then `+55` after weighting.
   [Basel MAR21.77, Table 10](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
5. **Apply the scenario once to the completed pair correlation.** For
   distinct same-kind factors, use `0.15` in buckets 1–4, `0.25` in 5–8,
   `0.075` in 9, `0.125` in 10 and `0.80` in 12–13. For a mixed spot and
   repo pair sharing the same qualified issuer or index name, use `0.999`.
   For different names multiply the bucket coefficient by `0.999`. The
   name association used for a mixed pair is distinct from exact factor
   identity for netting.
   [Basel MAR21.78](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
6. **Use the absolute sum for bucket 11 inside the cross-bucket root.**
   After factor netting set its `K` to the sum of absolute weighted factors
   and retain its raw signed `S`. Include its `K²` in the cross-bucket
   sum. Use zero base cross-correlation if either bucket is 11, `0.15`
   between buckets 1–10, `0.75` between buckets 12 and 13, and `0.45` for
   remaining distinct pairs. Apply the scenario once and use the shared
   checked quadratic with the conditional `S` cap. An ordinary bucket
   with `K = 3` and bucket 11 with `K = 4` yield a charge of `5`.
   Preserve admitted factors, weights, bucket totals and scenario evidence
   through Rust, wire results and the existing Python and WASM surfaces.
   [FRTB aggregation](02-frtb-scenarios-and-aggregation.md#accepted-decision-for-checked-frtb-scenarios-and-aggregation) ·
   [Basel MAR21.79–80](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).

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

Combining spot and repo sensitivities before risk weighting would erase a material unit and correlation distinction. The accepted design requires exact factor references and a separate qualified name association for mixed-pair correlation.

## Evidence and role

Runtime: [src/regulatory/frtb/delta.rs](../../../../finstack-quant/margin/src/regulatory/frtb/delta.rs); weights: [src/regulatory/frtb/params/equity.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/equity.rs); correlations: [src/regulatory/frtb/params/mod.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs). [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) pins the direct bucket-1 repo calculation `100×0.55=55`; [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) locates spot weight examples. Parameter tests pin literal tables; they do not certify input normalization. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

The inspected implementation documents the observed behavior. Actionable reconciliation and qualification work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
