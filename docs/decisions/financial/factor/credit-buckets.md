# FACTOR-012: Hierarchy inventory and small-bucket fold-up

**Decision status:** Accepted 2026-11-17 for eligible bucket membership, explicit thresholds and replayable folds.

**Implementation status:** All-member counting and independent level folds are implemented. Explicit threshold admission and checked fold evidence remain pending.

**Validation status:** Four focused credit fold tests passed on 2026-11-17 against current behavior. Threshold admission and replay across folded parents and active children remain to be qualified.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Build canonical nested membership from the admitted panel and validated tags. Count each eligible issuer once per level, including `BucketOnly` members that contribute to the bucket. Ignore tags for issuers outside the panel. [Credit identity](credit-matching.md) · [Panel eligibility](credit-panel.md).
2. Require one explicit positive threshold per hierarchy level. Compare each bucket independently against its original membership. A narrower bucket can remain active when its parent folds. For example, a parent with three members folds at threshold four while a child with two members passes threshold two.
3. Treat a fold as skipping that level. Set its loading to zero and exclude folded members from that level's mean. Continue the residual through later active levels without refitting ancestors. Preserve and validate issuer, level, bucket, member count and threshold for replay. Parent paths describe ancestry. They confer no active factor or pooled exposure.

## Observed choice and rationale

Build nested bucket paths from ordered issuer tags and suppress factor levels whose membership is below a configured threshold. Fold-up records are operative state used again during snapshot decomposition, not merely diagnostic decoration.

## Input domain and conventions

Every calibrated issuer must supply all ordered hierarchy tags; dot is reserved as the value-path separator. Inventory counts all calibrated members, including BucketOnly names. Extra tag-panel issuers outside the history/mode universe do not enter counts. Built-in/custom keys must remain distinct; duplicate keys are rejected by final model validation.

## Algorithm stages

1. Walk issuers in BTreeMap order, collect tag taxonomy sets and complete path prefixes such as IG, IG.EU, IG.EU.Financial.
2. Build per-level bucket membership sets and integer counts.
3. At each level independently compare count with its threshold. A missing threshold entry means 5; default_for_levels also returns 5 each.
4. For every undersized bucket, mark each member folded at that level and record original path, level, reason and immediate parent (or <root>).
5. Sort records by level then issuer ID. The peel excludes folded members at that level and leaves their level beta zero; their residual continues through remaining levels and the issuer adder.

## Selection and controls

Thresholds are raw usize values with no positive-minimum validation; zero prevents count-based folds. Each level uses the original inventory rather than recalculating counts after previous folds. A later child level can remain active after its parent folded when the caller supplies different thresholds.

## Limitations, failures, and fallbacks

Fold-up is not literal pooling or refitting into the recorded parent: ancestors have already been processed, and the record identifies the skipped level. An invariant such as every descendant folded after a parent is not implemented. Taxonomy describes the calibrated universe only. Diagnostics openness on the wire does not permit dropping known fold-up records.

## Alternatives and tradeoffs

Count thresholds govern eligibility. Estimation precision and concentration require separate qualification. Skipping a bucket can leave common spread variation in the issuer residuals.

## Implementation and evidence

- [src/credit/calibration/inventory.rs](../../../../finstack-quant/factor/src/credit/calibration/inventory.rs) — inspected implementation.
- [src/credit/calibration/config.rs](../../../../finstack-quant/factor/src/credit/calibration/config.rs) — inspected implementation.
- [src/credit/hierarchy.rs](../../../../finstack-quant/factor/src/credit/hierarchy.rs) — inspected implementation.

[Calibration tests](../../../../finstack-quant/factor/src/credit/calibration/tests.rs), [integration tests](../../../../finstack-quant/factor/tests/credit_calibration.rs) and [snapshot tests](../../../../finstack-quant/factor/tests/credit_decomposition.rs) passed for current membership and fold behavior. Qualify exact threshold counts, independent levels and checked replay of folds.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
