# FACTOR-012: Hierarchy inventory and small-bucket fold-up

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

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

Bottom-up merging/refitting, adaptive thresholds based on effective observations or weight concentration, and pure shrinkage are distinct alternatives. Count thresholds are simple but do not establish estimation precision or independence.

## Implementation and evidence

- [src/credit/calibration/inventory.rs](../../../../finstack-quant/factor/src/credit/calibration/inventory.rs) — inspected implementation.
- [src/credit/calibration/config.rs](../../../../finstack-quant/factor/src/credit/calibration/config.rs) — inspected implementation.
- [src/credit/hierarchy.rs](../../../../finstack-quant/factor/src/credit/hierarchy.rs) — inspected implementation.

[src/credit/calibration/tests.rs](../../../../finstack-quant/factor/src/credit/calibration/tests.rs) locates all-member counting and BucketOnly fold checks; [tests/credit_calibration.rs](../../../../finstack-quant/factor/tests/credit_calibration.rs) locates sparse-bucket parent labels. [tests/credit_decomposition.rs](../../../../finstack-quant/factor/tests/credit_decomposition.rs) checks that folded names are excluded from snapshot bucket means. These are contract/conservation tests, not evidence that five members is statistically adequate.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
